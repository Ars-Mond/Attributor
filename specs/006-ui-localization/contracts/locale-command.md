# Contract: `detect_os_locale` Tauri command

First-run OS-language detection. Pure Rust (`sys-locale`), invoked once when `general.language` has no saved
value. Conforms to Constitution IX (typed IPC, `Result<T, String>`, never panics, camelCase types).

## Signature (Rust)

```rust
#[tauri::command]
fn detect_os_locale() -> Result<String, String>;
```

- Returns `Ok(tag)` where `tag` is the OS locale as a BCP-47 string (e.g. `"ru-RU"`, `"en-US"`).
- If `sys_locale::get_locale()` returns `None` (locale unavailable), returns `Ok("en")` as a graceful
  default and logs at warn level (Constitution VI) — it MUST NOT return `Err` for the merely-absent case so
  the frontend always has a usable value.
- `Err(String)` is reserved for genuinely unexpected failures; the frontend treats any `Err` as "fall back
  to DEFAULT_LOCALE".
- No panics across the IPC boundary.
- No request payload; no struct fields, so `#[serde(rename_all = "camelCase")]` is vacuously satisfied.

## Registration

- Added to the `tauri::generate_handler![...]` list in `src-tauri/src/lib.rs`.

## Frontend usage

```ts
import {invoke} from '@tauri-apps/api/core';
const tag = await invoke<string>('detect_os_locale').catch(() => 'en');
const subtag = tag.toLowerCase().split(/[-_]/)[0];   // "ru-RU" -> "ru"
const loc: Locale = (LOCALES as string[]).includes(subtag) ? (subtag as Locale) : DEFAULT_LOCALE;
```

## Dependency

- `sys-locale = "0.3"` added to `src-tauri/Cargo.toml` `[dependencies]`. Pure Rust; zero transitive deps on
  the Windows/macOS/Linux desktop targets. Justified under Constitution I (pure Rust) and X (minimal deps);
  see research.md Decision 2.
