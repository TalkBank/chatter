# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Before 1.0, breaking changes to the CLI or library APIs bump the minor
version and are listed under "Changed" / "Removed".

## [Unreleased]

## [0.2.1] - 2026-06-24

### Added

- **The `talkbank-lsp` language server now ships as a standalone release
  artifact.** Prebuilt, code-signed `talkbank-lsp` binaries for macOS (Apple
  Silicon and Intel), Linux (x86_64 and aarch64, static musl), and Windows are
  attached to the GitHub Release, each with its own `talkbank-lsp-installer.sh`
  / `talkbank-lsp-installer.ps1`. Any LSP-aware editor can now install the server
  without building it from source; it is a first-class artifact in its own right,
  not only the binary the VS Code extension bundles per platform.

## [0.2.0] - 2026-06-23

### Added

- **More of CLAN CHECK's invalidity is now enforced.** A batch of CHECK-parity
  rules was implemented so `chatter validate` rejects more invalid CHAT:
  - `E514`: an `@ID` line's corpus field is required (CHECK 63).
  - `E547`: a constant participant header must follow the `@ID` block.
  - `E548`: closes the case CHECK 126 covers.
  - `E549`: a speaker may not be declared twice (CHECK 13).
  - Duplicate `@ID` lines and out-of-order `@Options` fields (CHECK 13, 125).
  - A dependent tier used without being declared (CHECK 17).
  - An out-of-range `@Time Duration` (CHECK 35).
  - An `@Media` header marked unlinked while the transcript still carries timing
    bullets (CHECK 124), and an `@Media` filename that does not match the data
    file (CHECK 157).
  - A replacement `[: ...]` now requires a preceding space (CHECK 161).
  - Tree-sitter recovery nodes are surfaced as invalidity rather than silently
    repaired: a surviving `ERROR` node maps to `E316` and a `MISSING` node to
    `E342` (with the re2c oracle mirroring it), covering a group with no
    annotation and swallowed recovery nodes inside comma-list headers
    (CHECK 5/6/106/108).
- **Phon:** `U` (unknown) is accepted as a legal syllable-constituent code on the
  `%xmodsyl` and `%xphosyl` tiers.
- A formal behavioral CHECK-validity parity test suite that runs real CLAN CHECK
  and chatter on the same fixtures and fails if either side drifts.

### Changed

- **`chatter update` now self-updates in process.** It embeds the axoupdater
  self-updater as a library, reads the cargo-dist install receipt (keyed by the
  package name), and replaces the running binary from GitHub Releases. This
  removes the package-name coupling that previously made `chatter update` report
  "not installed" on a correctly installed binary.
- **The CLI package is renamed `talkbank-cli` to `chatter`** (the crate now lives
  at `crates/chatter/`). The generated install scripts are therefore
  `chatter-installer.sh` and `chatter-installer.ps1` (previously
  `talkbank-cli-installer.*`); update any pinned install URL accordingly. The
  binary is still `chatter`, and the library/API crates keep their `talkbank-*`
  names.
- **Validation is stricter.** Because of the new CHECK-parity rules above, some
  files that passed `chatter validate` under 0.1.1 may now report errors. This is
  intended: chatter is the CHAT-validity authority and is at least as strict as
  CLAN CHECK.

### Removed

- The standalone self-updater binary (cargo-dist `install-updater = false`). The
  `chatter update` subcommand is unchanged for users; it now updates in process
  instead of shelling out to a separate program.

### Fixed

- The recovery-node invalidity backstop is scoped to localized errors so it does
  not over-flag, and several malformed `@ID` test fixtures were corrected.
- Hardened the CHECK-parity audit and corrected a CHECK 126 verdict it had
  falsely certified; the curated CHECK error-code map is restored in place of a
  brittle keyword heuristic.

## [0.1.1] - 2026-06-22

### Fixed

- **Validation cache could serve a stale verdict across rule-set changes.**
  `chatter validate` keyed its result cache on the cache crate's package
  version, which does not change when validation rules change, so a "Valid"
  result cached before a new rule (such as a retrace-marker check) existed kept
  being served, while a fresh conversion of the same bytes correctly rejected
  them. The cache key now folds in a fingerprint over every error-code rule, so
  adding, removing, or renaming any rule invalidates stale entries; the cache
  is kept and still functions, only keyed correctly.
- CLI usage lines pin the binary name to `chatter` regardless of the invoked
  path (clap `bin_name`).
- The book renders Mermaid diagrams again (restored mdbook-mermaid assets).
- **Desktop app version is now locked to the release version.** The desktop
  bundle (`.dmg` / `.exe` / `.deb`) and the Tauri auto-updater manifest now report
  the same version as the CLI. A version-sync gate (`scripts/sync-app-version.py`,
  enforced in CI and at release time) keeps `tauri.conf.json`, `package.json`, the
  workspace version, and this changelog from drifting, so the updater can never
  again advertise a version the installed bundle does not match.

### Changed

- CI book toolchain bumped to mdBook 0.5.3 and mdbook-mermaid 0.17.0.
- Build: force `serialize-javascript >= 7.0.5` to clear advisories, and bump
  `rand` in the spec crate.
- Docs: the book intro is de-staged for the public release (download-first).

## [0.1.0] - 2026-06-15

First public release.

### Added

- **CHAT-format core.** A strict, incremental tree-sitter parser
  (`talkbank-parser`) with an independent re2c oracle parser
  (`talkbank-parser-re2c`) that cross-checks it on every file; a typed
  CHAT data model with structured validation, error codes, and tier
  alignment (`talkbank-model`); and CHAT-to-JSON / JSON-to-CHAT / XML
  conversion, normalization, transcript-merge, and redaction pipelines
  (`talkbank-transform`).
- **Phon extension tiers.** The four Phon `%x` dependent tiers
  (`%xmodsyl`, `%xphosyl`, `%xphoaln`, `%xphoint`) are parsed and
  validated as first-class CHAT tiers, on by default (pass
  `--suppress xphon` to opt out): syllabification constituent codes and
  phone-vs-source reconstruction, model-to-actual phone alignment, and
  per-phone time intervals, with dedicated error codes.
- **`chatter` CLI.** `validate`, `normalize`, `to-json` / `from-json` /
  `to-xml`, `merge`, `speaker-id`, `batch`, `pipeline`, `adjudicate`,
  `sanity-scan`, `lint`, `clean`, `watch`, `new-file`, `show-alignment`,
  `validate-utseg`, `schema`, `update`, and a content cache.
- **Language server** (`talkbank-lsp`): real-time validation, hover,
  go-to-definition, and cross-tier alignment for any LSP-aware editor.
- **Desktop app** (`Chatter`): a Tauri-based CHAT validation app, shipping
  in the coordinated release alongside the CLI.
- **Auto-update.** The `chatter` CLI self-updates with `chatter update`
  (the bundled cargo-dist / axoupdater self-updater), and the desktop app
  checks for and installs new releases on launch (Tauri updater). Both pull
  from GitHub Releases. The CLI self-updater is experimental.
- **Prebuilt binaries** for macOS (Apple Silicon and Intel), Linux, and
  Windows, plus desktop installers, attached to the GitHub Release. The
  macOS desktop `.dmg` is signed and notarized.

### Known limitations

- **The merge and adjudication surface is experimental.** `merge`,
  `adjudicate`, `speaker-id`, and `sanity-scan` work, but their
  interfaces and heuristics may change before 1.0.
- **Windows binaries are not code-signed yet**, so Windows SmartScreen
  warns on first run (choose "More info" then "Run anyway"). macOS CLI
  binaries are codesigned but not notarized; install via the release
  installer script to avoid the Gatekeeper quarantine prompt.
- **Not on crates.io yet.** crates.io publication is deferred.

[Unreleased]: https://github.com/TalkBank/chatter/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/TalkBank/chatter/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/TalkBank/chatter/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/TalkBank/chatter/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/TalkBank/chatter/releases/tag/v0.1.0
