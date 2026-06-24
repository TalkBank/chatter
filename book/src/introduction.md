# Introduction

**Status:** Current
**Last modified:** 2026-06-21 21:33 EDT

[TalkBank](https://talkbank.org/) is the world's largest open repository of spoken language data. This repository (`TalkBank/chatter`) is the standalone home of the CHAT format authority and the `chatter` tool family: the `chatter` CLI, the Rust crates for parsing/validation/transformation, the `tree-sitter-talkbank` grammar, the `talkbank-lsp` language server, and the desktop validation app.

`chatter` is publicly released. To get it right away:

- **Command-line tool** (macOS / Linux): `curl --proto '=https' --tlsv1.2 -LsSf https://github.com/TalkBank/chatter/releases/latest/download/chatter-installer.sh | sh` (Windows and other options: [Install](install/index.md)).
- **Desktop app**: download for your platform from the [latest release](https://github.com/TalkBank/chatter/releases/latest).
- **Full installation guide** (all platforms, package details): [Install](install/index.md).

The Rust crates are source-available from this repository (not yet published to
crates.io). As a 0.x release, APIs and flags may change before 1.0.

## Choose the right surface

| Task | Recommended Surface |
|---|---|
| **CHAT validation, normalization, or conversion** | `chatter` CLI |
| **LSP integration in editors** | `talkbank-lsp` standalone |
| **Build CHAT tooling in Rust** | Rust crates (`talkbank-model`, `talkbank-parser`, etc.) |
| **Reuse grammar in other tools** | `tree-sitter-talkbank` |
| **Standalone desktop GUI for CHAT validation** | Chatter Desktop (`apps/chatter-desktop/`) |

## What's In This Repo

- **`chatter` CLI**: validate, convert, normalize, and analyze CHAT files from the command line, with an interactive TUI for corpus-scale workflows
- **Language Server (LSP)**: works with any LSP-compatible editor (Neovim, Emacs, Helix, Zed, etc.) to provide live validation and cross-tier alignment
- **JSON data model**: every CHAT structure as typed JSON with lossless roundtrip fidelity, backed by a published JSON Schema
- **Rust API**: parse, validate, inspect, and transform CHAT files programmatically via library crates

## Who This Book Is For

| Audience | Start Here | Then Go To |
|---|---|---|
| **CLI users** validating, normalizing, or converting CHAT | [Install](install/index.md) | [chatter Quick Start](chatter/user-guide/quick-start.md), [CLI Reference](chatter/user-guide/cli-reference.md) |
| **Rust library consumers** parsing or transforming CHAT | [Library Usage](chatter/integrating/library-usage.md) | crate-root rustdoc for `talkbank-model`, `talkbank-parser`, and `talkbank-transform` |
| **Grammar / format consumers** embedding CHAT parsing in other tools | [CHAT Format Overview](chat-format/overview.md) | `tree-sitter-talkbank` docs and the grammar/reference chapters |
| **Contributors / maintainers** working in this repo | [Contributing setup](contributing/setup.md) | [CI and release](contributing/ci-and-release.md) |

## Repository Layout

```text
grammar/        Tree-sitter grammar for CHAT
spec/           Source of truth: CHAT specification + error specs
crates/         Rust crates for model, parser, transform, cache, CLI, LSP, tests, and FFI support
apps/           Tauri v2 desktop app (`chatter-desktop`)
corpus/         Reference corpus (must stay 100% valid under the regression gate)
schema/         JSON Schema for the CHAT AST
tests/          Integration tests and fixtures
fuzz/           Fuzz testing targets (separate Cargo workspace)
docs/           Strategy docs, proposals, and investigations for this repo
book/           This documentation (mdBook)
```

Data flows: **spec** (source of truth) → **grammar** (tree-sitter) → **Rust crates** (parsers, model, validation, CLI, LSP) → **applications** (`chatter`, desktop app).
