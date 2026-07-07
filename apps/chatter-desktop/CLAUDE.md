# CLAUDE.md, Chatter Desktop App

**Status:** Current
**Last updated:** 2026-07-06 17:27 EDT

## Overview

Native desktop validation app for CHAT files, built with Tauri v2 (Rust backend, React + TypeScript frontend). Designed for linguists and researchers who don't use a terminal.

## Functional Parity with TUI

**The desktop app must achieve full functional parity with the `chatter validate` TUI.** Every feature the TUI provides for displaying validation results must work equivalently in the desktop app. The TUI is the reference implementation.

**Current parity status: partial.** See the feature parity tables below for details.

### Error Display Parity (mandatory)

The TUI source is at `crates/chatter/src/ui/validation_tui/`. Every rendering behavior listed below must be matched:

All error rendering is handled by **miette on the Rust side**, through the
**single shared orchestration** `talkbank_transform::render_diagnostics()`. The
CLI (`chatter/src/output.rs`) and this desktop bridge
(`src-tauri/src/events.rs`) both call it: it enhances each error once
(`enhance_errors_with_source`) and renders the plain text (the "Copy" form) and,
under `RenderMode::Ansi`, the colored form. The desktop converts that ANSI to
HTML via the `ansi-to-html` crate and ships both forms in the `diagnostics` field
of each `Errors` event; the frontend displays the HTML in a `<pre>` block. Going
through one function means the GUI cannot diverge from the CLI on enhancement or
on which source line the caret lands at. The cross-surface guard is
`crates/talkbank-transform/tests/render_parity.rs`.

> History: before this consolidation the desktop maintained its own inline
> `clone -> enhance -> render` block and rendered carets at the **wrong source
> line** (an error on line 8 drew its caret at line 1), because the colored
> render path resolved the error's window-relative span against the full file.
> Both the consolidation and the root-cause fix landed together; do not
> reintroduce a desktop-local rendering block.

| Feature | Status |
|---------|--------|
| **Miette-style error rendering** | **Implemented**: server-side rendering via `render_error_with_miette_with_source()` + `ansi-to-html` |
| **Multi-line context, tab expansion, bullets, underlines, labels, suggestions** | **Implemented**: all handled by miette |
| **Colored output** | **Implemented**: ANSI colors converted to HTML `<span style="color:...">` |

### File List Parity (mandatory)

| Feature | TUI implementation | Desktop status |
|---------|-------------------|----------------|
| **Hide valid files** | TUI tracks `total_files_with_errors()` separately | **Implemented**: only error files shown; header shows "N files with errors / M total" |
| **Alphabetical sort** | Files sorted during validation (`state.files.sort_by`) | **Implemented**: `localeCompare` sort in `buildTree()` |
| **Recursive directory tree** | Full recursive traversal with indented display | **Implemented**: collapsible tree with pruned empty dirs |
| **Error count badges** | Per-file error count in file list | **Implemented** |

### Navigation Parity (mandatory)

| Feature | TUI keybinding | Desktop equivalent |
|---------|---------------|-------------------|
| Switch panes | Tab | Click (mouse-native UI, Tab not applicable) |
| Navigate files | j/k, ↑/↓ | Click |
| Navigate errors | j/k, ↑/↓ (in error pane) | Scroll |
| Open in CLAN | Enter / c | Button per error (shared `send2clan::open_location_in_clan`, identical request to the CLI) |
| Revalidate | Ctrl+R / Cmd+R | **Implemented**: keyboard shortcut + button |
| Cancel validation | Escape | **Implemented**: keyboard shortcut + button |
| Quit | q / Esc | Window close |

### Lifecycle Parity (mandatory)

| Feature | TUI implementation | Desktop status |
|---------|-------------------|----------------|
| **Progress throttling** | Configurable file-stride between redraws | Not yet implemented, React batches DOM updates via `requestAnimationFrame` which provides some natural throttling |
| **Streaming vs complete states** | Two distinct UI modes during/after validation | **Implemented**: ProgressBar shows different content per phase |
| **Progress header** | "Done \| X files with errors / Y files" + gauge | **Implemented**: tree header shows "N files with errors / M total"; gated on `phase === "finished"` via `shouldShowAllFilesValid`, never derived from a still-streaming partial file set (fixed 2026-07-06) |

### Validation Engine Parity (mandatory)

Fixed 2026-07-06 (a user report showed the desktop cache and file-count
message diverged from the CLI; a systemic audit found the single-file
validation path bypassed the shared engine entirely). Both single-file and
directory targets now route through the identical `talkbank-transform`
streaming entrypoints the CLI uses, with a real cache instance.

| Feature | CLI implementation | Desktop status |
|---------|--------------------|-----------------|
| **On-disk validation cache** | `Arc<UnifiedCache>` constructed via `UnifiedCache::new()`, passed to the streaming entrypoints | **Implemented**: same construction, same entrypoints, for both directory and single-file targets |
| **`@Media`-filename check (E531)** | Runs via the shared worker loop's file-stem dispatch | **Implemented** for single-file targets (previously skipped entirely) |
| **`--roundtrip` / `--parser re2c` / `--strict-linkers` / `--jobs`** | CLI flags map onto `ValidationConfig` fields | **Implemented**: a settings popover (`ValidationSettingsPanel`) sends the same fields through `ValidateRequest` |
| **Stats accounting (valid/invalid/cache-hit counts)** | Shared `ValidationStats` accumulator | **Implemented** for both targets (previously hand-rolled for single files) |

## Architecture

```
apps/chatter-desktop/
  src-tauri/            Rust backend (Tauri v2)
    src/
      protocol.rs       Shared command/event names + transport request types
      commands.rs       #[tauri::command] handlers (validate, cancel, export, open_in_clan)
      events.rs         ValidationEvent → FrontendEvent bridge (serde camelCase)
      lib.rs            Tauri entry point + plugin registration
    tests/
      validation_bridge.rs   Integration tests (no GUI needed)
  src/                  React + TypeScript frontend
    components/         DropZone, FileTree, ErrorPanel, ProgressBar
    hooks/              useValidation + validationState reducer
    protocol/           Centralized command/event names + TS transport mirrors
    runtime/            Tauri transport + capability-focused runtime seam
  tests/unit/           Focused seam tests (Node test runner + compiled TS)
  tests/e2e/            WebdriverIO smoke tests (Linux/Windows only)
  wdio.conf.ts          WebdriverIO config
```

### Key design decisions

- **Direct Rust linking**: both `validate_directory_streaming()` and `validate_files_streaming()` from `talkbank-transform` are called directly (the latter for single-file targets, mirroring the CLI's own dispatch), not shelling out to the CLI. Streaming events over crossbeam channels → Tauri emit.
- **Lock-free concurrency**: `ArcSwapOption` for the cancel sender, no mutex. See the [mutex policy](../book/src/architecture/concurrency.md).
- **Centralized protocol contracts**: Tauri command/event names and transport payload types live in `src-tauri/src/protocol.rs` and `src/protocol/desktopProtocol.ts`.
- **serde camelCase bridge**: Rust structs use snake_case with `#[serde(rename_all = "camelCase")]` so JSON matches TypeScript types. The Rust integration tests verify the serialized JSON shape. **Every enum variant with fields needs its own `#[serde(rename_all = "camelCase")]`**, not just the enum-level one (the enum-level attribute only renames the `type` tag, not field names): a missing per-variant attribute on `FrontendFileStatus::Valid` silently shipped `cache_hit` instead of `cacheHit` until caught by the 2026-07-06 cache regression test.
- **Single-target contract**: desktop validation accepts one `.cha` file or one folder at a time. Native drag/drop must use Tauri's webview drag-drop API, not browser file-name placeholders.
- **Capability-first runtime seam**: keep `@tauri-apps/*` imports inside `src/runtime/tauriTransport.ts`; components and hooks should depend on narrow capability hooks rather than a whole desktop service object.
- **No desktop-local domain logic; reuse the CLI's**: this covers the FULL validation pipeline, not just presentation. Error rendering goes through `talkbank_transform::render_diagnostics()` and Open-in-CLAN through `send2clan::open_location_in_clan()`, the exact functions the CLI uses. **Validation orchestration itself is the same shared functions too**: both single-file and directory targets route through `talkbank_transform::validation_runner::{validate_files_streaming, validate_directory_streaming}` with a real `Arc<UnifiedCache>` (constructed identically to the CLI's `initialize_validation_cache`, `crates/chatter/src/commands/validate/cache.rs`), never a bespoke single-file loop built on the bare `parse_and_validate_streaming` primitive. Before 2026-07-06, the single-file path took exactly that bespoke shortcut and silently diverged from the CLI/directory path on caching, the `@Media`-filename check (E531), `--roundtrip`/`--parser`/`--strict-linkers` reachability, and stats accounting; the desktop must not re-implement enhancement, miette rendering, CLAN-location resolution, cache lookups, config dispatch, or stats aggregation; doing so is how the GUI silently drifted from the CLI. `commands::resolve_open_in_clan()` is split out from the Apple-Event send so the Open-in-CLAN resolution (file read + `resolve_clan_location` + message) is testable without launching CLAN.

## Development

```bash
cd apps/chatter-desktop
npm install
cargo tauri dev           # Launch with hot reload (frontend + backend)
cargo tauri build         # Distributable app bundle (DMG/MSI/AppImage)
cargo tauri build --debug # Debug build for E2E testing
```

## Testing

Three tiers, see [Desktop App Testing](../book/src/contributing/desktop-testing.md) for full details.

```bash
# Tier 1: focused frontend/runtime seam tests
cd apps/chatter-desktop && npm run test:unit

# Tier 2: Rust integration tests (fast, run always)
cargo nextest run -p chatter-desktop --test validation_bridge

# Tier 3: E2E smoke tests (slow, Linux/Windows only)
tauri-driver &
npm run test:e2e
```

## App Identity

The official name is **Chatter**, not "chatter-desktop". The Cargo package name
is `chatter-desktop` to avoid conflicts with the CLI package (`chatter`
produces the `chatter` binary), but the user-visible name everywhere must be
"Chatter":

- `tauri.conf.json` → `productName: "Chatter"` (controls `.app` bundle name)
- Window title: "Chatter, CHAT Validation"
- macOS About dialog: "About Chatter" (requires running as `.app` bundle, not raw binary)
- `cargo tauri dev` runs the raw binary, so About shows "chatter-desktop"; this is expected in dev mode only

## CLI Bundling

The desktop app should bundle the `chatter` CLI binary so that power users who
download the GUI can also run the CLI from their terminal (like VS Code ships
the `code` command).

**Approach (VS Code-style):**

1. Build `chatter` alongside the desktop app (`cargo build --release -p chatter`)
2. Include it as a Tauri `resources` entry, bundled inside the `.app`
3. Add a menu item "Install CLI command" that symlinks the bundled binary
   to `/usr/local/bin/chatter` (macOS/Linux) or adds to PATH (Windows)
4. On macOS: `Chatter.app/Contents/Resources/chatter` → `/usr/local/bin/chatter`

This is a Phase 3 item, requires the build pipeline to produce both binaries.

## Release Hardening

Public macOS releases should ship as a signed and notarized `Chatter.app` with
bundle identifier `org.talkbank.chatter`. Follow the signing and notarization
playbook in `../../docs/strategy/distribution-and-signing.md`.

- This is a Rust/Tauri app, so JVM JIT entitlements do **not** apply here.
  Start with hardened runtime + timestamp and only add entitlements if a
  future Tauri capability requires them.
- Raw unsigned `.app` bundles are acceptable for local development only, not for
  end-user release artifacts.
- If the bundled-CLI plan lands, sign the nested `chatter` resource before
  sealing the `.app` and notarizing the outer DMG/zip. A signed app does not
  retroactively cover an unsigned separately shipped CLI artifact.

## Auto-update

The app auto-updates via `tauri-plugin-updater`. On launch it checks the
GitHub Releases `latest.json` (`plugins.updater.endpoints` in
`tauri.conf.json`), and on finding a newer version prompts the user and, on
acceptance, downloads, installs, and relaunches. The flow is a best-effort
launch-time check that never throws, so an offline or failed check leaves the
app running on the current version.

Wiring (do not bypass the runtime seam):

- **Frontend:** the `updates` capability
  (`src/runtime/capabilities/updates.ts`) owns the orchestration; the
  `@tauri-apps/plugin-updater` / `@tauri-apps/plugin-process` /
  `@tauri-apps/plugin-dialog` calls live in `src/runtime/tauriTransport.ts`
  (`checkForUpdate`, `askInstallUpdate`), per the capability-first seam rule.
  `App.tsx` calls `updates.checkOnLaunch()` once on mount. Orchestration is
  unit-tested in `tests/unit/updates.test.cjs` with a fake transport.
- **Backend:** `tauri_plugin_updater` + `tauri_plugin_process` are registered
  in `lib.rs`; permissions `updater:default` + `process:default` are in
  `capabilities/default.json`.
- **Signing:** the updater uses its OWN minisign keypair, separate from the
  Apple Developer ID certificate. `tauri.conf.json` carries the PUBLIC key
  (`plugins.updater.pubkey`); the PRIVATE key is supplied at build time via
  `TAURI_SIGNING_PRIVATE_KEY` (+ `_PASSWORD`). **`bundle.createUpdaterArtifacts`
  is `true`, so any `tauri build` (local or CI) requires those env vars; a
  keyless bundle build fails.** Never commit the private key.
- **Manifest:** `release-desktop.yml` builds + signs the per-platform updater
  bundles, then the `updater-manifest` job assembles `latest.json` via
  `scripts/generate-latest-json.mjs` (structure unit-tested in
  `tests/unit/latestJson.test.mjs`) and uploads it to the release.

Full design, key management, and succession: `../../docs/strategy/distribution-and-signing.md`
("Auto-update").

## Coding Standards

Follow the root `CLAUDE.md` for all Rust code. Additional rules for the desktop app:

- **No mutex**: use `ArcSwapOption`, atomics, or channels. See the mutex policy.
- **serde field names**: every enum variant with fields needs `#[serde(rename_all = "camelCase")]`. The enum-level `rename_all` only affects tag names, not field names.
- **TypeScript types must mirror Rust types**: when changing `events.rs`, update `types.ts` and run the integration tests to verify.
- **Reference the TUI source** when implementing display features, `crates/chatter/src/ui/validation_tui/` is the reference implementation for error rendering, file list behavior, and navigation.
