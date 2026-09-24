# Chatter Desktop

**Status:** Current
**Last modified:** 2026-09-24 00:21 EDT

Chatter Desktop is a native graphical validation app for CHAT files, released
alongside the `chatter` CLI. Prefer the `chatter` CLI for scripted or batch
validation; use the desktop app when you want a standalone graphical validation
experience without a terminal.

## When to use Chatter Desktop

Chatter Desktop (`apps/chatter-desktop/`) is the right tool when you want to:
- Validate CHAT files through a graphical interface, no terminal required
- Drag and drop a file or folder and read errors with source snippets
- Work on the desktop without setting up a terminal workflow

**Related surfaces:**
- **Validate CHAT from the command line:** use `chatter validate`

This page documents the desktop surface:

- **Chatter Desktop** (`apps/chatter-desktop/`), the CHAT validation GUI

## Current status

- **Release contract:** released alongside the CLI in the public chatter release
- **Distribution:** ships in the coordinated chatter release alongside the CLI; also buildable from source (below)
- **Platforms:** macOS, Windows, and Linux

## Staying up to date

Chatter Desktop keeps itself current. When you launch it, it quietly checks
for a newer release; if one is available it asks whether to update, and on
your confirmation it downloads, installs, and restarts into the new version.
The app also checks every six hours while running. Use **Check for Updates…**
in the application menu to check immediately; a manual check reports when you
are current or when the check fails. Overlapping checks share one operation,
so repeated menu clicks do not start duplicate prompts or installations.

If a background check cannot reach the network, the app keeps working on the
installed version. A failed check is not evidence that the installed version
is current. Retry the menu command later or download an installer from the
release page. Export any results you want to retain before accepting an update:
installation relaunches the app, and the current results are not a saved session.
**About Chatter** shows the installed version and links to the project.

## Getting Started

### Install the released application

Open the [Chatter release page](https://github.com/TalkBank/chatter/releases/latest)
and choose the desktop installer for your platform, rather than a CLI archive:

| Platform | Desktop download | Installation |
| --- | --- | --- |
| macOS, Apple silicon | `Chatter-macos-apple-silicon.dmg` | Open the disk image and copy Chatter to Applications. |
| macOS, Intel | `Chatter-macos-intel.dmg` | Open the disk image and copy Chatter to Applications. |
| Windows | `Chatter-windows-setup.exe` | Run the installer. |
| Linux | `Chatter-linux-x86_64.deb` or `.AppImage` | Use the Debian package manager, or make the AppImage executable and launch it. |

The macOS application and disk image are signed and notarized by the release
pipeline. Windows installers currently lack Authenticode signing and may show
a SmartScreen warning. Verify that you downloaded the intended project release;
do not disable operating-system security globally to install it. Updater
signatures are separate from operating-system signing.

No Rust toolchain, Node.js, terminal, or separate CLI installation is needed to
use a released desktop app. CLAN is optional and needed only for **Open in CLAN**.

### A first validation session

1. Choose one `.cha` file or a folder. Folder validation includes subfolders.
2. Leave **Tree-sitter** selected. Keep optional roundtrip and strict-linker
   checks off unless you need them for this session.
3. Wait for completion; a blank problem list during discovery or processing
   does not certify the files.
4. Select a file with a diagnostic or processing failure, read its message,
   and use **Copy**, **Reveal**, or **Open in CLAN** as appropriate.
5. Edit the original transcript in your editor, save it, and **Re-validate**.
   Export results if you need a record before starting another target.

Validation does not edit or rename your transcripts. The desktop app is not a
CHAT editor or an automatic repair interface.

### Build from source

```bash
cd apps/chatter-desktop
npm ci
cargo tauri dev       # launches the app with hot reload
cargo tauri build     # produces a distributable app bundle
```

Use the repository's pinned Rust toolchain and the Node version used by its
desktop workflow. Linux also needs Tauri's native system dependencies. A
distributable updater-enabled build needs signing configuration; the two build
commands above are not a substitute for the coordinated release pipeline.
See [Desktop App Testing](../../contributing/desktop-testing.md) and
[CI and Release](../../contributing/ci-and-release.md).

## Using the App

### Opening files

Chatter validates **one target at a time**: a single `.cha` file or one folder.

Three ways to start validating:

1. **Choose File**: opens a file picker filtered to `.cha` files
2. **Choose Folder**: opens a folder picker; validates all `.cha` files recursively
3. **Drag and drop**: drag one `.cha` file or one folder onto the app window

When idle, if you've previously validated a target, the drop zone shows
**"Last: corpus/reference/, Re-validate?"** as a clickable shortcut.

### Reading results

The main window has three areas:

```text
┌──────────────────────────────────────────────────────────────┐
│  [Choose File] [Choose Folder] or drag here  [System|Light|Dark] │
├──────────────────┬───────────────────────────────────────────┤
│ 3 FILES WITH     │  Filter by code… [All|Errors|Warnings]    │
│ ERRORS / 120     │                                           │
│                  │  ▾ [E302] Missing @End header              │
│  📁 corpus/      │  ┌───────────────────────┐                │
│    ✗ file1 (3)   │  │ 41 │ *CHI: hello .    │                │
│    ✗ file3 (1)   │  │ 42 │                   │                │
│                  │  │    │ ^                 │                │
│                  │  └───────────────────────┘                │
│                  │  💡 Add @End on the last line             │
│                  │  [Copy] [Open in CLAN]                    │
├──────────────────┴───────────────────────────────────────────┤
│  Progress: 45/120 │ 4 errors │ ~2m 30s remaining │ [Cancel]  │
└──────────────────────────────────────────────────────────────┘
```

- **File tree** (left), collapsible directory tree showing files with diagnostics
  (including warnings) or read, parse and roundtrip failures. Valid files without
  diagnostics are hidden to reduce clutter. A header shows "N files
  with errors / M total". Files are sorted alphabetically.

- **Error panel** (right), for the selected file, shows each error with its
  code in `[E001]` format, severity color, message, source snippet with caret
  underlines, and multi-span labels for complex errors (e.g., alignment
  mismatches across tiers). CHAT-specific formatting is handled: tabs expanded
  to 8-column boundaries, `\x15` bullets rendered as `•`, underline markers
  shown as styled underlined text. Suggestions prefixed with 💡.

- **Status bar** (bottom), streaming progress during validation, ETA after 5+
  files, total error count, and action buttons.

### Filtering errors

A compact filter bar appears above the error cards when a file has diagnostics:

- **Code filter**: type "E7" to show only alignment errors, "W" for warnings, etc.
- **Severity toggle**: switch between All / Errors / Warnings

The file header updates to show filtered vs. total count (e.g., "3 errors (7 total)").

### Collapsible error cards

Each error card has a clickable header that toggles between expanded and
collapsed view. Collapsed cards show only the error code and first line of the
message. When a file has 5 or more errors, an **Expand All / Collapse All**
button appears.

### Validation settings

A **⚙ Settings** popover next to the file picker exposes the same knobs the
CLI's flags do, since both surfaces build the same underlying validation
config:

| Setting | Equivalent CLI flag | Default |
|---------|---------------------|---------|
| Roundtrip check | `--roundtrip` | Off |
| Parser | `--parser tree-sitter\|re2c` | Tree-sitter |
| Strict cross-utterance linkers | `--strict-linkers` | Off |
| Parallel jobs | `--jobs N` | All CPUs |

Settings are disabled while a validation run is in progress and apply to the
next run (including Re-validate).

**Re2c is experimental and incomplete.** It is not a second validity authority;
use Tree-sitter for ordinary work and report disagreements with a minimal CHAT
example. Strict-linker checks enforce additional quotation/completion conventions
that are not enabled for ordinary validation. Roundtrip checking compares the
parsed model with its serialized and reparsed form; it is not an audio check or
a guarantee that every byte keeps its original formatting.

The app shares the CLI's validation cache and rule-aware engine. Changed files
are revalidated; a cached validation result is not itself proof that an optional
roundtrip check ran. Selected settings apply to the next run, not the run already
in progress. Settings currently reset to their defaults when the app restarts.

### Failures, warnings and incomplete results

Read and parse failures may have no CHAT error card because the validator could
not obtain a usable transcript. They still appear in the file tree, and the
detail panel shows the reason. A roundtrip failure is likewise a failed check,
not “No errors.” Report unexpected roundtrip failures rather than editing the
data merely to silence an internal mismatch.

“All files valid” requires a completed, non-cancelled, nonempty run with every
file accounted for as valid and no visible diagnostics or roundtrip failures.
Cancelling, losing files during a run, or failing to read a file cannot earn
that summary. An empty folder means **No CHAT files found**, not a successful
validation population. The title, notification and status bar share the same
completion summary.

Warning-only files remain visible even though a warning is not a hard error.
For example, W109 identifies a nonstandard Unicode spelling in the `@Media`
name, the stored transcript filename, or both. Typing a different Unicode form
of the same filesystem path must not change that diagnosis. The optional CLI
repair `chatter fix --code W109 --apply <file>` normalizes only the media-name
token; it never renames the file. A file-only warning can remain afterward.
See [Headers](../../chat-format/headers.md) for normalization details.

### Dark mode

Chatter follows your system appearance by default. A **System / Light / Dark**
toggle in the drop zone area lets you override. Your preference is remembered
across sessions.

The dark palette uses muted Apple-style colors, readable miette error
highlighting on dark backgrounds.

### Clickable file paths

Click the file name in the error panel heading to **reveal the file in Finder**
(macOS), Explorer (Windows), or the default file manager (Linux).

### Copy errors

Each error card has a **Copy** button that copies the full miette-rendered error
text (plain text, not HTML) to your clipboard for pasting into issue reports or
messages.

### Actions

| Action | Where | What it does |
|--------|-------|--------------|
| **Re-validate** | Status bar / last-target hint | Re-run validation on the same target (picks up edits) |
| **Cancel** | Status bar (during validation) | Stop the current run |
| **Export** | Status bar | Save results as JSON or plain text via a save dialog |
| **Open in CLAN** | Per-error button | Opens the file at the error location in the CLAN editor |
| **Copy** | Per-error button | Copies the plain-text error to clipboard |
| **Reveal in file manager** | File name heading | Opens the file's parent directory |

"Open in CLAN" only appears when the CLAN application is detected on your
system (macOS and Windows only). It adjusts line numbers to account for headers
that CLAN hides (`@UTF8`, `@PID`, `@Font`, `@ColorWords`, `@Window`).

### Exporting results

After a run ends, **Export** opens a save dialog. JSON preserves per-file
diagnostics and status; plain text includes each file's status and its rendered
diagnostics. Read, parse and roundtrip failure reasons are included even when
there is no CHAT diagnostic card. Copying one card exports only that diagnostic,
not the outcome of the whole file or folder.

A cancelled run contains only the results obtained before cancellation. An
export is a record of those results, not proof that every requested file was
checked. Retain the run's completion/cancellation context with the report;
the current per-file export does not include a full session manifest with
settings, application version and run coverage. Revalidate to obtain a complete
population before making a whole-folder validity claim.

### Keyboard shortcuts

| Shortcut | Action |
|----------|--------|
| Ctrl+R / Cmd+R | Re-validate |
| Escape | Cancel running validation |

All other navigation is mouse-driven (click files, scroll errors).

### Window title

The window title updates to reflect the current state:

- **Idle:** "Chatter"
- **Starting:** "Chatter, Starting…"
- **Discovering:** "Chatter, Discovering files…"
- **Running:** "Chatter, Validating (45/120)"
- **Finished:** a diagnostic/failure summary or "Chatter, All 74 files valid"
- **Cancelled:** "Chatter, Cancelled; results are partial"
- **Empty:** "Chatter, No CHAT files found"
- **Incomplete:** "Chatter, Incomplete (2 files not checked)"
- **Stopped:** "Chatter, Run stopped unexpectedly"

The last two are failures, and they never claim anything about your whole
folder. **Incomplete** means the validator finished but some files were
never opened, so the counts it shows describe only the rest; you will see
how many were missed, and re-validating is the right response.
**Stopped** means the run died without producing results at all. Neither
one can show "All N files valid", because that sentence is a claim about
every file and neither run examined every file.

"Starting" and "Discovering" are different states, and the difference is worth
knowing if you ever need to report a problem. **Starting** means the app has
asked the validator to begin and has not heard back; nothing has been scanned
yet, so a run stuck there is a fault in start-up rather than anything about
your files. **Discovering** means the validator is walking the folder, which
legitimately takes time on a large one. If the app sits on "Starting" for more
than a few seconds it says so in the status bar, and that message is worth
quoting in a bug report.

### ETA

After 5 or more files have been processed, the status bar shows an estimated
time remaining (e.g., "~2m 30s remaining"). The estimate updates every second.

### Notifications

When validation finishes while the app is not focused, a system notification
shows the summary ("Validation complete, 14 errors in 3 files").

### First launch

On first launch, an onboarding overlay explains the four main interactions: drag
files, error panel, keyboard shortcuts, and export. Dismiss with "Got it", it
won't appear again.

## CLI Bundling

Install the CLI separately from the same release when you need scripted
validation or repairs. The current desktop bundle configuration does not ship a
CLI resource or an **Install CLI Command** menu item. A backend installation
command is not evidence that a released bundle contains a CLI. Bundling remains
future work, not a prerequisite for using desktop validation.

## Troubleshooting and reporting a problem

| Symptom | What to check |
| --- | --- |
| Stuck on Starting | Quote the startup message; this is before file discovery. |
| Discovering takes time | Large or network folders can be slow. Try one local file to isolate the issue. |
| Read error | Confirm the file still exists and the app can read it; check volume availability and permissions. |
| No CHAT files found | Select the intended folder and check that transcripts have `.cha` extensions. |
| Open in CLAN unavailable | Confirm a supported CLAN installation; validation itself does not need CLAN. |
| Update failed | Keep using the installed app, retry later, or use the official installer. |
| Roundtrip failed | Retain the transcript and reported reason; this can indicate a parser/serializer defect. |

Include the installed version from **About Chatter**, operating system, selected
parser/settings, whether the target was one file or a folder, and copied
diagnostics. Share a minimal sanitized example when possible. Review exported
results before sharing: they can contain file paths and transcript snippets.
Do not put private participant data in a public issue.

### Current limitations

- One validation target per run; no in-app editing or automatic fix command.
- No persistent validation-result session; export results before closing or updating.
- Re2c remains experimental; Tree-sitter is the default.
- Desktop navigation is primarily mouse-driven; it is not the terminal UI's key map.
- CLAN integration is platform-dependent. Native end-to-end automation is available
  on Linux/Windows; macOS requires a real-app smoke review as well as seam tests.

## Architecture

The desktop app lives in `apps/chatter-desktop/`:

```text
apps/chatter-desktop/
  src-tauri/          Rust backend (Tauri v2)
    src/
      main.rs         Bin entry, calls chatter_desktop_lib::run()
      lib.rs          Tauri app setup (Builder + module wiring)
      protocol.rs     Shared command/event names + request types
      commands.rs     validate, cancel, open_in_clan, export, reveal, install_cli
      events.rs       ValidationEvent → frontend event bridge
      validation.rs   Desktop validation orchestration for one target
  src/                React + TypeScript frontend
    components/       DropZone, FileTree, ErrorPanel, ProgressBar, OnboardingOverlay
    hooks/            useValidation, validationState, useTheme
    protocol/         Command/event names + TypeScript transport mirrors
    runtime/          Tauri transport + capability-focused runtime seam
```

The Rust backend calls `validate_directory_streaming()` and
`validate_files_streaming()` from `talkbank-transform` directly (folder vs.
single-file targets respectively), the same streaming validation pipeline and
on-disk cache used by the CLI and TUI. Events flow over crossbeam channels to
the Rust side, then are serialized to JSON and emitted to the frontend via
Tauri's event bridge.

Cancellation uses `ArcSwapOption` for lock-free atomic swap of the cancel
sender, no mutex.

The frontend keeps Tauri-specific code confined to `src/runtime/tauriTransport.ts`.
React components and hooks consume narrower capabilities (`validationRunner`,
`validationTarget`, `clan`, `exports`) instead of reaching for one broad
desktop service object.

## Comparison with TUI

| Feature | TUI (`chatter validate`) | Desktop app |
|---------|--------------------------|-------------|
| File selection | CLI arguments | Drag-and-drop, file picker |
| Navigation | Keyboard (Tab, arrows) | Mouse click |
| Error display | Two-pane terminal UI | Scrollable panels with source snippets |
| Error filtering |, | Code filter + severity toggle |
| Copy error |, | Copy button per error |
| Open in CLAN | `c` key | Button per error |
| Export | `--format json --audit` | Save dialog (JSON or text) |
| Streaming progress | Progress bar | Progress bar + ETA |
| Dark mode | Terminal theme | System/Light/Dark toggle |
| Caching | Same engine | Same engine |
| Who it's for | Power users, CI | Researchers, linguists |

Both use the identical validation engine and produce the same error codes.

## When to Use Which Tool

The TalkBank toolchain offers validation through three interfaces. Each serves a
different workflow:

| Tool | Audience | Use when |
|------|----------|----------|
| **Chatter Desktop** | Researchers, linguists | You want a graphical, drag-and-drop CHAT validation app without using a terminal. |
| **`chatter validate` (TUI)** | Power users | You're comfortable in a terminal and want keyboard-driven navigation. |
| **`chatter validate` (CLI)** | CI, scripts | You need machine-readable output (`--format json`) or batch audits (`--audit`). |

Chatter Desktop focuses on **validation only**.
