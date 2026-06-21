# Setup

**Status:** Current
**Last modified:** 2026-06-15 15:00 EDT

Development is supported on **Windows, macOS, and Linux**. The instructions below use Unix shell syntax; on Windows, use PowerShell or Git Bash equivalently.

## Prerequisites

- **Rust (stable)** via [rustup](https://rustup.rs/) (all platforms)
- **Node.js** for tree-sitter grammar generation and symbol validation
- **tree-sitter CLI**: `cargo install tree-sitter-cli`
- **just** (optional but recommended) for the repo's top-level helper recipes

## Clone Repository

```bash
mkdir -p ~/talkbank && cd ~/talkbank
git clone https://github.com/TalkBank/chatter.git
cd chatter
```

## Build

From your chatter checkout root:

```bash
cargo build --workspace --locked
cargo build --workspace --all-targets --locked

# Optional helpers from the root justfile
just build
just test
just book-install-tools
just book
```

## Two Cargo Workspaces

The repository has two independent Cargo workspaces:

### 1. Root workspace (`Cargo.toml`)

Contains all Rust crates for parsing, model, validation, and transform:

```bash
cargo build
cargo test
```

### 2. Spec workspace (`spec/Cargo.toml`)

Contains two sibling crates for spec-driven artifacts. Invoke with
`--manifest-path` relative to the chatter repo root:

```bash
cargo build --manifest-path spec/tools/Cargo.toml
cargo build --manifest-path spec/runtime-tools/Cargo.toml
cargo run --manifest-path spec/tools/Cargo.toml --bin gen_tree_sitter_tests -- --help
cargo run --manifest-path spec/runtime-tools/Cargo.toml --bin validate_error_specs -- --help
```

## Root justfile recipes

```bash
just build        # Build the Rust workspace
just build-release
just test         # cargo test --workspace
just clippy
just fmt
just fmt-check
```

## Verification

This repo does **not** currently have the old monorepo-wide `make verify`
wrapper ported into the root checkout. Until that lands, use the concrete
verification commands from the repo guidance:

```bash
cargo fmt
cargo check --workspace --all-targets
cargo build --workspace --all-targets --locked
cargo nextest run --workspace
cargo test --doc
```

Add grammar/spec commands when your change touches those surfaces:

```bash
cd grammar && tree-sitter generate && tree-sitter test
cargo build --manifest-path spec/tools/Cargo.toml
cargo build --manifest-path spec/runtime-tools/Cargo.toml
```

CI green on the pushed commit remains the authoritative pre-push gate for this
staging repo.

## Editor Setup

### rust-analyzer

The workspace should work out of the box with rust-analyzer. The root `Cargo.toml` workspace configuration is standard.
