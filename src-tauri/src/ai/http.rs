//! HTTP helpers shared by the provider clients.

use std::time::Duration;

use serde_json::{Map, Value};

/// Timeout for one inference request; local vision models on CPU can take minutes per photo.
pub const GENERATE_TIMEOUT: Duration = Duration::from_secs(600);

/// Timeout for status checks and model listings.
pub const QUICK_TIMEOUT: Duration = Duration::from_secs(10);

/// HTTP client for every provider except Ollama (whose client is unchanged); identifies the app in `User-Agent`.
/// Redirects are followed only within the same origin: reqwest keeps custom credential headers such as Claude's `x-api-key` on a redirect to another host.
pub fn client() -> reqwest::Client {
    let same_origin = reqwest::redirect::Policy::custom(|attempt| {
        let origin = |u: &reqwest::Url| {
            (
                u.scheme().to_string(),
                u.host_str().map(str::to_string),
                u.port_or_known_default(),
            )
        };
        let first = attempt.previous().first().map(origin);
        if attempt.previous().len() > 5 {
            attempt.error("too many redirects")
        } else if first.is_some_and(|first| first == origin(attempt.url())) {
            attempt.follow()
        } else {
            attempt.stop()
        }
    });
    reqwest::Client::builder()
        .user_agent(concat!("Attributor/", env!("CARGO_PKG_VERSION")))
        .redirect(same_origin)
        .build()
        .unwrap_or_else(|e| {
            log::warn!("HTTP client setup failed ({e}); using defaults");
            reqwest::Client::new()
        })
}

/// Joins an API root and an endpoint path, tolerating a trailing slash on the root.
pub fn url(base: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

/// Reads an error response into a short message: the `error.message` (or a string `error`/`detail`) of a JSON body, else the trimmed text.
pub async fn error_detail(resp: reqwest::Response) -> String {
    let text = resp.text().await.unwrap_or_default();
    let message = serde_json::from_str::<Value>(&text).ok().and_then(|v| {
        let field = |key: &str| v.get(key).and_then(Value::as_str).map(str::to_string);
        v.get("error")
            .and_then(|e| e.get("message"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| field("error"))
            .or_else(|| field("detail"))
            .or_else(|| field("message"))
    });
    let message = message.unwrap_or_else(|| text.trim().to_string());
    truncate(&message, 500)
}

/// Shortens `s` to at most `max` characters, marking the cut with an ellipsis.
pub fn truncate(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((cut, _)) => format!("{}…", &s[..cut]),
        None => s.to_string(),
    }
}

/// Refuses to send a cloud API key over plain HTTP to a host other than this machine.
///
/// # Errors
///
/// Returns an error for a malformed URL or a non-HTTPS, non-loopback address.
pub fn check_key_destination(base_url: &str) -> Result<(), String> {
    let url =
        reqwest::Url::parse(base_url).map_err(|e| format!("invalid base URL {base_url}: {e}"))?;
    let host = url
        .host_str()
        .unwrap_or_default()
        .trim_start_matches('[')
        .trim_end_matches(']');
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback());
    if url.scheme() == "https" || loopback {
        Ok(())
    } else {
        Err(format!(
            "refusing to send the API key over plain HTTP to {base_url}; use an https:// address"
        ))
    }
}

/// Copies the profile's run options into the request body as top-level fields; options win over the defaults already in `body`.
pub fn merge_options(body: &mut Map<String, Value>, options: &Value) {
    if let Some(options) = options.as_object() {
        for (key, value) in options {
            body.insert(key.clone(), value.clone());
        }
    }
}

/// A one-shot HTTP server for testing the provider clients without a real provider.
#[cfg(test)]
pub(crate) mod mock {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// A request captured by [`serve_once`].
    pub(crate) struct Captured {
        /// Request line and headers, as sent.
        pub head: String,
        pub body: serde_json::Value,
    }

    impl Captured {
        /// The value of header `name` (case-insensitive).
        pub(crate) fn header(&self, name: &str) -> Option<&str> {
            self.head
                .lines()
                .skip(1)
                .filter_map(|l| l.split_once(':'))
                .find(|(k, _)| k.trim().eq_ignore_ascii_case(name))
                .map(|(_, v)| v.trim())
        }

        pub(crate) fn request_line(&self) -> &str {
            self.head.lines().next().unwrap_or_default()
        }
    }

    /// Serves one request on `127.0.0.1` with `status` and a JSON `body`. Returns the server's base URL and a handle yielding the captured request.
    pub(crate) async fn serve_once(
        status: u16,
        body: serde_json::Value,
    ) -> (String, tokio::task::JoinHandle<Captured>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let base = format!(
            "http://{}",
            listener.local_addr().expect("listener address")
        );
        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept test request");
            let mut data = Vec::new();
            let mut buf = [0u8; 8192];
            let header_end = loop {
                let n = stream.read(&mut buf).await.expect("read test request");
                data.extend_from_slice(&buf[..n]);
                if let Some(pos) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                    break pos + 4;
                }
                assert!(n > 0, "connection closed before the request head ended");
            };
            let head = String::from_utf8_lossy(&data[..header_end]).to_string();
            let length = head
                .lines()
                .filter_map(|l| l.split_once(':'))
                .find(|(k, _)| k.trim().eq_ignore_ascii_case("content-length"))
                .and_then(|(_, v)| v.trim().parse::<usize>().ok())
                .unwrap_or(0);
            while data.len() < header_end + length {
                let n = stream.read(&mut buf).await.expect("read test body");
                assert!(n > 0, "connection closed before the body ended");
                data.extend_from_slice(&buf[..n]);
            }
            let body_bytes = &data[header_end..header_end + length];
            let text = body.to_string();
            let response = format!("HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len());
            stream
                .write_all(response.as_bytes())
                .await
                .expect("write test response");
            Captured {
                head,
                body: serde_json::from_slice(body_bytes).unwrap_or(serde_json::Value::Null),
            }
        });
        (base, handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn url_joins_with_single_slash() {
        assert_eq!(url("http://h:1/v1/", "/models"), "http://h:1/v1/models");
        assert_eq!(
            url("http://h:1/v1", "chat/completions"),
            "http://h:1/v1/chat/completions"
        );
    }

    #[test]
    fn truncate_respects_char_boundaries() {
        assert_eq!(truncate("абвгд", 3), "абв…");
        assert_eq!(truncate("abc", 3), "abc");
    }

    #[test]
    fn merge_options_overrides_defaults() {
        let mut body = json!({"max_tokens": 1}).as_object().cloned().unwrap();
        merge_options(&mut body, &json!({"max_tokens": 2, "temperature": 0.5}));
        assert_eq!(
            Value::Object(body),
            json!({"max_tokens": 2, "temperature": 0.5})
        );
    }
}

#[cfg(test)]
mod destination_tests {
    use super::check_key_destination;

    #[test]
    fn keys_go_only_to_https_or_loopback() {
        assert!(check_key_destination("https://api.openai.com/v1").is_ok());
        assert!(check_key_destination("http://127.0.0.1:4000/v1").is_ok());
        assert!(check_key_destination("http://localhost:4000").is_ok());
        assert!(check_key_destination("http://[::1]:4000").is_ok());
        assert!(check_key_destination("http://proxy.example.com/v1").is_err());
        assert!(check_key_destination("not a url").is_err());
    }
}
