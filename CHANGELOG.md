# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Before 1.0, breaking changes to the CLI or library APIs bump the minor
version and are listed under "Changed" / "Removed".

## [Unreleased]

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

[Unreleased]: https://github.com/TalkBank/chatter/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/TalkBank/chatter/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/TalkBank/chatter/releases/tag/v0.1.0
