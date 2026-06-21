# chatter

[![CI](https://github.com/TalkBank/chatter/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/TalkBank/chatter/actions/workflows/ci.yml)
[![Cross-platform](https://github.com/TalkBank/chatter/actions/workflows/cross-platform.yml/badge.svg?branch=main)](https://github.com/TalkBank/chatter/actions/workflows/cross-platform.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

**chatter** is the modern toolchain for the TalkBank
[CHAT](https://talkbank.org/0info/manuals/CHAT.html) transcript format:
validate, convert, normalize, and explore `.cha` files through a desktop app,
a command-line tool, and an editor language server.

## Are you a clinician or researcher? Start here

If you work with CHAT transcripts (CHILDES, AphasiaBank, FluencyBank, and the
rest of TalkBank) and just want to use chatter, you do not need Rust or any
build tools:

- **Desktop app (no terminal needed).** Download **Chatter** for macOS,
  Windows, or Linux from the
  [latest release](https://github.com/TalkBank/chatter/releases/latest), open
  a `.cha` file, and read its validation results. The app keeps itself up to
  date: it checks for a new version on launch and offers to install it.
  Recommended if you do not use a command line.
- **Command line (`chatter`).** Install with one command, then validate a
  single file or a whole folder:

  ```sh
  # macOS / Linux (Windows: see Installation for the PowerShell one-liner)
  curl --proto '=https' --tlsv1.2 -LsSf https://github.com/TalkBank/chatter/releases/latest/download/talkbank-cli-installer.sh | sh

  chatter validate myfile.cha       # check one transcript
  chatter validate path/to/folder   # check an entire corpus
  chatter --help                    # list every command
  chatter update                    # update to the newest release
  ```

New to chatter? The
**[User Guide](book/src/chatter/user-guide/quick-start.md)** walks through
validating, fixing, and converting transcripts step by step, and the
[Installation](#installation) section below has full per-platform details.

CHAT (Codes for the Human Analysis of Transcripts) is the 40-year-old
transcription format behind every [TalkBank](https://talkbank.org) corpus.
chatter is the canonical home of the CHAT-format authority and the `chatter`
tool family, modernizing the CLAN-era workflow with typed Rust infrastructure:
a strict parser, structured validation diagnostics, programmable transforms,
and an LSP-backed editor experience.

> **Early release (v0.1.0).** chatter is usable today and the CHAT-format core
> (parser, data model, validation, alignment) is mature, but no surface is
> API-stable yet: expect changes before 1.0. See the
> [support tiers page](book/src/contributing/support-tiers.md) for the
> surface-by-surface stability matrix.

## What's in this repo

### Library crates (the CHAT format authority)

The Rust crates behind the CHAT-format core. The foundation crates are
preview-quality; some surfaces are intentionally held back as
experimental or internal until their stability contracts are settled.

| Crate | Role | Stability |
|---|---|---|
| [`talkbank-model`](crates/talkbank-model/) | CHAT data model, validation rules, error codes, tier-alignment primitives | Preview |
| [`talkbank-parser`](crates/talkbank-parser/) | Tree-sitter-backed primary parser (incremental, LSP-friendly) | Preview |
| [`talkbank-parser-re2c`](crates/talkbank-parser-re2c/) | Independent re2c-based oracle parser that cross-checks tree-sitter on every wild-corpus file | Preview |
| [`talkbank-parser-tests`](crates/talkbank-parser-tests/) | Shared parser test harness + reference corpus golden tests | Internal |
| [`talkbank-transform`](crates/talkbank-transform/) | CHAT to JSON, normalization, transcript-merge, redaction, transform pipelines | Preview |
| [`talkbank-derive`](crates/talkbank-derive/) | Derive macros for the data model | Preview |
| [`talkbank-cache`](crates/talkbank-cache/) | SQLite-backed pass/fail cache for validation + roundtrip results | Preview |
| [`talkbank-llm`](crates/talkbank-llm/) | OpenAI-compatible HTTP judgment provider (the only network-dependent crate; backs holistic speaker-id) | Experimental |
| [`send2clan`](crates/send2clan/) | Rust bindings for "open in CLAN" (macOS Apple Events, Windows WM_APP) | Experimental |

### User-facing tools

| Component | What it is | Stability |
|---|---|---|
| [`talkbank-cli`](crates/talkbank-cli/), the `chatter` binary | The flagship CLI: validate, normalize, convert (JSON / XML), lint, watch, and the experimental merge / speaker-id reconciliation commands | Preview |
| [`talkbank-lsp`](crates/talkbank-lsp/), the `talkbank-lsp` binary | Language Server Protocol implementation; powers real-time validation, hover, go-to-definition, cross-tier alignment in any LSP-aware editor | Preview |
| [`apps/chatter-desktop/`](apps/chatter-desktop/) | Tauri-based desktop validation app, a TUI parity target for researchers who don't use a terminal | Preview |

### Format authority

| Path | Role |
|---|---|
| [`grammar/`](grammar/) | The `tree-sitter-talkbank` grammar, the source of truth for CHAT syntax. Generates the parser used by `talkbank-parser`. |
| [`spec/`](spec/) | The CHAT format specification (constructs, errors, symbol registry); generates the test corpus that gates every grammar/parser change |
| [`schema/`](schema/) | JSON Schema for the typed CHAT AST emitted by `chatter to-json` |
| [`corpus/reference/`](corpus/reference/) | The reference corpus: every file MUST pass parser equivalence and roundtrip validation on every commit |
| [`test-fixtures/`](test-fixtures/) | Minimal `.cha` fixtures used by integration tests |

### Documentation

| Where | What |
|---|---|
| [`book/`](book/) | Comprehensive mdBook: user guides for the chatter CLI, plus CHAT format documentation, architecture, and contributor guides |
| [`docs/strategy/`](docs/strategy/) | Strategic planning docs (distribution + signing) |
| [`docs/proposals/`](docs/proposals/) | Format-extension proposals under review |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | How to set up a fresh checkout and contribute |

## Installation

Prebuilt binaries for macOS (Apple Silicon and Intel), Linux, and
Windows are attached to every [GitHub
Release](https://github.com/TalkBank/chatter/releases). The `chatter`
CLI installs via the release's shell (macOS/Linux) or PowerShell
(Windows) installer script, or you can download the archive for your
platform directly from the release page.

- **macOS / Linux (CLI):** run the installer one-liner from the
  [latest release](https://github.com/TalkBank/chatter/releases/latest).
- **Windows (CLI):** run the PowerShell installer from the latest
  release. The downloaded binary is not yet code-signed, so SmartScreen
  may warn; choose "More info" then "Run anyway".
- **Desktop app:** download the installer for your platform from the
  release page. The macOS `.dmg` is signed and notarized; the Windows
  installer is not yet signed (same SmartScreen note as above).
- **From crates.io:** not yet published (see the support tiers page).

To build the CLI from source instead, see the developer quick start
below.

## Quick start (for developers)

Prerequisites: the Rust toolchain pinned by `rust-toolchain.toml`
(currently 1.96.0; `rustup` installs it automatically) and SQLite dev
headers (macOS bundled; Linux needs `libsqlite3-dev`).

```sh
# Clone
git clone git@github.com:TalkBank/chatter.git
cd chatter

# Build the whole workspace
cargo build --workspace --all-targets --locked

# Run the full workspace test suite
cargo test --workspace --locked

# Try the chatter binary
./target/debug/chatter --help
./target/debug/chatter validate path/to/file.cha
```

Build the docs locally:

```sh
just book-install-tools
just book
# Output at book/build/html/
```

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the full development
setup and the coding conventions.

## Architecture at a glance

```
                ┌────────────────────────────────────────────────┐
                │  spec/  →  grammar/  →  talkbank-parser        │  CHAT format
                │  (source of truth)        (tree-sitter, LSP-friendly) │  authority
                └─────────────────────────┬──────────────────────┘
                                          ▼
                ┌────────────────────────────────────────────────┐
                │  talkbank-model  +  talkbank-derive            │  Data model
                │  (validation rules, error codes, alignment)    │  + lints
                └─────────────────────────┬──────────────────────┘
                                          ▼
                ┌────────────────────────────────────────────────┐
                │  talkbank-transform  +  talkbank-cache         │  Pipelines
                │  (CHAT↔JSON, normalize, merge, redact)         │
                └─────────────────────────┬──────────────────────┘
                                          ▼
              ┌────────────────┬───────────┴───────┐
              ▼                ▼                   ▼
       talkbank-cli      talkbank-lsp       chatter-desktop
         (chatter)       (lsp binary)         (Tauri app)
```

Detailed architecture documentation: the
[architecture overview](book/src/architecture/overview.md).

## Toolchain

- **Rust** (pinned to a specific stable in `rust-toolchain.toml`, currently
  `1.96.0`; edition 2024; no MSRV declared until crates.io publication)
- **Node 20+** for the desktop app's web front end
- **SQLite** dev headers (macOS bundled; Linux `libsqlite3-dev`)

## License

Dual-licensed under your choice of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

Unless explicitly stated otherwise, any contribution intentionally
submitted for inclusion in this work shall be dual-licensed as
above, without any additional terms or conditions.

## Related projects

- [`batchalign3`](https://github.com/TalkBank/batchalign3): the
  upstream neural ML pipeline (ASR, forced alignment, neural
  morphotag); the two projects coordinate at the CHAT-format boundary
- [TalkBank](https://talkbank.org): the parent project; CHAT format
  is the data substrate for every TalkBank corpus
- [CHILDES](https://childes.talkbank.org),
  [AphasiaBank](https://aphasia.talkbank.org),
  [FluencyBank](https://fluency.talkbank.org), and the other TalkBank
  banks
