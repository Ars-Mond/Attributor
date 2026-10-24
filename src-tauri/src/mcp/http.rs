//! Loopback Streamable HTTP transport of the MCP server (JSON responses only).
//! It listens on 127.0.0.1 and serves only `POST /mcp` requests that carry the bearer token, a loopback `Host` and no `Origin`; this shuts out web pages (DNS rebinding, cross-site requests) and processes that cannot read the token.

use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, Semaphore};
use tokio::time::Instant;

use super::protocol::{self, HttpHeaders, ToolHost};

/// Largest request head.
const MAX_HEAD: usize = 16 * 1024;
/// Largest request body.
const MAX_BODY: usize = 4 * 1024 * 1024;
/// Time a client has to send the whole request.
const READ_TIMEOUT: Duration = Duration::from_secs(30);
/// Connections served at once; further ones are closed right away.
const MAX_CONNECTIONS: usize = 32;

/// The tool host plus the token requests must present.
pub(crate) trait ServerHost: ToolHost + Send + 'static {
    /// The current access token (it can change while the server runs).
    fn token(&self) -> String;
}

/// A running server.
pub(crate) struct Running {
    pub port: u16,
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}

impl Running {
    /// Stops accepting connections; requests in flight still complete.
    pub(crate) async fn stop(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        let _ = self.task.await;
    }
}

/// Binds `127.0.0.1:port` (0 picks a free port) and serves requests until stopped. Must be called inside a Tokio runtime.
///
/// # Errors
///
/// Returns a readable message when the port cannot be bound, for example because another program uses it.
pub(crate) async fn start<H: ServerHost>(port: u16, host: Arc<H>) -> Result<Running, String> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                format!("port {port} is already in use")
            } else {
                format!("cannot listen on port {port}: {e}")
            }
        })?;
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(port);
    let (tx, rx) = oneshot::channel();
    let task = tokio::spawn(accept_loop(listener, host, port, rx));
    Ok(Running {
        port,
        shutdown: Some(tx),
        task,
    })
}

async fn accept_loop<H: ServerHost>(
    listener: TcpListener,
    host: Arc<H>,
    port: u16,
    mut shutdown: oneshot::Receiver<()>,
) {
    let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => {
                    let Ok(slot) = slots.clone().try_acquire_owned() else {
                        log::warn!("mcp: too many connections, refusing one");
                        continue;
                    };
                    let host = host.clone();
                    tokio::spawn(async move {
                        serve_connection(stream, &*host, port).await;
                        drop(slot);
                    });
                }
                Err(e) => {
                    log::warn!("mcp: accept failed: {e}");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }
    log::info!("mcp: server on port {port} stopped");
}

/// A parsed request head.
struct Head {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
}

impl Head {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    fn count(&self, name: &str) -> usize {
        self.headers.iter().filter(|(n, _)| n == name).count()
    }
}

/// A response about to be written.
struct Response {
    status: u16,
    body: Option<Vec<u8>>,
    extra: Vec<(&'static str, &'static str)>,
}

impl Response {
    fn json(status: u16, body: &Value) -> Self {
        Self {
            status,
            body: Some(body.to_string().into_bytes()),
            extra: Vec::new(),
        }
    }

    /// A transport-level refusal, worded as a JSON-RPC error so clients can show the reason.
    fn refuse(status: u16, message: &str) -> Self {
        Self::json(
            status,
            &json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32001, "message": message}}),
        )
    }

    fn with(mut self, name: &'static str, value: &'static str) -> Self {
        self.extra.push((name, value));
        self
    }
}

async fn serve_connection(mut stream: TcpStream, host: &impl ServerHost, port: u16) {
    let deadline = Instant::now() + READ_TIMEOUT;
    let response = match tokio::time::timeout_at(deadline, read_head(&mut stream)).await {
        Err(_) => Response::refuse(408, "request timed out"),
        Ok(Err(response)) => response,
        Ok(Ok((head, rest))) => match check(&head, port, &host.token()) {
            Err(response) => response,
            Ok(framing) => {
                match tokio::time::timeout_at(deadline, read_body(&mut stream, rest, framing)).await
                {
                    Err(_) => Response::refuse(408, "request timed out"),
                    Ok(Err(response)) => response,
                    Ok(Ok(body)) => dispatch(host, &head, &body).await,
                }
            }
        },
    };
    write_response(&mut stream, response).await;
}

async fn dispatch(host: &impl ServerHost, head: &Head, body: &[u8]) -> Response {
    let headers = HttpHeaders {
        protocol_version: head.header("mcp-protocol-version").map(str::to_owned),
        method: head.header("mcp-method").map(str::to_owned),
        name: head.header("mcp-name").map(str::to_owned),
    };
    let reply = protocol::handle(host, body, Some(&headers)).await;
    match reply.body {
        Some(body) => Response::json(reply.status, &body),
        None => Response {
            status: reply.status,
            body: None,
            extra: Vec::new(),
        },
    }
}

/// Reads the request head; returns it with any body bytes read past it.
async fn read_head(stream: &mut TcpStream) -> Result<(Head, Vec<u8>), Response> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 4096];
    let end = loop {
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos;
        }
        if buf.len() > MAX_HEAD {
            return Err(Response::refuse(431, "request head too large"));
        }
        let n = stream
            .read(&mut chunk)
            .await
            .map_err(|_| Response::refuse(400, "connection error"))?;
        if n == 0 {
            return Err(Response::refuse(400, "incomplete request"));
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let rest = buf.split_off(end + 4);
    let text = std::str::from_utf8(&buf[..end])
        .map_err(|_| Response::refuse(400, "request head is not UTF-8"))?;
    parse_head(text)
        .map(|head| (head, rest))
        .ok_or_else(|| Response::refuse(400, "malformed request"))
}

fn parse_head(text: &str) -> Option<Head> {
    let mut lines = text.split("\r\n");
    let mut request_line = lines.next()?.split(' ');
    let method = request_line.next()?.to_string();
    let target = request_line.next()?.to_string();
    if !request_line.next()?.starts_with("HTTP/1.") {
        return None;
    }
    let mut headers = Vec::new();
    for line in lines {
        // Folded header lines are obsolete (RFC 9112) and rejected.
        if line.starts_with([' ', '\t']) {
            return None;
        }
        let (name, value) = line.split_once(':')?;
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
    }
    Some(Head {
        method,
        target,
        headers,
    })
}

/// How the body is delimited.
enum Framing {
    Length(usize),
    Chunked,
}

/// Checks everything that does not need the body, so unauthenticated clients never get to upload one.
fn check(head: &Head, port: u16, token: &str) -> Result<Framing, Response> {
    let path = head.target.split('?').next().unwrap_or_default();
    if path != "/mcp" && path != "/mcp/" {
        return Err(Response::refuse(404, "not found; the MCP endpoint is /mcp"));
    }
    if head.count("host") != 1 || !is_loopback_host(head.header("host").unwrap_or_default(), port) {
        return Err(Response::refuse(
            403,
            "the Host header must name this computer's loopback address",
        ));
    }
    if head.count("origin") > 0 {
        return Err(Response::refuse(
            403,
            "requests from web pages are not accepted",
        ));
    }
    if !head
        .header("authorization")
        .and_then(bearer)
        .is_some_and(|given| token_matches(given, token))
    {
        return Err(Response::refuse(
            401,
            "missing or invalid access token; copy it from Attributor > Settings > MCP",
        )
        .with("WWW-Authenticate", "Bearer"));
    }
    if head.method != "POST" {
        return Err(Response::refuse(405, "only POST is supported").with("Allow", "POST"));
    }
    let media_type = head
        .header("content-type")
        .and_then(|v| v.split(';').next())
        .map(str::trim);
    if !media_type.is_some_and(|m| m.eq_ignore_ascii_case("application/json")) {
        return Err(Response::refuse(415, "the body must be application/json"));
    }
    if head
        .header("transfer-encoding")
        .is_some_and(|v| v.to_ascii_lowercase().contains("chunked"))
    {
        return Ok(Framing::Chunked);
    }
    let length = head
        .header("content-length")
        .and_then(|v| v.parse::<usize>().ok())
        .ok_or_else(|| Response::refuse(411, "Content-Length is required"))?;
    if length > MAX_BODY {
        return Err(Response::refuse(413, "request body too large"));
    }
    Ok(Framing::Length(length))
}

/// Whether `host` is a loopback authority for `port` (`127.0.0.1`, `localhost` or `[::1]`).
fn is_loopback_host(host: &str, port: u16) -> bool {
    let host = host.to_ascii_lowercase();
    ["127.0.0.1", "localhost", "[::1]"]
        .iter()
        .any(|name| host == format!("{name}:{port}"))
}

fn bearer(header: &str) -> Option<&str> {
    let (scheme, token) = header.split_once(' ')?;
    scheme.eq_ignore_ascii_case("bearer").then(|| token.trim())
}

/// Compares tokens in constant time: hashing gives equal-length inputs, and the fold does not stop at the first difference.
fn token_matches(given: &str, expected: &str) -> bool {
    use ring::digest::{digest, SHA256};
    let (a, b) = (
        digest(&SHA256, given.as_bytes()),
        digest(&SHA256, expected.as_bytes()),
    );
    let diff = a
        .as_ref()
        .iter()
        .zip(b.as_ref())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y));
    !expected.is_empty() && diff == 0
}

async fn read_body(
    stream: &mut TcpStream,
    rest: Vec<u8>,
    framing: Framing,
) -> Result<Vec<u8>, Response> {
    let mut reader = BodyReader {
        stream,
        buf: rest,
        pos: 0,
    };
    match framing {
        Framing::Length(n) => reader.take(n).await,
        Framing::Chunked => {
            let mut body = Vec::new();
            loop {
                let line = reader.line().await?;
                let size = line
                    .split(';')
                    .next()
                    .map(str::trim)
                    .and_then(|s| usize::from_str_radix(s, 16).ok());
                let size = size.ok_or_else(|| Response::refuse(400, "malformed chunked body"))?;
                if size == 0 {
                    // Trailer fields end with an empty line.
                    while !reader.line().await?.is_empty() {}
                    break;
                }
                if body.len().saturating_add(size) > MAX_BODY {
                    return Err(Response::refuse(413, "request body too large"));
                }
                body.extend(reader.take(size).await?);
                if !reader.line().await?.is_empty() {
                    return Err(Response::refuse(400, "malformed chunked body"));
                }
            }
            Ok(body)
        }
    }
}

/// Reads from bytes left over after the head, then from the stream.
struct BodyReader<'a> {
    stream: &'a mut TcpStream,
    buf: Vec<u8>,
    pos: usize,
}

impl BodyReader<'_> {
    async fn fill(&mut self) -> Result<(), Response> {
        let mut chunk = [0u8; 8192];
        let n = self
            .stream
            .read(&mut chunk)
            .await
            .map_err(|_| Response::refuse(400, "connection error"))?;
        if n == 0 {
            return Err(Response::refuse(400, "incomplete request body"));
        }
        self.buf.drain(..self.pos);
        self.pos = 0;
        self.buf.extend_from_slice(&chunk[..n]);
        Ok(())
    }

    async fn take(&mut self, n: usize) -> Result<Vec<u8>, Response> {
        while self.buf.len() - self.pos < n {
            self.fill().await?;
        }
        let out = self.buf[self.pos..self.pos + n].to_vec();
        self.pos += n;
        Ok(out)
    }

    /// One CRLF-terminated line without the terminator, at most a few KiB long.
    async fn line(&mut self) -> Result<String, Response> {
        loop {
            if let Some(i) = self.buf[self.pos..].windows(2).position(|w| w == b"\r\n") {
                let line = String::from_utf8_lossy(&self.buf[self.pos..self.pos + i]).into_owned();
                self.pos += i + 2;
                return Ok(line);
            }
            if self.buf.len() - self.pos > 4096 {
                return Err(Response::refuse(400, "malformed chunked body"));
            }
            self.fill().await?;
        }
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        411 => "Length Required",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        431 => "Request Header Fields Too Large",
        _ => "Error",
    }
}

async fn write_response(stream: &mut TcpStream, response: Response) {
    let body = response.body.unwrap_or_default();
    let mut head = format!(
        "HTTP/1.1 {} {}\r\n",
        response.status,
        reason(response.status)
    );
    if !body.is_empty() {
        head.push_str("Content-Type: application/json\r\n");
    }
    head.push_str(&format!(
        "Content-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n",
        body.len()
    ));
    for (name, value) in response.extra {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    let mut out = head.into_bytes();
    out.extend_from_slice(&body);
    if let Err(e) = stream.write_all(&out).await {
        log::debug!("mcp: response write failed: {e}");
    }
    let _ = stream.shutdown().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::protocol::{tool_json, CallContext};

    struct Host;

    impl ToolHost for Host {
        async fn call_tool(&self, name: &str, _arguments: Value, _ctx: CallContext) -> Value {
            tool_json(json!({"called": name}))
        }
    }

    impl ServerHost for Host {
        fn token(&self) -> String {
            "secret".into()
        }
    }

    async fn send(port: u16, raw: &str) -> (u16, String) {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
            .await
            .unwrap();
        stream.write_all(raw.as_bytes()).await.unwrap();
        let mut out = String::new();
        stream.read_to_string(&mut out).await.unwrap();
        let status = out
            .split(' ')
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let body = out
            .split_once("\r\n\r\n")
            .map(|(_, b)| b.to_string())
            .unwrap_or_default();
        (status, body)
    }

    fn post(port: u16, extra: &str, body: &str) -> String {
        format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer secret\r\nContent-Type: application/json\r\n{extra}Content-Length: {}\r\n\r\n{body}",
            body.len()
        )
    }

    const CALL: &str =
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_status"}}"#;

    #[tokio::test]
    async fn serves_authorized_requests_and_refuses_the_rest() {
        let server = start(0, Arc::new(Host)).await.unwrap();
        let port = server.port;

        let (status, body) = send(port, &post(port, "", CALL)).await;
        assert_eq!(status, 200);
        assert!(body.contains("\"called\":\"get_status\""), "{body}");

        let (status, body) = send(
            port,
            &post(
                port,
                "",
                r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            ),
        )
        .await;
        assert_eq!((status, body.as_str()), (202, ""));

        let wrong_token = post(port, "", CALL).replace("Bearer secret", "Bearer nope");
        assert_eq!(send(port, &wrong_token).await.0, 401);
        let no_token = post(port, "", CALL).replace("Authorization: Bearer secret\r\n", "");
        assert_eq!(send(port, &no_token).await.0, 401);
        assert_eq!(
            send(port, &post(port, "Origin: https://example.com\r\n", CALL))
                .await
                .0,
            403
        );
        let rebound = post(port, "", CALL).replace(
            &format!("Host: 127.0.0.1:{port}"),
            &format!("Host: evil.example:{port}"),
        );
        assert_eq!(send(port, &rebound).await.0, 403);
        let text = post(port, "", CALL).replace("application/json", "text/plain");
        assert_eq!(send(port, &text).await.0, 415);
        let get = format!(
            "GET /mcp HTTP/1.1\r\nHost: localhost:{port}\r\nAuthorization: Bearer secret\r\n\r\n"
        );
        assert_eq!(send(port, &get).await.0, 405);
        let other = post(port, "", CALL).replace("POST /mcp", "POST /other");
        assert_eq!(send(port, &other).await.0, 404);
        let no_length = format!("POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer secret\r\nContent-Type: application/json\r\n\r\n");
        assert_eq!(send(port, &no_length).await.0, 411);

        server.stop().await;
        assert!(TcpStream::connect((Ipv4Addr::LOCALHOST, port))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn reads_chunked_bodies() {
        let server = start(0, Arc::new(Host)).await.unwrap();
        let port = server.port;
        let (first, second) = CALL.split_at(20);
        let raw = format!(
            "POST /mcp HTTP/1.1\r\nHost: [::1]:{port}\r\nAuthorization: bearer secret\r\nContent-Type: application/json; charset=utf-8\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{first}\r\n{:x}\r\n{second}\r\n0\r\n\r\n",
            first.len(),
            second.len()
        );
        let (status, body) = send(port, &raw).await;
        assert_eq!(status, 200, "{body}");
        assert!(body.contains("get_status"));
        server.stop().await;
    }

    #[test]
    fn token_comparison() {
        assert!(token_matches("abc", "abc"));
        assert!(!token_matches("abd", "abc"));
        assert!(!token_matches("", ""));
        assert!(is_loopback_host("LOCALHOST:80", 80));
        assert!(!is_loopback_host("127.0.0.1", 80));
    }
}
