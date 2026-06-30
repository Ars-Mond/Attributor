<!--
SYNC IMPACT REPORT
==================
Version change: 1.0.0 -> 1.1.0
Bump rationale (1.1.0, MINOR): Principle VIII amended — batch photo processing in the
app's logic uses `rayon`, but other concurrent workloads (e.g. background thumbnail
generation) MAY use an equivalent thread pool (producer–consumer over standard threads).
Clarifies `rayon` is not the sole permitted concurrency tool. No template changes required.
Prior — 1.0.0: Initial ratification of the project constitution. All template
placeholders replaced with concrete, project-specific principles.

Principles (11, defined from maintainer input):
  I.   Pure Rust Backend
  II.  Modern Svelte 5 (Runes)
  III. Themed SCSS Tokens
  IV.  Cross-Platform Parity
  V.   Reuse UI Primitives
  VI.  Mandatory Logging
  VII. Phase-Based Commits
  VIII.Rust Performance First
  IX.  Typed Tauri IPC
  X.   Fixed Stack
  XI.  Code Style

Added sections:
  - Communication & Documentation
  - Development Workflow

Removed sections: none (template placeholders filled in place).

Templates / artifacts reviewed for consistency:
  - .specify/templates/plan-template.md ........ ✅ aligned (Constitution Check
        gate reads from this file dynamically; no hardcoded principles to update)
  - .specify/templates/spec-template.md ........ ✅ no change required
  - .specify/templates/tasks-template.md ....... ✅ no change required
  - .specify/templates/commands/*.md ........... ✅ none present / no stale refs

Follow-up TODOs: none.
-->

# Attributor Constitution

Attributor is a cross-platform desktop application (Tauri 2 + SvelteKit/Svelte 5)
for reading, editing, and writing XMP/EXIF metadata of stock photos
(JPEG/PNG/WebP) prior to submission to photo-stock agencies.

## Core Principles

### I. Pure Rust Backend

The backend MUST be written in pure, safe Rust. No native/system libraries, no
C/C++ dependencies, and no FFI bindings are permitted. Crates that link against or
wrap non-Rust libraries MUST NOT be added; only pure-Rust implementations are
allowed. `unsafe` introduced solely to bridge non-Rust code is forbidden.

Rationale: A pure-Rust dependency graph keeps builds hermetic and reproducible
across every target OS, eliminates per-platform toolchain breakage, and preserves
memory safety end to end.

### II. Modern Svelte 5 (Runes)

Frontend state and reactivity MUST use Svelte 5 runes exclusively:
`$state`, `$derived`, `$effect`, `$props`, `$bindable`. Legacy patterns are
forbidden: no legacy stores (`writable`/`readable`) for component state, no
`export let` props, no `$:` reactive statements.

Rationale: A single modern reactivity model keeps components consistent,
prevents legacy/runes interop bugs, and aligns with the framework's supported path.

### III. Themed SCSS Tokens

All colors, spacing, and typography MUST be sourced from tokens defined in
`_mixins.scss` / `_themes.scss`. Hardcoded hex colors, raw spacing values, and
ad-hoc font sizes are prohibited in component styles.

Rationale: Centralized tokens make theming and dark/light parity trivial,
guarantee visual consistency, and prevent drift that hardcoded values cause.

### IV. Cross-Platform Parity

Windows, Linux, and macOS MUST behave identically. `#[cfg(target_os)]` is
permitted ONLY to smooth over platform differences behind uniform behavior, and
MUST NOT be used to gate a feature on or off per operating system.

Rationale: Users on any OS get the same product; platform-specific feature gating
fragments behavior and creates untested code paths.

### V. Reuse UI Primitives

Existing UI primitives (e.g. `DockLayout`, `MenuBar`, `ConfirmDialog`, `FileTree`)
MUST be reused. Creating a parallel component that duplicates the role of an
existing primitive is prohibited; extend or generalize the existing primitive
instead.

Rationale: One canonical component per role avoids divergent behavior, reduces
maintenance surface, and keeps the UI coherent.

### VI. Mandatory Logging

Every site where an error can occur MUST log through the project logger, and code
SHOULD log at other points where it aids diagnosis. Rust uses the `log` crate
routed to `tauri-plugin-log`; the frontend uses `@tauri-apps/plugin-log`.
`println!`, `dbg!`, and `console.*` are forbidden outside tests. Log messages MUST
be concise and written in English.

Rationale: Consistent, centralized logging is the only reliable way to diagnose
issues in a packaged desktop app where stdout is not observed by users.

### VII. Phase-Based Commits

After completing EACH Spec Kit phase (specify, clarify, plan, tasks, implement) a
git commit via `/speckit-git-commit` is MANDATORY before moving to the next phase.
One commit equals one phase; changes from different phases MUST NOT be mixed.
Commit messages MUST be in English and concise — phase plus touched artifacts. The
commit description MUST NOT contain copyright notices.

Rationale: Phase-aligned commits keep the spec-driven history auditable and make
each phase independently reviewable and revertible.

### VIII. Rust Performance First

All heavy work — parsing, decoding, directory scanning, and batch processing —
MUST run in Rust, never in TypeScript. IPC MUST NOT be invoked inside hot loops;
batch the work and cross the boundary once. Batch photo processing within the app's
logic uses `rayon`; other concurrent workloads (e.g. background thumbnail generation)
MAY instead use an equivalent thread pool, such as a producer–consumer pool over
standard threads.

Rationale: Rust delivers the throughput these workloads need; chatty IPC and JS-side
computation become the bottleneck and stall the UI.

### IX. Typed Tauri IPC

Tauri commands MUST accept and return `serde`-serializable types, MUST return
`Result<T, String>` and never panic across the IPC boundary, and MUST apply
`#[serde(rename_all = "camelCase")]` on the types crossing that boundary.

Rationale: Typed, non-panicking, camelCase-consistent commands give the frontend a
stable contract and surface backend failures as handleable errors rather than crashes.

### X. Fixed Stack

The technology stack is already defined: Tauri 2, SvelteKit + Svelte 5 +
TypeScript + SCSS, and the designated Rust metadata crates (including the
`little_exif` fork). Any new dependency MUST be justified in the plan before it is
adopted.

Rationale: A deliberate, small dependency set keeps the project auditable and
pure-Rust-compliant; unjustified additions erode both.

### XI. Code Style

Comments, identifiers, and code-level documentation MUST be in English. In JS/TS,
braces MUST NOT contain inner spaces (`{x}`, not `{ x }`). Struct fields, object
keys, and assignments MUST NOT be aligned with padding spaces.

Rationale: A uniform, tooling-friendly style minimizes diff noise and keeps the
codebase readable to every contributor.

## Communication & Documentation

- Conversation with the maintainer is conducted in Russian.
- All code, comments, identifiers, and Spec Kit artifacts are written in English.
- Project/application documentation (everything that is NOT a Spec Kit artifact)
  MUST be maintained in both English and Russian: English is the source of truth
  and every change is mirrored into the Russian version.

## Development Workflow

- After editing any frontend file (`.svelte`, `.ts`, `.svelte.ts`), run
  `npx svelte-check --tsconfig ./tsconfig.json` and resolve all reported issues
  before the change is considered complete.
- Python is an auxiliary tool only and is never used for target application
  features. When Python is needed, invoke it via `uv run python`, or
  `uv run --with <package> python` when a library is required.
- Each Spec Kit phase ends with a passing build/check and the phase commit required
  by Principle VII; an unfinished check or missing commit blocks the next phase.

## Governance

- This constitution supersedes all other practices and conventions. Where guidance
  conflicts, the constitution wins.
- Amendments require a documented rationale, a version bump per the policy below,
  and propagation of any affected rules into the dependent templates under
  `.specify/templates/`.
- Versioning policy (semantic):
  - MAJOR — backward-incompatible governance or principle removal/redefinition.
  - MINOR — a new principle/section is added or guidance is materially expanded.
  - PATCH — clarifications, wording, and non-semantic refinements.
- Compliance: every plan's Constitution Check and every code review MUST verify
  adherence. Any violation MUST be justified in the plan's Complexity Tracking
  table or remediated before merge.

**Version**: 1.1.0 | **Ratified**: 2026-06-18 | **Last Amended**: 2026-06-22
