# Contract: Ollama local HTTP API (consumed by the backend)

The backend is the only client; it speaks plain HTTP to the local daemon (default `http://localhost:11434`,
overridable via `ollama.baseUrl`). Verified against the current (2026) Ollama API.

## Heartbeat / version

```
GET {base}/api/version   ->  200 { "version": "0.x.y" }
```
Short connect timeout (~1–2 s). Connection-refused/timeout ⇒ not reachable (the only availability signal —
no binary/filesystem probe). When not reachable, the settings page offers the scripted install (see
`install_ollama` in tauri-commands.md).

## List installed models

```
GET {base}/api/tags  ->  200 { "models": [ { "name": "...", "model": "...", "size": 0, "details": {…} }, … ] }
```
Use `name` as the model id. Empty `models` ⇒ nothing pulled (attribute stays disabled).

## Pull (download) a model — NDJSON stream

```
POST {base}/api/pull   { "model": "<name>", "stream": true }
->  one JSON object per line:
    { "status": "pulling manifest" }
    { "status": "pulling <digest>", "digest": "sha256:…", "total": <bytes>, "completed": <bytes> }   (repeated)
    { "status": "verifying sha256 digest" } { "status": "writing manifest" } { "status": "success" }
```
`completed` is absent until bytes flow; `total`/`completed` are **per layer (digest)** — compute % per the
current digest, treat missing as indeterminate. Read via `bytes_stream()`, split on `\n`,
`serde_json::from_str` each line, forward as `PullProgress`. On `success`, re-`GET /api/tags`.

## Single vision inference — non-streaming

```
POST {base}/api/generate
{
  "model": "<vision-model>",
  "prompt": "<profile prompt> … return as JSON matching the schema",
  "images": ["<RAW base64, no data: prefix>"],   // ONE image per request
  "stream": false,
  "format": { …the fixed JSON Schema… },          // structured output
  "options": { "num_ctx": 4096, "temperature": 0, … },
  "think": false,                                  // or "low"|"medium"|"high"|"max"; omit if unset
  "keep_alive": "5m"
}
->  200 {
      "response": "<the strict JSON as a STRING>",
      "done": true, "done_reason": "stop", "thinking": "…optional…", …
    }
```
The strict JSON is inside the `response` **string** — parse it again and validate against the schema in Rust
before applying (the model can drift even with `format`). Image bytes come from the file (optionally
downscaled/re-encoded via the `image` crate to shrink payload) then base64-encoded (standard engine).

## Notes

- Plain HTTP on `127.0.0.1:11434`; TLS not required (rustls included for robustness/pure-Rust).
- Streaming endpoints are **NDJSON** (line-delimited), not a JSON array.
- The daemon performs the registry download itself; our app only speaks to localhost — no registry
  credentials/TLS on our side.
- Errors to map to `Result<_, String>`: connection-refused (not running), 404 (model removed), 400 (bad
  schema/options), slow first token (cold load → generous read timeout).
