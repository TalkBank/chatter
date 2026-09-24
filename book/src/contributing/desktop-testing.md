# Desktop App Testing

**Status:** Current
**Last updated:** 2026-09-24 00:21 EDT

This document covers the testing strategy for the Chatter desktop app
(`apps/chatter-desktop/`). Testing is split into three tiers by speed and scope.

## Testing Tiers

```text
┌─────────────────────────────────────────────────────────┐
│  Tier 3: E2E (WebdriverIO + tauri-driver)               │
│  Real app, real DOM, real IPC. Slow (~5-10s/test).       │
│  Catches: rendering bugs, IPC wiring, platform quirks.   │
│  Run: manually before releases, optionally in CI.        │
├─────────────────────────────────────────────────────────┤
│  Tier 2: Rust integration tests                          │
│  Real validation pipeline, real event bridge, no GUI.    │
│  Catches: serialization mismatches, event ordering,      │
│  stats consistency, single-file handling.                 │
│  Run: every commit, CI required.                         │
├─────────────────────────────────────────────────────────┤
│  Tier 1: Unit tests (Rust + TypeScript)                  │
│  Pure functions and thin runtime seams in isolation.     │
│  Catches: protocol drift, reducer bugs, CLAN math.       │
│  Run: every commit, CI required.                         │
└─────────────────────────────────────────────────────────┘
```

Tier 2 exercises the shared validation runner and event bridge, not the entire
native app. Command-context tests must enter `tauri::async_runtime::block_on`:
a plain-thread call cannot detect nested-runtime failures at the Tauri boundary.
Use isolated cache directories, never the real user cache. Passing these tests
does not establish that native menus, dialogs, drag/drop, signing or installation
work; those require their respective runtime or release checks.

## Release-facing behavioral contracts

The desktop links the same validation engine as the CLI. Keep CHAT semantics in
the shared Rust implementation; use canonical specs/reference files for desktop
boundary tests rather than inventing a second fixture oracle.

### File outcomes and completion

`validationState.ts::fileOutcome` projects a streamed `FileEntry` into a
discriminated union: pending, valid, or problem with an explanation. Both the
file tree and the detail panel use it. In particular, `readError`, `parseError`,
`roundtripFailed` and cached invalid statuses must remain visible when there is
no diagnostic array. Do not infer success from an empty array.

`shouldShowAllFilesValid` additionally requires a nonempty, non-cancelled,
finished population whose totals certify all files as valid, with no read/parse
or roundtrip failures. `finishedRunSummary` is shared by the window title,
notification and status bar. Regression tests exercise failure statuses without
diagnostics, warning visibility, cancellation, empty populations and success.
When adding a new wire status, update the exhaustive projection, not a second
component-local classification.

### Update lifecycle

`updates.ts` owns an idle/checking state. A checking state carries the single
in-flight result and feedback mode. Launch, six-hour background and manual menu
requests join that operation; a manual join promotes it to visible feedback.
There can be only one prompt/install sequence for overlapping requests. Completion
or failure returns the capability to idle so a later retry can start.

The update seam tests cover accepted/declined installs, no update, network
failure, overlapping requests, a manual join, retry, and native error-dialog
failure. The latter must not reject a fire-and-forget menu callback. These tests
use transport doubles: they do not verify a real signature, actual installation,
or relaunch. Installer and updater artifacts require separate release evidence.

### Stored Unicode identity

The runtime bridge test copies the canonical W109 media specimen into an owned
temporary directory, then validates it through stored NFD, NFC alias (where the
filesystem supports it), and directory targets from inside Tauri's runtime.
It requires the same both-side W109, no E531 mismatch, and nonempty HTML/plain
renderings. Production identity comes from the shared stored-name resolver;
the desktop must not independently normalize paths or downgrade failed name
resolution to anonymous validation.

### Export admission and failure preservation

The async command tests exercise text export with each file status, including
failures without diagnostic cards and pending results. Text export deserializes
typed file records and the same status enum used by the event bridge; it no
longer navigates arbitrary JSON with question-mark fallbacks. Required paths
and rendered diagnostic text must be present. An unknown status or malformed
record refuses before filesystem writing, preserving an existing report.

Status and diagnostics are separate facts: a roundtrip failure can coexist with
diagnostic text, and neither may suppress the other. Tests preserve rendered
text verbatim. JSON export retains its existing wire representation. Neither
per-file export is a whole-run coverage certificate; cancellation/session metadata
is not presently included, as documented in the user guide.

### Before calling a desktop candidate ready

Run the focused frontend seam tests, compile the production frontend, and run
the Rust bridge tests on the final sources. Inspect the actual app on supported
platforms: open a canonical valid file, a diagnostic fixture and a failing target;
verify cancellation/revalidation, copy/export, menus and update feedback. Do not
call the app ready solely because TypeScript compiled or the headless bridge
passed. Use the coordinated release process for signed installers and updater
manifests, with versions tied to the exact release commit.

## Tier 1 & 2: Unit and integration tests

### Running

```bash
# TypeScript capability/seam tests
cd apps/chatter-desktop && npm run test:unit

# Rust contract/integration tests
cargo test -p chatter-desktop --test validation_bridge
```

### What they cover

| Test | What it verifies |
|------|-----------------|
| `apps/chatter-desktop/tests/unit/validationRunner.test.cjs` | Validation capability uses centralized command names, subscribes before invoke, and disposes listeners exactly once |
| `apps/chatter-desktop/tests/unit/validationState.test.cjs` | Validation reducer computes relative file names and merges diagnostics/status immutably |
| `reference_corpus_no_hard_errors` | every file under `corpus/reference/` produces zero `Severity::Error` (warnings allowed) |
| `event_lifecycle_has_correct_sequence` | Discovering → Started → FileComplete×N → Finished ordering |
| `frontend_events_serialize_to_expected_json_shape` | Every event has `type` field; camelCase field names match TypeScript types; diagnostics include `renderedText` |
| `protocol_contracts_serialize_to_expected_json_shape` | Rust command/event constants and request payloads stay aligned with the TypeScript protocol module |
| `single_file_validation` | Single-file path validates exactly the selected file |
| `finished_stats_match_file_events` | `valid + invalid + parseErrors == totalFiles`; FileComplete count matches |
| `rendered_html_present_for_errors` | Every diagnostic carries non-empty miette HTML with box-drawing characters and `style=` attributes (ANSI colors converted to HTML) |

### Adding new tests

Test file: `apps/chatter-desktop/src-tauri/tests/validation_bridge.rs`

The tests use `collect_events()` which runs the real validation pipeline and
collects all `FrontendEvent` values. To test a specific scenario:

```rust
#[test]
fn my_scenario() {
    let target = workspace_root().join("path/to/corpus");
    let events = collect_events(&target);
    let summary = summarize(&events);
    // assert on summary fields or individual events
}
```

### Miette rendering pipeline

Error rendering is server-side. Each `FrontendDiagnostic` carries two
renderings:

- **`rendered_html`**: `render_error_with_miette_with_source_colored()` produces
  ANSI-colored text, `ansi-to-html` converts it to HTML `<span style="...">`.
  The frontend displays it in a `<pre>` block via `dangerouslySetInnerHTML`.
  This guarantees identical output to the CLI.
- **`rendered_text`**: `render_error_with_miette_with_source()` produces plain
  text (no ANSI codes) for clean clipboard copy-paste.

The `rendered_html_present_for_errors` integration test verifies that every
error diagnostic includes non-empty HTML containing miette box-drawing
characters and `style=` attributes from ANSI color conversion.

### TypeScript seam tests

The TypeScript unit tests compile a focused subset of `apps/chatter-desktop/src/` to a
temporary CommonJS directory, then run Node's built-in test runner against the
compiled output. This keeps the test toolchain small while still exercising the
runtime seam as real JavaScript.

- Runner script: `apps/chatter-desktop/scripts/run-unit-tests.mjs`
- Compile config: `apps/chatter-desktop/tsconfig.unit.json`
- Test files: `apps/chatter-desktop/tests/unit/*.test.cjs`

### TypeScript ↔ Rust contract

The Rust integration tests verify that serialized JSON matches what the
TypeScript frontend expects. If you change a field name or event structure in
`events.rs`, the `frontend_events_serialize_to_expected_json_shape` test will
catch the mismatch before you discover it at runtime.

The key serde attributes:

- `#[serde(tag = "type", rename_all = "camelCase")]` on enums, variant names
  become camelCase tag values (`fileComplete`, not `FileComplete`)
- `#[serde(rename_all = "camelCase")]` on individual variants, field names
  become camelCase (`totalFiles`, not `total_files`)
- Both must be present: the enum-level `rename_all` only affects tag names,
  not field names within variants

## Tier 3: E2E Tests (WebdriverIO)

### Prerequisites

```bash
cargo install tauri-driver    # WebDriver backend for Tauri (Linux/Windows only)
cargo tauri build --debug     # Build the app binary
```

**Note:** `tauri-driver` only works on Linux and Windows. On macOS, WKWebView
does not support WebDriver. Run E2E tests in CI (Linux) or on a Windows machine.

### Running

```bash
# Terminal 1: start tauri-driver (WebDriver server on :4444)
tauri-driver

# Terminal 2: run the tests
cd apps/chatter-desktop
npm run test:e2e
```

### What they cover

The smoke tests in `tests/e2e/smoke.spec.ts` verify that the app launches and
renders the expected UI elements:

- Drop zone with Choose File / Choose Folder buttons
- Empty file tree ("No files loaded")
- Empty error panel ("Select a file to view errors")
- Status bar showing "Ready"

### Limitations

**File dialogs cannot be driven via WebDriver.** The native file picker
(`@tauri-apps/plugin-dialog`) opens an OS-level dialog that WebDriver can't
interact with. Options for testing the validation flow:

1. **Test-only Tauri command**: add `validate_for_test(path)` behind
   `#[cfg(debug_assertions)]` that bypasses the file dialog
2. **Programmatic invoke**: use `driver.executeScript()` to call
    `window.__TAURI__.core.invoke("validate", { path })` directly
3. **Drag-and-drop simulation**: possible but platform-dependent and fragile

For now, the Rust integration tests cover the full validation pipeline. E2E
tests focus on UI rendering and user-visible layout.

### Adding E2E tests

Test file: `apps/chatter-desktop/tests/e2e/*.spec.ts`

WebdriverIO provides `$()` and `$$()` for CSS selectors, plus Tauri-aware
capabilities:

```typescript
it("should show validation results", async () => {
  // Programmatically trigger validation (bypasses file dialog)
    await browser.executeAsync(async (path, done) => {
      await (window as any).__TAURI__.core.invoke("validate", {
        path,
      });
      // Wait for finished event
      setTimeout(done, 5000);
  }, "/path/to/corpus");

  const tree = await $(".file-tree-panel");
  const text = await tree.getText();
  expect(text).not.toContain("No files loaded");
});
```

### When to run E2E tests

- **Before releases**: manual run to verify the built app works end-to-end
- **Optionally in CI**: requires `tauri-driver` and a display server (Xvfb on
  Linux). Slow, so consider running only on release branches.
- **Not on every commit**: the Rust integration tests are fast and cover more
  ground

## Platform-Specific Considerations

| Platform | WebView engine | E2E support |
|----------|---------------|-------------|
| macOS | WKWebView | **Not supported**: `tauri-driver` does not work on macOS (WKWebView has no WebDriver API) |
| Windows | WebView2 (Chromium) | Full support via `tauri-driver` |
| Linux | WebKitGTK | Full support via `tauri-driver`; requires Xvfb for headless |

**macOS limitation:** Apple's WKWebView does not expose a WebDriver endpoint,
so `tauri-driver` cannot drive the app on macOS. E2E tests must run on Linux
(CI) or Windows. For local macOS development, rely on the Rust integration
tests (Tier 2) and manual smoke testing.

CSS rendering differs slightly between WebKit (Linux) and Chromium (Windows).
Visual regressions are possible, consider screenshot comparison tests if this
becomes a problem.

## Test Data

All tests use the reference corpus at `corpus/reference/`. This
corpus is checked into the repo and must always pass validation with
zero hard errors (warnings are allowed). The exact set of files and
the current warning-emitting files are whatever
`find corpus/reference -name '*.cha' -type f` and the validator
report, do not hard-code those lists here.

Do not create ad-hoc `.cha` test files. Use existing reference corpus files
or ask the user to provide test data.

## CI Integration

Add to the existing CI workflow:

```yaml
# Rust integration tests (fast, always run)
- name: Desktop integration tests
  run: cargo test -p chatter-desktop --test validation_bridge

# E2E tests (slow, release branches only)
- name: Build desktop app
  if: startsWith(github.ref, 'refs/heads/release')
  run: cargo tauri build --debug
- name: E2E smoke tests
  if: startsWith(github.ref, 'refs/heads/release')
  run: |
    tauri-driver &
    sleep 2
    cd apps/chatter-desktop && npm run test:e2e
```
