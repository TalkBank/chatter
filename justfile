# chatter: top-level recipes.
#
# Uniform shape across the workspace. More commands (cli, lsp, gui, docs)
# arrive in later staging sessions.

set shell := ["bash", "-c"]

# Book toolchain. mdBook + mdbook-mermaid are pinned to current and kept in
# lockstep across the justfile, ci.yml, and book.yml. mdbook-mermaid is a
# preprocessor (it rewrites fenced mermaid blocks) plus the mermaid.min.js and
# mermaid-init.js assets that book.toml loads via additional-js. Link-checking
# is decoupled onto lychee (runs on the built HTML, independent of mdBook).
mdbook_version := "0.5.3"
mdbook_mermaid_version := "0.17.0"
lychee_version := "0.24.2"
book_tools_root := justfile_directory() + "/.tooling/book-tools"
book_tools_bin := book_tools_root + "/bin"

# Default: list available recipes.
default:
    @just --list --justfile {{ justfile() }}

# Build the entire workspace (debug).
build:
    cargo build --workspace

# Build the entire workspace (release).
build-release:
    cargo build --workspace --release

# Run the full workspace test suite via cargo.
test:
    cargo test --workspace

# Line/region/function coverage over the whole workspace via cargo-llvm-cov,
# using the nextest runner (matches the project's test convention). Prints a
# per-crate summary plus a TOTAL row; the archived baseline number lives in
# the wind-down QC tracker. CI wiring is intentionally deferred to the public
# repo: instrumented builds are slow, and gating every push on coverage would
# burn Actions minutes for little signal.
#
# Coverage rebuilds and instruments every test binary, which is memory-heavy.
# On a memory-constrained machine cap the build parallelism with
# CARGO_BUILD_JOBS, e.g. `CARGO_BUILD_JOBS=4 just coverage`.
coverage:
    cargo llvm-cov nextest --workspace --summary-only

# Same coverage run, rendered as a browsable HTML report (local exploration
# of which lines are uncovered). Opens the report when it finishes.
coverage-html:
    cargo llvm-cov nextest --workspace --html --open

# Documentation gate: build the workspace docs with every rustdoc warning
# (missing docs, broken intra-doc links, private-item links, redundant link
# targets) promoted to an error, then run all doctests. The first-wave crates
# additionally carry `#![deny(missing_docs)]`, so a new undocumented public
# item fails the plain build too. Run this before relying on the docs being
# clean: CI does not yet check docs, so this local gate is what keeps the
# workspace rustdoc-clean (the state established 2026-06-13).
doc-check:
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace
    cargo test --doc --workspace

# Run clippy exactly as CI does (.github/workflows/ci.yml): two passes.
# Production code (lib + bins) is held strict; test targets get the panic /
# unwrap / expect lints relaxed, since tests may unwrap fixtures by convention.
# A single --all-targets pass would deny expect/unwrap in tests and diverge
# from CI (producing false positives), so this mirrors the two-pass split. The
# pre-push hook calls this, so `just clippy` and CI stay identical.
clippy:
    cargo clippy --workspace --lib --bins --locked -- -D warnings
    cargo clippy --workspace --tests --locked -- \
        -A clippy::unwrap_used \
        -A clippy::expect_used \
        -A clippy::panic \
        -A clippy::unreachable \
        -A clippy::todo \
        -A clippy::unimplemented \
        -D warnings

# Format the workspace.
fmt:
    cargo fmt --all

# Check formatting (CI-style; non-mutating).
fmt-check:
    cargo fmt --all -- --check

# Sync CI workflow Rust-version pins to their sources of truth
# (rust-toolchain.toml for the toolchain, Cargo.toml rust-version for the
# marked MSRV pin). Run this after bumping either file.
rust-sync:
    python3 scripts/sync-rust-versions.py --fix

# Verify the pins are in sync (CI-style; non-mutating).
rust-sync-check:
    python3 scripts/sync-rust-versions.py --check

# Sync the app version (tauri.conf.json, package.json) to the canonical
# [workspace.package] version in Cargo.toml. Run after bumping the version.
app-sync:
    python3 scripts/sync-app-version.py --fix

# Verify the app version is in sync everywhere (CI-style; non-mutating).
app-sync-check:
    python3 scripts/sync-app-version.py --check

# Lint GitHub Actions workflows locally (catches expression/action-input/shell
# errors WITHOUT pushing). Config in .github/actionlint.yaml. The default run
# is clean; if it reports something, fix it (do not suppress).
actionlint:
    actionlint

# Run the full CI gate (fmt + workflow lint + pin sync + two-pass clippy) and
# THEN push. Use this instead of `git push`: the gate runs before git opens its
# connection, so a long clippy run cannot stall the push past GitHub's SSH idle
# timeout (which is why clippy is not in the pre-push hook). The hook still runs
# the fast checks (fmt + actionlint) as a backstop.
push *ARGS:
    just fmt-check
    just actionlint
    just rust-sync-check
    just app-sync-check
    just clippy
    git push {{ARGS}}

# Regenerate symbol registry outputs for grammar and Rust consumers.
symbols-gen:
    node {{ justfile_directory() }}/spec/symbols/validate_symbol_registry.js
    node {{ justfile_directory() }}/spec/symbols/generate_grammar_symbol_sets.js
    node {{ justfile_directory() }}/spec/symbols/generate_rust_symbol_sets.js

# Check first-wave crates.io publication readiness for the foundation crates.
crates-io-foundation-check:
    bash {{ justfile_directory() }}/scripts/release/check-foundation-publication-readiness.sh --allow-dirty

# Install the pinned book toolchain into a repo-local root.
book-install-tools:
    cargo install \
      --root {{ book_tools_root }} \
      mdbook@{{ mdbook_version }} \
      mdbook-mermaid@{{ mdbook_mermaid_version }} \
      lychee@{{ lychee_version }} \
      --locked

# Build the book and link-check it with the repo-local pinned toolchain.
# mermaid renders diagrams; lychee validates internal links on the built
# HTML (--offline skips web links; --root-dir resolves the 404 page's '/').
book:
    PATH="{{ book_tools_bin }}:$PATH" mdbook build {{ justfile_directory() }}/book
    PATH="{{ book_tools_bin }}:$PATH" lychee --offline --root-dir {{ justfile_directory() }}/book/build {{ justfile_directory() }}/book/build

# Serve the book locally with the repo-local pinned mdBook toolchain.
book-serve:
    PATH="{{ book_tools_bin }}:$PATH" mdbook serve {{ justfile_directory() }}/book
