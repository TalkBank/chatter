# Desktop App Testing

**Status:** Current
**Last updated:** {{git-dates:page}}

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
file tree and the detail panel use it, and `fileStatusLabel` words it for
the detail panel and the text export. In particular, `readError`,
`internalFailure`, `roundtripFailed` and cached invalid statuses must remain
visible when there is no diagnostic array. Do not infer success from an empty
array.

`shouldShowAllFilesValid` requires a `finished` phase whose `passed` is
true (the runner's own verdict, the one the CLI's exit status reads) and no
file with anything to show. A cancelled run is the separate `stopped` phase,
and a target with no transcript is `nothingFound`, so neither can reach this
check. An internal failure increments `internalFailures`, not the valid or
invalid counter: CHAT validity remains undetermined.
`finishedRunSummary` is shared by the window title,
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

Reveal-in-file-manager travels through the validation-target capability and the
typed command protocol, not a component-local Tauri import. Transport-double
tests retain the exact Unicode path and propagate native failures. They do not
launch or certify the operating system's file manager.

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

### Candidate evidence matrix

An implemented test is not a passing receipt, and a headless receipt is not
native interaction evidence. Record candidate identity, platform, actual result
and remaining gaps in the release record; do not copy historical success into
a new candidate's acceptance.

| Workflow | Existing automated boundary | Required native observation |
|---|---|---|
| Valid and invalid files | Reference corpus and canonical spec bridge tests | Select each; verify visible verdict, diagnostic navigation and completion |
| Unreadable or unsupported target | Typed outcome projection and path-contract tests | Surface failure visibly; do not show an all-valid result |
| Directory selection | Nested discovery and event/statistics bridge tests | Select a directory; inspect relative paths and final totals |
| Unicode paths | Stored-identity W109 runtime bridge test | Open an accented path through the native chooser and inspect media warnings |
| Cancel and revalidate | Runner lifecycle and cancelled-state seam tests | Cancel an active run, change input, revalidate; no stale result may claim success |
| Copy and export | Typed export admission and refusal tests | Check clipboard and saved output, including failure without diagnostic cards |
| Menus and updates | Single-flight update seam tests | Exercise menu feedback; test signed installation/update separately |
| Settings preservation | No complete native proof supplied by bridge tests | Verify preferences survive relaunch and upgrade without touching unrelated settings |

Declare platform support and installation/update acceptance explicitly. A
configured CI target or available installer is not evidence that every workflow
above has been exercised on that platform.

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
| `finished_stats_match_file_events` | Typed valid, invalid, parse-error and internal-failure counts account for the population; FileComplete count matches |
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

**Note:** the checked-in configuration drives standalone `tauri-driver`, which
supports Linux and Windows, not macOS. This is a limitation of that route,
not of every available Tauri automation option; see the evaluation below.

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

The native file picker is outside this suite's WebDriver-controlled DOM.
The checked-in smoke suite verifies launch UI only: it does not yet automate
file selection, a validation run, cancellation, export, or native menus.
The Rust integration tests cover the shared validation pipeline and bridge,
not those native interactions.

Do not copy a direct `window.__TAURI__.core.invoke("validate", { path })`
example: this app does not enable the global Tauri API, and `validate` takes
one `request` argument (a `ValidateRequest`: path, roundtrip, parser kind,
strict linkers and jobs), not a bare path. Production calls
use the typed runtime capability and transport. A future automated native
validation test must use that contract, subscribe before starting the run,
await its terminal event with a bounded timeout, and isolate cache/settings.
A fixed sleep is not proof of completion. Do not add a production test-only
command or bypass admission just to drive a test.

### Adding E2E tests

Test file: `apps/chatter-desktop/tests/e2e/*.spec.ts`

Follow the existing selector-based launch checks for layout assertions.
New validation-flow coverage must meet the lifecycle and isolation requirements
above; passing a DOM assertion after a delay is not a validation receipt.

### When to run E2E tests

- **Before releases**: manual run to verify the built app works end-to-end
- **Optionally in CI**: requires `tauri-driver` and a display server (Xvfb on
  Linux). Slow, so consider running only on release branches.
- **Not on every commit**: the Rust integration tests are fast and cover more
  ground

## Platform-Specific Considerations

| Platform | WebView engine | E2E support |
|----------|---------------|-------------|
| macOS | WKWebView | Not supported by our current standalone-driver configuration; service-based alternatives exist |
| Windows | WebView2 (Chromium) | Full support via `tauri-driver` |
| Linux | WebKitGTK | Full support via `tauri-driver`; requires Xvfb for headless |

**Current macOS coverage:** use the Rust integration tests (Tier 2) and explicit
native smoke evidence. Do not infer native coverage from browser or transport
doubles. The service-based option below has not been adopted or verified here.

### Automation follow-up: evaluation, not implementation

The [Tauri WebDriver guide](https://v2.tauri.app/develop/tests/webdriver/)
documents `@wdio/tauri-service` with an embedded WebDriver plugin supporting
macOS as well as Linux and Windows. It also documents renderer-only browser mode
and external-driver alternatives. See the
[WebdriverIO Tauri documentation](https://webdriver.io/docs/desktop-testing/tauri/).
These capabilities are upstream claims, not passing Chatter test receipts.

The least disruptive next experiment is rendered-component testing through the
existing `DesktopRuntimeProvider` injection seam: valid, invalid and internal
failure results; cancel/revalidate; export failure; and overlapping update
requests. Keep typed event sequences and bounded completion assertions. This
would complement the existing Rust bridge tests, not replace real IPC evidence.

Before adding a native automation dependency, review its permissions, lifecycle,
dependency cost and release exclusion. An embedded control server must never
ship in production artifacts; require an explicit test-build configuration and
an automated release-exclusion check. Do not add a second validator, test-only
production command, or global IPC escape hatch. Keep settings/cache isolated.

The existing native smoke deck only checks launch UI. Its configuration also
resolves the binary under `apps/target`, rather than the workspace `target`, and
does not select the Windows `.exe` suffix. Correct binary discovery and retain
an actual successful run before treating this deck as a usable release check.
The smoke test's suggested `validate_for_test` shortcut is not an approved design.

CSS rendering differs slightly between WebKit (Linux) and Chromium (Windows).
Visual regressions are possible, consider screenshot comparison tests if this
becomes a problem.

## Test Data

All tests use the reference corpus at `corpus/reference/`. This
corpus is checked into the repo and must always pass validation with
zero hard errors (warnings are allowed). The exact set of files and
the current warning-emitting files are whatever
`rg --files corpus/reference -g '*.cha'` and the validator
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
