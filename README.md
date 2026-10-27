# chatter

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/README.md)

[![CI](https://github.com/TalkBank/chatter/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/TalkBank/chatter/actions/workflows/ci.yml)
[![Cross-platform](https://github.com/TalkBank/chatter/actions/workflows/cross-platform.yml/badge.svg?branch=main)](https://github.com/TalkBank/chatter/actions/workflows/cross-platform.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

**chatter** is the modern toolchain for the TalkBank
[CHAT](https://talkbank.org/0info/manuals/CHAT.html) transcript format:
validate, convert, normalize, and explore `.cha` files through a
desktop app, a command-line tool, and an editor language server.

## Are you a clinician or researcher? Start here

If you work with CHAT transcripts (CHILDES and the other TalkBank corpora)
and just want to use chatter, you do not need Rust or any
build tools:

- **Desktop app (no terminal needed).** Download **Chatter** for macOS,
  Windows, or Linux from the
  [latest release](https://github.com/TalkBank/chatter/releases/latest), open
  a `.cha` file or a whole folder, and read the validation results. The app
  checks for a new version on launch and offers to install it; an offer is
  not a completed update. Recommended if you do not use a command line.
- **Command line (`chatter`).** Install with one command, then validate a
  single file or a whole folder:

  ```sh
  # macOS / Linux (Windows: see Installation for the PowerShell one-liner)
  curl --proto '=https' --tlsv1.2 -LsSf https://github.com/TalkBank/chatter/releases/latest/download/chatter-installer.sh | sh

  chatter validate myfile.cha       # check one transcript
  chatter validate path/to/folder   # check an entire corpus
  chatter --help                    # list every command
  chatter experimental update                    # update to the newest release
  ```

The installer command downloads and executes a script. Review the official
release and follow your organization's installation policy; see
[Installation](book/src/chatter/user-guide/installation.md) for manual downloads,
checksums and security prompts. `chatter experimental update` requires an official installer
receipt; source and package-manager installs use their original update method.

New to chatter? The
**[User Guide](book/src/chatter/user-guide/quick-start.md)** walks through
validating, fixing, and converting transcripts step by step, and the
[Installation](#installation) section below has full per-platform details.
For live feedback while typing, follow the
[Editor Integration tutorial](book/src/chatter/user-guide/editor-integration.md).

## What this project is

CHAT (Codes for the Human Analysis of Transcripts) is the 40-year-old
transcription format behind every [TalkBank](https://talkbank.org) corpus.
chatter is the canonical home of the CHAT-format authority and the `chatter`
tool family, modernizing the CLAN-era workflow with typed Rust infrastructure:
a strict parser, structured validation diagnostics, programmable transforms,
and an LSP-backed editor experience.

> **Pre-1.0.** chatter is in real production use today and the CHAT-format
> core (parser, data model, validation, alignment) is mature, but public APIs
> may still change between releases until 1.0, per standard semantic
> versioning. Release history and per-release changes:
> [CHANGELOG.md](CHANGELOG.md).

## What's in this repo

### Library crates (the CHAT format authority)

| Crate | Role |
|---|---|
| [`talkbank-model`](crates/talkbank-model/) | CHAT data model, validation rules, error codes, tier-alignment primitives |
| [`talkbank-parser`](crates/talkbank-parser/) | Tree-sitter backend (incremental, current CLI default), driven by a generated, exhaustive typed CST visitor |
| [`talkbank-parser-re2c`](crates/talkbank-parser-re2c/) | Peer CHAT backend using a re2c lexer and native parser combinators, with finite reference and error-spec comparisons |
| [`talkbank-parser-tests`](crates/talkbank-parser-tests/) | Internal test harness: parser equivalence, reference-corpus golden tests, CLAN CHECK parity fixtures |
| [`talkbank-transform`](crates/talkbank-transform/) | CHAT to JSON, normalization, transcript merge, redaction, transform pipelines |
| [`talkbank-derive`](crates/talkbank-derive/) | Derive macros for the data model |
| [`talkbank-cache`](crates/talkbank-cache/) | SQLite-backed pass/fail cache for validation and roundtrip results |
| [`talkbank-llm`](crates/talkbank-llm/) | Opt-in OpenAI-compatible HTTP judgment provider with a persistent response cache; backs holistic speaker-id |
| [`send2clan`](crates/send2clan/) | Rust bindings for "open in CLAN" (macOS Apple Events, Windows WM_APP) |

### User-facing tools

| Component | What it is |
|---|---|
| [`talkbank-chatter`](crates/chatter/), the `chatter` binary | The flagship CLI: validate, normalize, convert (JSON), watch, plus experimental speaker-id reconciliation and interactive adjudication |
| [`talkbank-lsp`](crates/talkbank-lsp/), the `talkbank-lsp` binary | Language Server Protocol implementation; powers real-time validation, hover, go-to-definition, and cross-tier alignment in any LSP-aware editor |
| [`apps/chatter-desktop/`](apps/chatter-desktop/) | Desktop validation app (Tauri) for researchers who do not use a terminal; runs the same validation engine as the CLI, with the same cache |

### Format authority

| Path | Role |
|---|---|
| [`grammar/`](grammar/) | The `tree-sitter-talkbank` grammar, the source of truth for CHAT syntax. Generates the parser used by `talkbank-parser`. |
| [`spec/`](spec/) | The CHAT format specification (constructs, errors, symbol registry); generates the test corpus that gates every grammar/parser change |
| [`schema/`](schema/) | JSON Schema for the typed CHAT AST emitted by `chatter convert to-json` |
| [`corpus/reference/`](corpus/reference/) | Finite reference transcripts for parser, validation and roundtrip regression contracts |
| [`test-fixtures/`](test-fixtures/) | Minimal `.cha` fixtures used by integration tests |

### Documentation

| Where | What |
|---|---|
| [`book/`](book/) | Comprehensive mdBook: user guides for the chatter CLI and desktop app, plus CHAT format documentation, architecture, and contributor guides |
| [`docs/strategy/`](docs/strategy/) | Strategic planning docs (distribution and signing) |
| [`docs/proposals/`](docs/proposals/) | Format-extension proposals under review |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | How to set up a fresh checkout and contribute |

## Installation

Choose a [GitHub Release](https://github.com/TalkBank/chatter/releases) and
inspect its available assets. CLI builds target macOS (Apple Silicon and Intel),
Linux (x86_64 and ARM64), and Windows (x64); desktop installers are separate.
Asset availability does not establish compatibility with every OS version. The `chatter`
CLI installs via the release's shell (macOS/Linux) or PowerShell
(Windows) installer script, or you can download the archive for your
platform directly from the release page.

- **macOS / Linux (CLI):** run the installer one-liner from the
  [latest release](https://github.com/TalkBank/chatter/releases/latest).
- **Windows (CLI):** use the PowerShell installer from the selected release.
  Verify the official source and follow the installation guide and your
  organization's policy if Windows displays a security warning.
- **Desktop app:** download the installer for your platform from the
  release page. The macOS `.dmg` is signed and notarized; the Windows
  installer is not yet signed (follow the security guidance above).
- **From crates.io:** the CLI/LSP packages are not yet published. Until registry
  installation is available, use a release download or the documented source
  installation. Library consumers can use a git dependency pinned to a reviewed
  release tag; see [Library Usage](book/src/chatter/integrating/library-usage.md).

To build the CLI from source instead, see the developer quick start
below.

## Quick start (for developers)

Start with [Contributor Setup](book/src/contributing/setup.md) for the selected
surface's prerequisites: workspace-selected stable Rust, native compiler/linker
tools, and any platform-specific libraries. A whole-workspace build also includes
the desktop app and its native/frontend requirements; it is not the minimum CLI
or library setup. Commands below use a Unix-style shell.

```sh
# Clone
git clone https://github.com/TalkBank/chatter.git
cd chatter

# Build the CLI without selecting the desktop workspace member
cargo build -p talkbank-chatter --locked

# Try the chatter binary
./target/debug/chatter --help
./target/debug/chatter validate path/to/file.cha
```

Choose focused tests through [Developer Verification
Checks](book/src/contributing/dev-checks.md). The root and `spec/` are separate
Cargo workspaces: root `cargo test --workspace` does not cover both. `just test`
runs root compiled test targets; doctests and final publication checks have
their own maintained recipes. A successful build is not a complete test run.

Build the docs locally after installing Python 3.11+ and `just`:

```sh
just book-install-tools
just book
# Output at book/build/
```

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the full development
setup and the coding conventions.

## Architecture at a glance

```mermaid
flowchart TD
    spec["spec/\n(CHAT format specification,\nsource of truth)"]
    grammar["grammar/\n(tree-sitter-talkbank grammar)"]
    parser["talkbank-parser\n(tree-sitter parser,\ngenerated typed CST visitor)"]
    re2c["talkbank-parser-re2c\n(independent CHAT backend)"]
    model["talkbank-model + talkbank-derive\n(data model, validation rules,\nerror codes, alignment)"]
    transform["talkbank-transform + talkbank-cache\n(CHAT to JSON, normalize,\nmerge, redact, pipelines)"]
    cli["chatter\n(CLI)"]
    lsp["talkbank-lsp\n(LSP server)"]
    desktop["chatter-desktop\n(Tauri desktop app)"]

    spec --> grammar
    grammar --> parser
    model --> parser
    model --> re2c
    parser --> transform
    re2c -. "cross-checks" .-> parser
    transform --> cli
    transform --> lsp
    transform --> desktop
```

Detailed architecture documentation: the
[architecture overview](book/src/architecture/overview.md).

## Toolchain

[`rust-toolchain.toml`](rust-toolchain.toml) selects stable Rust (edition 2024);
Chatter 1.0 supports current stable, with no older-compiler or fixed-MSRV promise. [Contributor
Setup](book/src/contributing/setup.md) owns the per-surface tool and platform
requirements, including grammar generation and desktop development.

## License

Dual-licensed under your choice of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

Unless explicitly stated otherwise, any contribution intentionally
submitted for inclusion in this work shall be dual-licensed as
above, without any additional terms or conditions.

## Related projects

- [Batchalign](https://github.com/TalkBank): the neural ML pipeline
  (ASR, forced alignment, neural morphotagging) built on top of
  chatter's crates; the two projects coordinate at the CHAT-format
  boundary
- [TalkBank](https://talkbank.org): the parent project; the CHAT format
  is the data substrate for every TalkBank corpus
