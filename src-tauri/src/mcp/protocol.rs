//! MCP message handling shared by the in-app HTTP server and the stdio bridge.
//! It speaks both protocol eras: the stateless 2026-07-28 revision (per-request `_meta`, `server/discover`) and the legacy `initialize` handshake of 2025-11-25 and 2025-06-18.
//! Only tools are offered, and the tool list never changes at run time, so no change notifications are needed.

use std::future::Future;

use base64::Engine;
use serde_json::{json, Map, Value};

use super::catalog;

/// The stateless protocol revision.
pub(crate) const MODERN_VERSION: &str = "2026-07-28";
/// Legacy revisions answered through `initialize`, newest first.
const LEGACY_VERSIONS: [&str; 2] = ["2025-11-25", "2025-06-18"];
/// Every revision this server speaks.
pub(crate) const SUPPORTED_VERSIONS: [&str; 3] =
    [MODERN_VERSION, LEGACY_VERSIONS[0], LEGACY_VERSIONS[1]];

pub(crate) const META_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
pub(crate) const META_CLIENT_INFO: &str = "io.modelcontextprotocol/clientInfo";
pub(crate) const META_CLIENT_CAPABILITIES: &str = "io.modelcontextprotocol/clientCapabilities";
const META_SERVER_INFO: &str = "io.modelcontextprotocol/serverInfo";

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const HEADER_MISMATCH: i64 = -32020;
const UNSUPPORTED_VERSION: i64 = -32022;

/// How long clients may cache `server/discover` and `tools/list`; the catalogue changes only with an app update.
const CACHE_TTL_MS: u64 = 3_600_000;

/// Guidance sent to clients in `initialize` and `server/discover`.
const INSTRUCTIONS: &str = "Attributor edits the stock-photo metadata (title, description, keywords, categories) of JPEG, PNG and WebP files. The Attributor app must be open on this computer. Use absolute file paths. A typical flow: open_folder or list_images, then get_metadata and get_preview, then set_metadata or attribute_images (which fills the fields with the AI model configured in Attributor), then save_metadata to write the metadata into the files. set_metadata and attribute_images keep their changes inside Attributor until save_metadata is called. save_metadata and attribute_images return a jobId when they take longer than waitSeconds; follow it with get_job. API keys and sign-ins are never available through MCP.";

/// MCP headers of an HTTP request; the stdio bridge has none.
#[derive(Default, Debug)]
pub(crate) struct HttpHeaders {
    pub protocol_version: Option<String>,
    pub method: Option<String>,
    pub name: Option<String>,
}

/// Who sent a tool call and which protocol era it speaks.
#[derive(Clone, Default, Debug)]
pub(crate) struct CallContext {
    pub client: Option<String>,
    pub modern: bool,
}

/// Runs tool calls: the app's dispatcher in the server, a forwarder to the app in the bridge.
pub(crate) trait ToolHost: Sync {
    /// Runs the catalogue tool `name` with `arguments` (always an object) and returns its `CallToolResult`.
    fn call_tool(
        &self,
        name: &str,
        arguments: Value,
        ctx: CallContext,
    ) -> impl Future<Output = Value> + Send;
}

/// Outcome of one message: the HTTP status and the JSON-RPC response; notifications and client responses get none.
#[derive(Debug, PartialEq)]
pub(crate) struct Reply {
    pub status: u16,
    pub body: Option<Value>,
}

impl Reply {
    fn accepted() -> Self {
        Self {
            status: 202,
            body: None,
        }
    }
}

/// A JSON-RPC error.
struct RpcError {
    code: i64,
    message: String,
    data: Option<Value>,
}

impl RpcError {
    fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(INVALID_PARAMS, message)
    }

    fn unsupported(requested: &str) -> Self {
        Self {
            code: UNSUPPORTED_VERSION,
            message: "Unsupported protocol version".into(),
            data: Some(json!({"supported": SUPPORTED_VERSIONS, "requested": requested})),
        }
    }

    /// HTTP status of the error response; protocol-level failures are 400, an unknown method of a modern request is 404.
    fn status(&self, modern: bool) -> u16 {
        match self.code {
            HEADER_MISMATCH | UNSUPPORTED_VERSION | INVALID_REQUEST | PARSE_ERROR => 400,
            METHOD_NOT_FOUND if modern => 404,
            _ => 200,
        }
    }

    fn reply(self, id: Value, modern: bool) -> Reply {
        let status = self.status(modern);
        let mut error = json!({"code": self.code, "message": self.message});
        if let Some(data) = self.data {
            error["data"] = data;
        }
        Reply {
            status,
            body: Some(json!({"jsonrpc": "2.0", "id": id, "error": error})),
        }
    }
}

/// The server's `Implementation` object.
pub(crate) fn server_info() -> Value {
    json!({"name": "attributor", "title": "Attributor", "version": env!("CARGO_PKG_VERSION")})
}

/// A successful tool result carrying `value` both as structured content and as its JSON text.
pub(crate) fn tool_json(value: Value) -> Value {
    json!({
        "content": [{"type": "text", "text": value.to_string()}],
        "structuredContent": value,
        "isError": false,
    })
}

/// A failed tool result; the model reads `message` and can correct its call.
pub(crate) fn tool_error(message: impl Into<String>) -> Value {
    json!({"content": [{"type": "text", "text": message.into()}], "isError": true})
}

/// Handles one raw JSON-RPC message.
pub(crate) async fn handle(host: &impl ToolHost, body: &[u8], http: Option<&HttpHeaders>) -> Reply {
    match serde_json::from_slice::<Value>(body) {
        Ok(msg) => handle_message(host, msg, http).await,
        Err(_) => RpcError::new(PARSE_ERROR, "Parse error").reply(Value::Null, false),
    }
}

/// Handles one parsed JSON-RPC message.
pub(crate) async fn handle_message(
    host: &impl ToolHost,
    msg: Value,
    http: Option<&HttpHeaders>,
) -> Reply {
    let Value::Object(mut obj) = msg else {
        let message = if msg.is_array() {
            "Batch requests are not supported"
        } else {
            "Invalid request"
        };
        return RpcError::new(INVALID_REQUEST, message).reply(Value::Null, false);
    };
    let id = obj.remove("id");
    if obj.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return RpcError::new(INVALID_REQUEST, "jsonrpc must be \"2.0\"")
            .reply(id.unwrap_or(Value::Null), false);
    }
    let Some(method) = obj.get("method").and_then(Value::as_str).map(str::to_owned) else {
        // A response to a server request: this server sends none, so it is acknowledged and dropped.
        if id.is_some() && (obj.contains_key("result") || obj.contains_key("error")) {
            return Reply::accepted();
        }
        return RpcError::new(INVALID_REQUEST, "Missing method")
            .reply(id.unwrap_or(Value::Null), false);
    };
    let Some(id) = id else {
        // Notifications (initialized, cancelled, ...) need no answer.
        return Reply::accepted();
    };
    if !(id.is_string() || id.is_i64() || id.is_u64()) {
        return RpcError::new(INVALID_REQUEST, "id must be a string or an integer")
            .reply(Value::Null, false);
    }
    let params = match obj.remove("params") {
        None | Some(Value::Null) => Map::new(),
        Some(Value::Object(p)) => p,
        Some(_) => return RpcError::invalid_params("params must be an object").reply(id, false),
    };

    let meta = params.get("_meta").and_then(Value::as_object);
    let meta_version = meta
        .and_then(|m| m.get(META_VERSION))
        .and_then(Value::as_str);
    let modern = meta_version.is_some() && method != "initialize";
    if let Some(version) = meta_version.filter(|_| modern) {
        if version != MODERN_VERSION {
            return RpcError::unsupported(version).reply(id, true);
        }
    }
    if let Some(headers) = http {
        if let Err(e) = check_headers(headers, modern, meta_version, &method, &params) {
            return e.reply(id, modern);
        }
    }

    let ctx = CallContext {
        client: client_name(meta, &params),
        modern,
    };
    let result = match method.as_str() {
        "initialize" => Ok(initialize(&params)),
        "ping" | "logging/setLevel" => Ok(json!({})),
        "server/discover" => Ok(discover()),
        "tools/list" => Ok(tools_list(modern)),
        "tools/call" => call_tool(host, &params, ctx).await,
        "resources/list" => Ok(json!({"resources": []})),
        "resources/templates/list" => Ok(json!({"resourceTemplates": []})),
        "prompts/list" => Ok(json!({"prompts": []})),
        _ => Err(RpcError::new(
            METHOD_NOT_FOUND,
            format!("Method not found: {method}"),
        )),
    };
    match result {
        Ok(mut value) => {
            if modern {
                decorate(&mut value);
            }
            Reply {
                status: 200,
                body: Some(json!({"jsonrpc": "2.0", "id": id, "result": value})),
            }
        }
        Err(e) => e.reply(id, modern),
    }
}

/// HTTP header rules: a modern request's headers must match its body (missing ones are tolerated); a legacy request's version header must name a supported revision.
fn check_headers(
    headers: &HttpHeaders,
    modern: bool,
    meta_version: Option<&str>,
    method: &str,
    params: &Map<String, Value>,
) -> Result<(), RpcError> {
    if !modern {
        return match headers.protocol_version.as_deref() {
            Some(v) if !SUPPORTED_VERSIONS.contains(&v) => Err(RpcError::unsupported(v)),
            _ => Ok(()),
        };
    }
    let mismatch = |header: &str| {
        RpcError::new(
            HEADER_MISMATCH,
            format!("The {header} header does not match the request body"),
        )
    };
    if headers
        .protocol_version
        .as_deref()
        .is_some_and(|v| Some(v) != meta_version)
    {
        return Err(mismatch("MCP-Protocol-Version"));
    }
    if headers.method.as_deref().is_some_and(|m| m != method) {
        return Err(mismatch("Mcp-Method"));
    }
    if matches!(method, "tools/call" | "resources/read" | "prompts/get") {
        if let Some(name) = headers.name.as_deref() {
            let expected = params
                .get("name")
                .or_else(|| params.get("uri"))
                .and_then(Value::as_str);
            if decode_header_value(name).as_deref() != expected {
                return Err(mismatch("Mcp-Name"));
            }
        }
    }
    Ok(())
}

/// Decodes the `=?base64?…?=` form MCP uses for header values that are not plain ASCII.
pub(crate) fn decode_header_value(value: &str) -> Option<String> {
    match value
        .strip_prefix("=?base64?")
        .and_then(|v| v.strip_suffix("?="))
    {
        Some(encoded) => base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok()),
        None => Some(value.to_string()),
    }
}

/// The client's name, from the modern `_meta` or the legacy `initialize` parameters.
fn client_name(meta: Option<&Map<String, Value>>, params: &Map<String, Value>) -> Option<String> {
    meta.and_then(|m| m.get(META_CLIENT_INFO))
        .or_else(|| params.get("clientInfo"))
        .and_then(|info| info.get("title").or_else(|| info.get("name")))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// Legacy handshake: echo the client's revision when supported, otherwise offer the newest legacy one.
fn initialize(params: &Map<String, Value>) -> Value {
    let requested = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let version = LEGACY_VERSIONS
        .iter()
        .find(|v| **v == requested)
        .copied()
        .unwrap_or(LEGACY_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": {"tools": {}},
        "serverInfo": server_info(),
        "instructions": INSTRUCTIONS,
    })
}

fn discover() -> Value {
    json!({
        "supportedVersions": SUPPORTED_VERSIONS,
        "capabilities": {"tools": {}},
        "instructions": INSTRUCTIONS,
        "ttlMs": CACHE_TTL_MS,
        "cacheScope": "public",
    })
}

fn tools_list(modern: bool) -> Value {
    let mut result = json!({"tools": catalog::catalogue()});
    if modern {
        result["ttlMs"] = json!(CACHE_TTL_MS);
        result["cacheScope"] = json!("public");
    }
    result
}

async fn call_tool(
    host: &impl ToolHost,
    params: &Map<String, Value>,
    ctx: CallContext,
) -> Result<Value, RpcError> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::invalid_params("tools/call needs a tool name"))?;
    if !catalog::is_known(name) {
        return Err(RpcError::invalid_params(format!("Unknown tool: {name}")));
    }
    let arguments = match params.get("arguments") {
        None | Some(Value::Null) => json!({}),
        Some(a @ Value::Object(_)) => a.clone(),
        Some(_) => return Err(RpcError::invalid_params("arguments must be an object")),
    };
    Ok(host.call_tool(name, arguments, ctx).await)
}

/// Adds the fields every modern result carries.
fn decorate(result: &mut Value) {
    let Some(obj) = result.as_object_mut() else {
        return;
    };
    obj.entry("resultType").or_insert_with(|| json!("complete"));
    let meta = obj.entry("_meta").or_insert_with(|| json!({}));
    if let Some(meta) = meta.as_object_mut() {
        meta.entry(META_SERVER_INFO).or_insert_with(server_info);
    }
}

/// Removes the modern-only result fields before a result goes to a legacy client.
pub(crate) fn strip_modern_fields(result: &mut Value) {
    if let Some(obj) = result.as_object_mut() {
        obj.remove("resultType");
        obj.remove("_meta");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Echoes the call so tests can see what reached the host.
    struct Echo;

    impl ToolHost for Echo {
        async fn call_tool(&self, name: &str, arguments: Value, ctx: CallContext) -> Value {
            tool_json(
                json!({"tool": name, "arguments": arguments, "client": ctx.client, "modern": ctx.modern}),
            )
        }
    }

    fn modern_meta() -> Value {
        json!({
            META_VERSION: MODERN_VERSION,
            META_CLIENT_CAPABILITIES: {},
            META_CLIENT_INFO: {"name": "test-client", "version": "1"},
        })
    }

    async fn run(msg: Value, http: Option<&HttpHeaders>) -> Reply {
        handle_message(&Echo, msg, http).await
    }

    #[tokio::test]
    async fn legacy_initialize_echoes_supported_versions_and_falls_back() {
        let r = run(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "c", "version": "1"}}}), None).await;
        let result = &r.body.unwrap()["result"];
        assert_eq!(result["protocolVersion"], "2025-06-18");
        assert_eq!(result["serverInfo"]["name"], "attributor");
        assert!(result.get("resultType").is_none());

        let r = run(json!({"jsonrpc": "2.0", "id": 2, "method": "initialize", "params": {"protocolVersion": "2024-11-05"}}), None).await;
        assert_eq!(r.body.unwrap()["result"]["protocolVersion"], "2025-11-25");
    }

    #[tokio::test]
    async fn modern_discover_and_list_carry_cache_fields() {
        let r = run(json!({"jsonrpc": "2.0", "id": "a", "method": "server/discover", "params": {"_meta": modern_meta()}}), None).await;
        let result = r.body.unwrap()["result"].clone();
        assert_eq!(result["resultType"], "complete");
        assert_eq!(result["supportedVersions"][0], MODERN_VERSION);
        assert_eq!(result["_meta"][META_SERVER_INFO]["name"], "attributor");

        let r = run(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list", "params": {"_meta": modern_meta()}}), None).await;
        let result = r.body.unwrap()["result"].clone();
        assert_eq!(result["cacheScope"], "public");
        assert!(result["ttlMs"].is_u64());
        assert!(!result["tools"].as_array().unwrap().is_empty());

        let r = run(
            json!({"jsonrpc": "2.0", "id": 4, "method": "tools/list"}),
            None,
        )
        .await;
        let result = r.body.unwrap()["result"].clone();
        assert!(result.get("ttlMs").is_none() && result.get("resultType").is_none());
    }

    #[tokio::test]
    async fn unsupported_modern_version_is_rejected_with_the_supported_list() {
        let mut meta = modern_meta();
        meta[META_VERSION] = json!("2099-01-01");
        let r = run(
            json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {"_meta": meta}}),
            None,
        )
        .await;
        assert_eq!(r.status, 400);
        let error = &r.body.unwrap()["error"];
        assert_eq!(error["code"], UNSUPPORTED_VERSION);
        assert_eq!(error["data"]["requested"], "2099-01-01");
    }

    #[tokio::test]
    async fn tool_calls_reach_the_host_with_context() {
        let r = run(json!({"jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": {"name": "get_status", "arguments": {"x": 1}, "_meta": modern_meta()}}), None).await;
        assert_eq!(r.status, 200);
        let result = r.body.unwrap()["result"].clone();
        assert_eq!(result["structuredContent"]["tool"], "get_status");
        assert_eq!(result["structuredContent"]["client"], "test-client");
        assert_eq!(result["structuredContent"]["modern"], true);
        assert_eq!(result["resultType"], "complete");

        let r = run(
            json!({"jsonrpc": "2.0", "id": 8, "method": "tools/call", "params": {"name": "nope"}}),
            None,
        )
        .await;
        assert_eq!(r.body.unwrap()["error"]["code"], INVALID_PARAMS);
        let r = run(json!({"jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": {"name": "get_status", "arguments": [1]}}), None).await;
        assert_eq!(r.body.unwrap()["error"]["code"], INVALID_PARAMS);
    }

    #[tokio::test]
    async fn notifications_responses_and_batches() {
        assert_eq!(
            run(
                json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
                None
            )
            .await,
            Reply::accepted()
        );
        assert_eq!(
            run(json!({"jsonrpc": "2.0", "id": 5, "result": {}}), None).await,
            Reply::accepted()
        );
        let r = run(json!([{"jsonrpc": "2.0", "id": 1, "method": "ping"}]), None).await;
        assert_eq!(r.status, 400);
        assert_eq!(r.body.unwrap()["error"]["code"], INVALID_REQUEST);
        let r = handle(&Echo, b"{oops", None).await;
        assert_eq!(r.body.unwrap()["error"]["code"], PARSE_ERROR);
        let r = run(
            json!({"jsonrpc": "2.0", "id": null, "method": "ping"}),
            None,
        )
        .await;
        assert_eq!(r.body.unwrap()["error"]["code"], INVALID_REQUEST);
    }

    #[tokio::test]
    async fn unknown_methods_are_404_for_modern_http_requests() {
        let r = run(json!({"jsonrpc": "2.0", "id": 1, "method": "nope", "params": {"_meta": modern_meta()}}), None).await;
        assert_eq!(r.status, 404);
        let r = run(json!({"jsonrpc": "2.0", "id": 1, "method": "nope"}), None).await;
        assert_eq!(r.status, 200);
        assert_eq!(r.body.unwrap()["error"]["code"], METHOD_NOT_FOUND);
    }

    #[tokio::test]
    async fn modern_headers_must_match_the_body() {
        let msg = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "get_status", "_meta": modern_meta()}});
        let ok = HttpHeaders {
            protocol_version: Some(MODERN_VERSION.into()),
            method: Some("tools/call".into()),
            name: Some("=?base64?Z2V0X3N0YXR1cw==?=".into()),
        };
        assert_eq!(run(msg.clone(), Some(&ok)).await.status, 200);

        for bad in [
            HttpHeaders {
                method: Some("tools/list".into()),
                ..Default::default()
            },
            HttpHeaders {
                name: Some("list_images".into()),
                ..Default::default()
            },
            HttpHeaders {
                protocol_version: Some("2025-11-25".into()),
                ..Default::default()
            },
        ] {
            let r = run(msg.clone(), Some(&bad)).await;
            assert_eq!(r.status, 400, "{bad:?}");
            assert_eq!(r.body.unwrap()["error"]["code"], HEADER_MISMATCH);
        }
        // Missing headers are tolerated.
        assert_eq!(run(msg, Some(&HttpHeaders::default())).await.status, 200);
    }

    #[tokio::test]
    async fn legacy_requests_need_a_supported_version_header() {
        let msg = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});
        let good = HttpHeaders {
            protocol_version: Some("2025-11-25".into()),
            ..Default::default()
        };
        assert_eq!(run(msg.clone(), Some(&good)).await.status, 200);
        let bad = HttpHeaders {
            protocol_version: Some("1999-01-01".into()),
            ..Default::default()
        };
        assert_eq!(run(msg, Some(&bad)).await.status, 400);
    }

    #[test]
    fn strips_modern_fields() {
        let mut result = json!({"content": [], "resultType": "complete", "_meta": {}});
        strip_modern_fields(&mut result);
        assert_eq!(result, json!({"content": []}));
    }
}
