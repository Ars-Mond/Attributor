//! Writes Attributor's MCP entry into the configuration files of the supported clients.
//! Claude Desktop starts the stdio bridge (`attributor --mcp`), since its configuration file takes only local commands; Claude Code and Codex (whose `config.toml` the ChatGPT desktop app shares) connect to the HTTP endpoint with the token.
//! Files are merged, never rewritten from scratch: other servers, settings and key order stay, the previous file is kept as `<name>.attributor-backup`, and an unreadable file is left untouched.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use toml_edit::{DocumentMut, InlineTable, Item, Table};

use super::config::{home_dir, non_empty_var, write_atomic};

/// Name of the server entry in every client file.
pub(crate) const SERVER_NAME: &str = "attributor";
/// Argument that starts the stdio bridge.
pub(crate) const BRIDGE_ARG: &str = "--mcp";
/// Tool timeout written for Codex: AI attribution of one image can exceed its 60-second default.
const CODEX_TOOL_TIMEOUT_SECS: i64 = 300;

/// A supported MCP client.
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Client {
    ClaudeDesktop,
    ClaudeCode,
    /// Codex CLI, its IDE extension and the ChatGPT desktop app, which share one configuration file.
    Codex,
}

impl Client {
    pub(crate) const ALL: [Client; 3] = [Client::ClaudeDesktop, Client::ClaudeCode, Client::Codex];

    /// Whether the client connects over HTTP, so its entry carries the URL and the token.
    pub(crate) fn uses_http(self) -> bool {
        self != Client::ClaudeDesktop
    }
}

/// What a client entry points at.
pub(crate) struct Endpoint {
    pub url: String,
    pub token: String,
    /// The executable Claude Desktop starts with [`BRIDGE_ARG`].
    pub command: String,
}

impl Endpoint {
    fn bearer(&self) -> String {
        format!("Bearer {}", self.token)
    }
}

/// A client's state as the MCP settings page shows it.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClientStatus {
    pub client: Client,
    /// The client seems to be installed (its configuration directory exists).
    pub detected: bool,
    /// Some configuration file has an Attributor entry.
    pub configured: bool,
    /// Every configuration file has an entry matching the current endpoint.
    pub up_to_date: bool,
    pub files: Vec<String>,
    pub error: Option<String>,
}

/// Whether a file's entry is missing, outdated or current.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Entry {
    Absent,
    Outdated,
    Current,
}

/// The configuration files of `client` that exist or would be created, and whether the client was detected.
fn files(client: Client) -> (Vec<PathBuf>, bool) {
    match client {
        Client::ClaudeDesktop => {
            let dirs: Vec<PathBuf> = claude_desktop_dirs()
                .into_iter()
                .filter(|d| d.is_dir())
                .collect();
            let detected = !dirs.is_empty();
            let dirs = if detected {
                dirs
            } else {
                claude_desktop_dirs().into_iter().take(1).collect()
            };
            (
                dirs.into_iter()
                    .map(|d| d.join("claude_desktop_config.json"))
                    .collect(),
                detected,
            )
        }
        Client::ClaudeCode => {
            // Claude Code's own rule: a legacy `.config.json` in its config directory wins; otherwise `.claude.json` in CLAUDE_CONFIG_DIR or the home directory.
            let custom = non_empty_var("CLAUDE_CONFIG_DIR");
            let config_dir = custom
                .clone()
                .or_else(|| home_dir().map(|home| home.join(".claude")));
            let legacy = config_dir
                .as_ref()
                .map(|dir| dir.join(".config.json"))
                .filter(|f| f.is_file());
            let file =
                legacy.or_else(|| custom.or_else(home_dir).map(|dir| dir.join(".claude.json")));
            let detected = file.as_ref().is_some_and(|f| f.is_file())
                || config_dir.is_some_and(|d| d.is_dir());
            (file.into_iter().collect(), detected)
        }
        Client::Codex => {
            let dir =
                non_empty_var("CODEX_HOME").or_else(|| home_dir().map(|home| home.join(".codex")));
            let detected = dir.as_ref().is_some_and(|d| d.is_dir());
            (
                dir.map(|d| d.join("config.toml")).into_iter().collect(),
                detected,
            )
        }
    }
}

/// Directories where Claude Desktop may keep its configuration, the default one first.
/// Installs from the Microsoft Store run virtualized and read a copy under `%LOCALAPPDATA%`, and newer builds keep their data under `Claude-Data`; the entry goes into every existing one.
fn claude_desktop_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if cfg!(windows) {
        if let Some(roaming) = non_empty_var("APPDATA") {
            dirs.push(roaming.join("Claude"));
        }
        if let Some(local) = non_empty_var("LOCALAPPDATA") {
            dirs.push(
                local
                    .join("Packages")
                    .join("Claude_pzs8sxrjxfjjc")
                    .join("LocalCache")
                    .join("Roaming")
                    .join("Claude"),
            );
            if let Ok(entries) = std::fs::read_dir(local.join("Claude-Data")) {
                let mut found: Vec<PathBuf> = entries
                    .flatten()
                    .filter(|e| e.file_name().to_string_lossy().starts_with("claude_"))
                    .map(|e| e.path())
                    .collect();
                found.sort();
                dirs.extend(found);
            }
        }
    } else if cfg!(target_os = "macos") {
        if let Some(home) = home_dir() {
            dirs.push(
                home.join("Library")
                    .join("Application Support")
                    .join("Claude"),
            );
        }
    } else if let Some(config) = non_empty_var("XDG_CONFIG_HOME")
        .filter(|d| d.is_absolute())
        .or_else(|| home_dir().map(|h| h.join(".config")))
    {
        dirs.push(config.join("Claude"));
    }
    dirs
}

/// The client's current state.
pub(crate) fn status(client: Client, endpoint: &Endpoint) -> ClientStatus {
    let (paths, detected) = files(client);
    let mut states = Vec::new();
    let mut error = None;
    for path in &paths {
        match read(path).and_then(|text| entry_state(client, &text, endpoint)) {
            Ok(state) => states.push(state),
            Err(e) => error = Some(format!("{}: {e}", path.display())),
        }
    }
    ClientStatus {
        client,
        detected,
        configured: states.iter().any(|s| *s != Entry::Absent),
        up_to_date: !states.is_empty()
            && error.is_none()
            && states.iter().all(|s| *s == Entry::Current),
        files: paths.iter().map(|p| p.display().to_string()).collect(),
        error,
    }
}

/// Adds or updates the Attributor entry in every configuration file of `client`.
///
/// # Errors
///
/// Returns the first file that cannot be parsed or written; files are checked before any is written.
pub(crate) fn install(client: Client, endpoint: &Endpoint) -> Result<ClientStatus, String> {
    let (paths, _) = files(client);
    if paths.is_empty() {
        return Err("cannot find the home directory".into());
    }
    let mut updates = Vec::new();
    for path in &paths {
        let text = read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let updated =
            with_entry(client, &text, endpoint).map_err(|e| format!("{}: {e}", path.display()))?;
        updates.push((path, text, updated));
    }
    for (path, old, new) in updates {
        if old != new {
            write(path, &old, &new)?;
            log::info!("mcp: wrote the {client:?} entry to {}", path.display());
        }
    }
    Ok(status(client, endpoint))
}

/// Removes the Attributor entry from every configuration file of `client`.
///
/// # Errors
///
/// Returns the first file that cannot be parsed or written.
pub(crate) fn uninstall(client: Client, endpoint: &Endpoint) -> Result<ClientStatus, String> {
    let (paths, _) = files(client);
    for path in &paths {
        if !path.is_file() {
            continue;
        }
        let text = read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if let Some(new) =
            without_entry(client, &text).map_err(|e| format!("{}: {e}", path.display()))?
        {
            write(path, &text, &new)?;
            log::info!("mcp: removed the {client:?} entry from {}", path.display());
        }
    }
    Ok(status(client, endpoint))
}

/// Rewrites the entries of HTTP clients that are configured but point at an old port or token.
pub(crate) fn refresh_http_clients(endpoint: &Endpoint) {
    for client in Client::ALL.into_iter().filter(|c| c.uses_http()) {
        let current = status(client, endpoint);
        if current.configured && !current.up_to_date {
            if let Err(e) = install(client, endpoint) {
                log::warn!("mcp: cannot update the {client:?} entry: {e}");
            }
        }
    }
}

/// The configuration to add by hand: the JSON entry, the `claude` command or the TOML table.
pub(crate) fn snippet(client: Client, endpoint: &Endpoint) -> String {
    match client {
        Client::ClaudeDesktop => {
            let value = json!({"mcpServers": {SERVER_NAME: desktop_entry(endpoint)}});
            serde_json::to_string_pretty(&value).unwrap_or_default()
        }
        Client::ClaudeCode => format!(
            "claude mcp add --transport http --scope user {SERVER_NAME} {} --header \"Authorization: {}\"",
            endpoint.url,
            endpoint.bearer()
        ),
        Client::Codex => toml_with_entry("", endpoint).unwrap_or_default(),
    }
}

/// The file's text, or an empty string when it does not exist.
fn read(path: &Path) -> Result<String, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.to_string()),
    }
}

/// Keeps the previous content as a backup, then replaces the file atomically.
/// A symlinked file is written at its target, so the link stays. The files hold tokens and other servers' secrets, so on Unix a new file or backup is readable by the current user only, and an existing file keeps its permissions.
fn write(path: &Path, old: &str, new: &str) -> Result<(), String> {
    let is_link = std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink());
    let target = if is_link {
        std::fs::canonicalize(path)
            .map_err(|e| format!("cannot follow the link {}: {e}", path.display()))?
    } else {
        path.to_path_buf()
    };
    if !old.is_empty() {
        let mut backup = target.as_os_str().to_owned();
        backup.push(".attributor-backup");
        let backup = PathBuf::from(backup);
        // A fresh backup gets the private mode instead of the one a previous backup had.
        let _ = std::fs::remove_file(&backup);
        write_atomic(&backup, old.as_bytes(), true)
            .map_err(|e| format!("cannot back up {}: {e}", path.display()))?;
    }
    write_atomic(&target, new.as_bytes(), true)
        .map_err(|e| format!("cannot write {}: {e}", path.display()))
}

fn entry_state(client: Client, text: &str, endpoint: &Endpoint) -> Result<Entry, String> {
    if client == Client::Codex {
        return toml_entry_state(text, endpoint);
    }
    let Some(entry) = json_entry(text)? else {
        return Ok(Entry::Absent);
    };
    let current = match client {
        Client::ClaudeDesktop => {
            entry.get("command").and_then(Value::as_str) == Some(endpoint.command.as_str())
                && entry.get("args") == Some(&json!([BRIDGE_ARG]))
        }
        _ => {
            entry
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|t| t == "http" || t == "streamable-http")
                && entry.get("url").and_then(Value::as_str) == Some(endpoint.url.as_str())
                && entry
                    .pointer("/headers/Authorization")
                    .and_then(Value::as_str)
                    == Some(endpoint.bearer().as_str())
        }
    };
    Ok(if current {
        Entry::Current
    } else {
        Entry::Outdated
    })
}

fn with_entry(client: Client, text: &str, endpoint: &Endpoint) -> Result<String, String> {
    match client {
        Client::ClaudeDesktop => json_with_entry(text, |entry| {
            entry.insert("command".into(), json!(endpoint.command));
            entry.insert("args".into(), json!([BRIDGE_ARG]));
            // Keys of an HTTP entry make Claude Desktop reject the whole file.
            for stale in ["type", "url", "headers"] {
                entry.shift_remove(stale);
            }
        }),
        Client::ClaudeCode => json_with_entry(text, |entry| {
            for stale in ["command", "args", "env"] {
                entry.shift_remove(stale);
            }
            entry.insert("type".into(), json!("http"));
            entry.insert("url".into(), json!(endpoint.url));
            let headers = entry.entry("headers").or_insert_with(|| json!({}));
            if !headers.is_object() {
                *headers = json!({});
            }
            if let Some(headers) = headers.as_object_mut() {
                headers.insert("Authorization".into(), json!(endpoint.bearer()));
            }
        }),
        Client::Codex => toml_with_entry(text, endpoint),
    }
}

fn without_entry(client: Client, text: &str) -> Result<Option<String>, String> {
    if client == Client::Codex {
        toml_without_entry(text)
    } else {
        json_without_entry(text)
    }
}

/// Description of the endpoint a Claude Desktop entry starts.
fn desktop_entry(endpoint: &Endpoint) -> Value {
    json!({"command": endpoint.command, "args": [BRIDGE_ARG]})
}

/// Parses a JSON configuration file; an empty file is an empty object, and a byte-order mark is ignored.
fn parse_json(text: &str) -> Result<Map<String, Value>, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if text.trim().is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_str(text) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(_) => Err("the file is not a JSON object".into()),
        Err(e) => Err(format!("invalid JSON ({e}); fix or remove the file first")),
    }
}

fn json_entry(text: &str) -> Result<Option<Value>, String> {
    Ok(parse_json(text)?
        .get("mcpServers")
        .and_then(|s| s.get(SERVER_NAME))
        .cloned())
}

/// The file with the entry created or updated in place by `update`, keeping every other key.
fn json_with_entry(
    text: &str,
    update: impl FnOnce(&mut Map<String, Value>),
) -> Result<String, String> {
    let mut root = parse_json(text)?;
    let servers = root.entry("mcpServers").or_insert_with(|| json!({}));
    let servers = servers
        .as_object_mut()
        .ok_or("mcpServers is not an object")?;
    let entry = servers.entry(SERVER_NAME).or_insert_with(|| json!({}));
    if !entry.is_object() {
        *entry = json!({});
    }
    if let Some(entry) = entry.as_object_mut() {
        update(entry);
    }
    Ok(pretty(&Value::Object(root)))
}

fn json_without_entry(text: &str) -> Result<Option<String>, String> {
    let mut root = parse_json(text)?;
    let removed = root
        .get_mut("mcpServers")
        .and_then(Value::as_object_mut)
        .and_then(|servers| servers.shift_remove(SERVER_NAME))
        .is_some();
    Ok(removed.then(|| pretty(&Value::Object(root))))
}

fn pretty(value: &Value) -> String {
    let mut text = serde_json::to_string_pretty(value).unwrap_or_default();
    text.push('\n');
    text
}

fn parse_toml(text: &str) -> Result<DocumentMut, String> {
    text.strip_prefix('\u{feff}')
        .unwrap_or(text)
        .parse::<DocumentMut>()
        .map_err(|e| format!("invalid TOML ({e}); fix the file first"))
}

fn toml_entry_state(text: &str, endpoint: &Endpoint) -> Result<Entry, String> {
    let doc = parse_toml(text)?;
    let Some(entry) = doc
        .get("mcp_servers")
        .and_then(Item::as_table_like)
        .and_then(|s| s.get(SERVER_NAME))
        .and_then(Item::as_table_like)
    else {
        return Ok(Entry::Absent);
    };
    let url = entry.get("url").and_then(Item::as_str);
    let auth = entry
        .get("http_headers")
        .and_then(Item::as_table_like)
        .and_then(|h| h.get("Authorization"))
        .and_then(Item::as_str);
    let current = url == Some(endpoint.url.as_str())
        && auth == Some(endpoint.bearer().as_str())
        && entry.get("command").is_none();
    Ok(if current {
        Entry::Current
    } else {
        Entry::Outdated
    })
}

/// The file with `[mcp_servers.attributor]` created or updated in place; keys the user added (such as approval modes) stay.
fn toml_with_entry(text: &str, endpoint: &Endpoint) -> Result<String, String> {
    let mut doc = parse_toml(text)?;
    if !doc.contains_key("mcp_servers") {
        let mut servers = Table::new();
        servers.set_implicit(true);
        doc.insert("mcp_servers", Item::Table(servers));
    }
    let servers = doc
        .get_mut("mcp_servers")
        .and_then(Item::as_table_like_mut)
        .ok_or("mcp_servers is not a table")?;
    if servers
        .get(SERVER_NAME)
        .and_then(Item::as_table_like)
        .is_none()
    {
        servers.insert(SERVER_NAME, Item::Table(Table::new()));
    }
    let entry = servers
        .get_mut(SERVER_NAME)
        .and_then(Item::as_table_like_mut)
        .ok_or("mcp_servers.attributor is not a table")?;
    // A local-command entry would compete with the URL.
    for stale in ["command", "args", "env", "cwd", "bearer_token_env_var"] {
        entry.remove(stale);
    }
    entry.insert("url", toml_edit::value(endpoint.url.as_str()));
    match entry
        .get_mut("http_headers")
        .and_then(Item::as_table_like_mut)
    {
        Some(headers) => {
            headers.insert("Authorization", toml_edit::value(endpoint.bearer()));
        }
        None => {
            let mut headers = InlineTable::new();
            headers.insert("Authorization", endpoint.bearer().into());
            entry.insert("http_headers", toml_edit::value(headers));
        }
    }
    if !entry.contains_key("tool_timeout_sec") {
        entry.insert(
            "tool_timeout_sec",
            toml_edit::value(CODEX_TOOL_TIMEOUT_SECS),
        );
    }
    Ok(doc.to_string())
}

fn toml_without_entry(text: &str) -> Result<Option<String>, String> {
    let mut doc = parse_toml(text)?;
    let removed = doc
        .get_mut("mcp_servers")
        .and_then(Item::as_table_like_mut)
        .and_then(|servers| servers.remove(SERVER_NAME))
        .is_some();
    if !removed {
        return Ok(None);
    }
    if doc
        .get("mcp_servers")
        .and_then(Item::as_table)
        .is_some_and(|t| t.is_empty() && t.is_implicit())
    {
        doc.remove("mcp_servers");
    }
    Ok(Some(doc.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint() -> Endpoint {
        Endpoint {
            url: "http://127.0.0.1:47821/mcp".into(),
            token: "tok".into(),
            command: r"C:\Program Files\Attributor\attributor.exe".into(),
        }
    }

    #[test]
    fn claude_desktop_merge_keeps_preferences_and_order() {
        let original = "\u{feff}{\n  \"preferences\": {\"z\": 1, \"a\": 2},\n  \"mcpServers\": {\"other\": {\"command\": \"x\"}}\n}";
        let ep = endpoint();
        assert_eq!(
            entry_state(Client::ClaudeDesktop, original, &ep).unwrap(),
            Entry::Absent
        );
        let merged = with_entry(Client::ClaudeDesktop, original, &ep).unwrap();
        let value: Value = serde_json::from_str(&merged).unwrap();
        assert_eq!(value["preferences"], json!({"z": 1, "a": 2}));
        assert_eq!(value["mcpServers"]["other"]["command"], "x");
        assert_eq!(
            value["mcpServers"]["attributor"],
            json!({"command": ep.command, "args": ["--mcp"]})
        );
        // Key order is kept: preferences stays first, and its keys keep their order.
        assert!(merged.find("preferences").unwrap() < merged.find("mcpServers").unwrap());
        assert!(merged.find("\"z\"").unwrap() < merged.find("\"a\"").unwrap());
        assert_eq!(
            entry_state(Client::ClaudeDesktop, &merged, &ep).unwrap(),
            Entry::Current
        );

        let removed = json_without_entry(&merged).unwrap().unwrap();
        let value: Value = serde_json::from_str(&removed).unwrap();
        assert!(value["mcpServers"].get("attributor").is_none());
        assert_eq!(value["mcpServers"]["other"]["command"], "x");
        assert_eq!(json_without_entry(&removed).unwrap(), None);
    }

    #[test]
    fn claude_code_entry_uses_http_and_keeps_extra_keys() {
        let original = r#"{"numStartups": 3, "mcpServers": {"attributor": {"command": "old", "timeout": 5000, "headers": {"X-Other": "1"}}}}"#;
        let ep = endpoint();
        assert_eq!(
            entry_state(Client::ClaudeCode, original, &ep).unwrap(),
            Entry::Outdated
        );
        let merged = with_entry(Client::ClaudeCode, original, &ep).unwrap();
        let value: Value = serde_json::from_str(&merged).unwrap();
        let entry = &value["mcpServers"]["attributor"];
        assert_eq!(entry["type"], "http");
        assert_eq!(entry["url"], ep.url);
        assert_eq!(entry["headers"]["Authorization"], "Bearer tok");
        assert_eq!(entry["headers"]["X-Other"], "1");
        assert_eq!(entry["timeout"], 5000);
        assert!(entry.get("command").is_none());
        assert_eq!(value["numStartups"], 3);
        assert_eq!(
            entry_state(Client::ClaudeCode, &merged, &ep).unwrap(),
            Entry::Current
        );
        let other = Endpoint {
            token: "new".into(),
            ..endpoint()
        };
        assert_eq!(
            entry_state(Client::ClaudeCode, &merged, &other).unwrap(),
            Entry::Outdated
        );
    }

    #[test]
    fn invalid_json_is_never_overwritten() {
        assert!(with_entry(Client::ClaudeCode, "{broken", &endpoint()).is_err());
        assert!(with_entry(Client::ClaudeCode, "[1]", &endpoint()).is_err());
        assert!(with_entry(Client::ClaudeCode, "", &endpoint()).is_ok());
    }

    #[test]
    fn codex_toml_merge_keeps_comments_and_other_servers() {
        let original = "# my settings\nmodel = \"gpt-6\"\n\n[mcp_servers.other]\ncommand = 'C:\\tools\\x.exe'\n";
        let ep = endpoint();
        assert_eq!(toml_entry_state(original, &ep).unwrap(), Entry::Absent);
        let merged = toml_with_entry(original, &ep).unwrap();
        assert!(
            merged.starts_with("# my settings\nmodel = \"gpt-6\""),
            "{merged}"
        );
        assert!(merged.contains("command = 'C:\\tools\\x.exe'"), "{merged}");
        let doc: DocumentMut = merged.parse().unwrap();
        assert_eq!(
            doc["mcp_servers"]["attributor"]["url"].as_str(),
            Some(ep.url.as_str())
        );
        assert_eq!(
            doc["mcp_servers"]["attributor"]["http_headers"]["Authorization"].as_str(),
            Some("Bearer tok")
        );
        assert_eq!(
            doc["mcp_servers"]["attributor"]["tool_timeout_sec"].as_integer(),
            Some(300)
        );
        assert_eq!(toml_entry_state(&merged, &ep).unwrap(), Entry::Current);

        let removed = toml_without_entry(&merged).unwrap().unwrap();
        assert!(!removed.contains("attributor"));
        assert!(removed.contains("[mcp_servers.other]"));
        assert_eq!(toml_without_entry(&removed).unwrap(), None);
    }

    #[test]
    fn codex_update_keeps_user_keys_and_drops_the_command() {
        let original = "[mcp_servers.attributor]\ncommand = \"x\"\ntool_timeout_sec = 900\ndefault_tools_approval_mode = \"writes\"\n";
        let merged = toml_with_entry(original, &endpoint()).unwrap();
        let doc: DocumentMut = merged.parse().unwrap();
        let entry = &doc["mcp_servers"]["attributor"];
        assert!(entry.get("command").is_none());
        assert_eq!(entry["tool_timeout_sec"].as_integer(), Some(900));
        assert_eq!(
            entry["default_tools_approval_mode"].as_str(),
            Some("writes")
        );
        // A fresh file gets no empty [mcp_servers] header, and removing the entry leaves nothing behind.
        let fresh = toml_with_entry("", &endpoint()).unwrap();
        assert!(fresh.starts_with("[mcp_servers.attributor]"), "{fresh}");
        assert_eq!(toml_without_entry(&fresh).unwrap().unwrap().trim(), "");
        assert!(toml_with_entry("mcp_servers = 1", &endpoint()).is_err());
        assert!(toml_with_entry("[broken", &endpoint()).is_err());
    }

    #[test]
    fn inline_mcp_servers_tables_are_supported() {
        let original = "mcp_servers = { other = { command = \"x\" } }\n";
        let merged = toml_with_entry(original, &endpoint()).unwrap();
        let doc: DocumentMut = merged.parse().unwrap();
        assert_eq!(doc["mcp_servers"]["other"]["command"].as_str(), Some("x"));
        assert_eq!(
            doc["mcp_servers"]["attributor"]["url"].as_str(),
            Some(endpoint().url.as_str())
        );
    }

    #[test]
    fn snippets() {
        let ep = endpoint();
        assert!(snippet(Client::ClaudeDesktop, &ep).contains("\"--mcp\""));
        assert!(snippet(Client::ClaudeCode, &ep).starts_with(
            "claude mcp add --transport http --scope user attributor http://127.0.0.1:47821/mcp"
        ));
        assert!(snippet(Client::Codex, &ep).contains("[mcp_servers.attributor]"));
    }

    #[test]
    fn install_and_uninstall_write_backups() {
        let dir =
            std::env::temp_dir().join(format!("attributor-mcp-install-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, "{\"mcpServers\": {}}").unwrap();
        let ep = endpoint();
        let text = read(&path).unwrap();
        let new = with_entry(Client::ClaudeCode, &text, &ep).unwrap();
        write(&path, &text, &new).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("config.json.attributor-backup")).unwrap(),
            "{\"mcpServers\": {}}"
        );
        assert_eq!(
            entry_state(Client::ClaudeCode, &read(&path).unwrap(), &ep).unwrap(),
            Entry::Current
        );
        assert_eq!(read(&dir.join("missing.json")).unwrap(), "");
    }

    #[test]
    fn http_clients_round_trip_through_their_config_dirs() {
        // Only these two variables are read here, and no other test touches them.
        let dir =
            std::env::temp_dir().join(format!("attributor-mcp-clients-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("codex")).unwrap();
        std::fs::write(dir.join("codex").join("config.toml"), "model = \"gpt-6\"\n").unwrap();
        std::env::set_var("CODEX_HOME", dir.join("codex"));
        std::env::set_var("CLAUDE_CONFIG_DIR", dir.join("claude"));
        let ep = endpoint();
        for client in [Client::ClaudeCode, Client::Codex] {
            let before = status(client, &ep);
            assert!(!before.configured, "{client:?}");
            let installed = install(client, &ep).unwrap();
            assert!(
                installed.configured && installed.up_to_date,
                "{client:?}: {installed:?}"
            );
            let moved = Endpoint {
                url: "http://127.0.0.1:50000/mcp".into(),
                ..endpoint()
            };
            assert!(!status(client, &moved).up_to_date);
            refresh_http_clients(&moved);
            assert!(status(client, &moved).up_to_date, "{client:?}");
            let removed = uninstall(client, &moved).unwrap();
            assert!(!removed.configured, "{client:?}");
        }
        let codex = std::fs::read_to_string(dir.join("codex").join("config.toml")).unwrap();
        assert_eq!(codex.trim(), "model = \"gpt-6\"");
        assert!(dir
            .join("codex")
            .join("config.toml.attributor-backup")
            .is_file());
    }
}
