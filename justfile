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
    cargo test --workspace --tests --locked

# The full TEST set: compiled tests, doctests, both workspaces, the UI suite.
#
# NOT the pre-push gate, which is `just gate` and includes this. This recipe
# used to say "what CI runs; use before pushing", which was true of the tests
# and false of everything else CI does.
#
# `--tests` above restricts the first pass to compiled test targets, because a
# bare `cargo test` ALSO runs doctests: without it, `test` and this recipe both
# ran the whole doctest suite twice over. Doctests are merged into one binary
# per crate (edition 2024), so they are cheap to RUN; the cost is the rustdoc
# compile, which is what the inner loop skips.
test-all: test check-feature-off test-spec
    cargo test --doc --workspace
    cargo test -p talkbank-derive --features ui-tests --tests ui_tests

# The spec workspace, which `--workspace` above does not reach.
#
# `spec/tools` generates the tree-sitter corpus tests, the Rust parser tests
# and the validation fixture corpus, so a break here stops the regeneration
# every other gate depends on. It went untested by CI and by this justfile
# until 2026-08-04.
test-spec:
    cargo test --manifest-path spec/Cargo.toml --workspace

# The `validation-runner`-off configuration of talkbank-transform (the
# SQL-free surface downstream consumers opt into with
# `default-features = false`). No in-workspace consumer builds it, so
# without this gate feature unification would let it rot silently.
#
# ITS OWN TARGET DIRECTORY, DELIBERATELY. `--no-default-features` changes
# feature unification across the whole shared dependency graph, so sharing
# `target/` with every other recipe makes the two evict each other. That is the
# same "two cargo unit configurations, one target dir, no reuse" trap as
# running `cargo check` before `cargo test`.
#
# Measured on ming, 2026-08-04, crates recompiled per alternation:
#
#                          shared target/     own target dir
#   check-feature-off        68  (17.7 s)      1  ( 1.7 s)
#   the next `just test`    133  (43.3 s)      5  (20.2 s)
#
# So an alternation went from 201 crate rebuilds to 6, and from about 61 s to
# 22 s. The cost is disk: target/feature-off is 1.4 GB beside target/'s 13 GB,
# plus one cold build of it (181 crates, 21 s).

check-feature-off:
    CARGO_TARGET_DIR=target/feature-off cargo test -p talkbank-transform --no-default-features --tests

# Line/region/function coverage over the whole workspace via cargo-llvm-cov,
# using cargo test (matches the project's test convention). Prints a
# per-crate summary plus a TOTAL row; the archived baseline number lives in
# the wind-down QC tracker. CI wiring is intentionally deferred to the public
# repo: instrumented builds are slow, and gating every push on coverage would
# burn Actions minutes for little signal.
#
# Coverage rebuilds and instruments every test binary, which is memory-heavy.
# On a memory-constrained machine cap the build parallelism with
# CARGO_BUILD_JOBS, e.g. `CARGO_BUILD_JOBS=4 just coverage`.
coverage:
    cargo llvm-cov --workspace --summary-only

# Same coverage run, rendered as a browsable HTML report (local exploration
# of which lines are uncovered). Opens the report when it finishes.
coverage-html:
    cargo llvm-cov --workspace --html --open

# The only test that runs BOTH parser backends over the real corpus, comparing
# parsed MODELS rather than verdicts. Named here rather than left as a command
# in a doc comment, because a test whose invocation is folklore is a test that
# does not get run: this one had been `#[ignore]`d with a retired default corpus
# path, so the two ways to invoke it were "wrong" and "not at all".
#
# Not in `test-all`: it needs a corpus no CI runner has, and it reads about
# 100,000 files with two parsers. Set TALKBANK_DATA to override the location.
# It FAILS rather than skips when there is no corpus, since you only ever run it
# on purpose.
corpus-parse-equivalence:
    cargo test -p talkbank-parser-re2c --test integration --release \
        -- --ignored --nocapture full_corpus_parse_equivalence

# Every test that needs the wild corpus, which is every `#[ignore]`d test in
# the re2c integration binary. They are ignored because no CI runner has a
# corpus, and a test that cannot run should be absent rather than green: before
# this was made explicit, thirteen of them ran in the default suite and either
# printed "Skipping" or worked on an empty input set, both of which cargo
# reports as a pass.
#
# Set TALKBANK_DATA to point somewhere other than ~/0tb/data.
corpus-tests:
    cargo test -p talkbank-parser-re2c --test integration --release -- --ignored

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
# from CI (producing false positives), so this mirrors the two-pass split.
# CI owns clippy (see CLAUDE.md clippy policy); run locally only when
# working ON clippy findings.
clippy:
    # Single pass: production strictness lives in the workspace [lints]
    # table, which reaches a crate only because that crate declares
    # `[lints] workspace = true`; test relaxation lives in-source (crate-root
    # cfg_attr for unit tests, a crate-root allow block for each integration
    # test target). One flag set = one build profile.
    cargo clippy --workspace --all-targets --locked

# Format BOTH workspaces.
#
# `cargo fmt --all` means "every member of THIS workspace", not "every crate in
# the repository", and `spec/` is a separate workspace. Formatting only the root
# left spec/ ungated from the day it was split out: by 2026-08-04 nine of its
# files had drifted, and nothing ever said so.
fmt:
    cargo fmt --all
    cargo fmt --manifest-path spec/Cargo.toml --all

# Check formatting (CI-style; non-mutating). Both workspaces, same reason.
fmt-check:
    cargo fmt --all -- --check
    cargo fmt --manifest-path spec/Cargo.toml --all -- --check

# Sync CI workflow Rust-version pins to their sources of truth
# (rust-toolchain.toml for the toolchain, Cargo.toml rust-version for the
# marked MSRV pin). Run this after bumping either file.
rust-sync:
    python3 scripts/sync-rust-versions.py --fix

# Verify the pins are in sync (CI-style; non-mutating).
rust-sync-check:
    python3 scripts/sync-rust-versions.py --check

# Sync the app version (package.json) to the canonical [workspace.package]
# version in Cargo.toml. Run after bumping the version. (tauri.conf.json has no
# version field by design; the desktop bundle inherits the crate version.)
app-sync:
    python3 scripts/sync-app-version.py --fix

# Verify the app version is in sync everywhere (CI-style; non-mutating).
app-sync-check:
    python3 scripts/sync-app-version.py --check

# Bump the release version EVERYWHERE in one command: the canonical
# [workspace.package] version, all internal path-dep pins, package.json, and
# both lockfiles. The one remaining manual step (deliberately) is writing the
# `## [X.Y.Z]` CHANGELOG section; the check gates enforce it. Then: commit,
# `just push`, wait for CI, `just release-tag X.Y.Z`.
release-bump VERSION:
    python3 scripts/sync-app-version.py --bump {{VERSION}}
    cargo update --workspace
    cargo update --workspace --manifest-path spec/Cargo.toml

# Tag and push vX.Y.Z, fail-closed: refuses on a dirty tree, an unpushed
# HEAD, any version-copy drift, a missing CHANGELOG section, or CI not yet
# green on this exact commit. Mechanizes away the tag-races-CI failure mode
# (v0.5.0, 2026-07-30).
release-tag VERSION:
    bash scripts/release-tag.sh {{VERSION}}

# Lint GitHub Actions workflows locally (catches expression/action-input/shell
# errors WITHOUT pushing). Config in .github/actionlint.yaml. The default run
# is clean; if it reports something, fix it (do not suppress).
actionlint:
    actionlint

# Regenerate the tree-sitter parser and fail if the committed output moved.
#
# The ONLY check that catches a stale `parser.c`. The traversal staleness guard
# hashes `grammar.json` and `node-types.json`, so a regeneration that changes
# only `parser.c` passes it correctly; a tree-sitter version bump does exactly
# that. A guard proves what it hashes.
grammar-generate-check:
    cd grammar && tree-sitter generate && git diff --exit-code src/parser.c src/grammar.json src/node-types.json

# The grammar's own corpus tests and editor queries.
grammar-test:
    cd grammar && tree-sitter test
    cd grammar && for q in queries/*.scm; do tree-sitter query "$q" ../corpus/reference/edge-cases/postcodes-and-gems.cha >/dev/null || exit 1; done

# Clippy over the spec workspace, which `just clippy` does not reach.
clippy-spec:
    cargo clippy --manifest-path spec/Cargo.toml --all-targets --locked

# The model and re2c parser must keep compiling for wasm32 (no C toolchain).
wasm-check:
    cargo check -p talkbank-model -p talkbank-parser-re2c --target wasm32-unknown-unknown --locked

# Every tracked shell script, at shellcheck's default severity.
shellcheck:
    bash scripts/lint/shellcheck-all.sh

# Dependency policy.
deps-check:
    cargo deny --locked check

# Point git at the tracked hooks. Run once per clone.
#
# `.git/hooks` does not survive a clone, so an untracked hook is a gate that
# exists on exactly one machine. The one this replaces was untracked, ran two
# checks, and printed "fast gate passed" while letting a broken push through.
install-hooks:
    git config core.hooksPath .githooks
    @echo "hooks installed: pre-push now requires a gate stamp (see just gate)"

# Assert CI and the gate cannot describe different checks.
ci-gate-sync:
    python3 scripts/check_ci_gate_sync.py

# THE FAST HALF of the gate: everything that fails in seconds.
#
# Ordered so a typo costs seconds, not the twelve minutes the slow half takes.
# Run this constantly; it is cheap enough to have no excuse.
gate-fast:
    just fmt-check
    just actionlint
    just ci-gate-sync
    just rust-sync-check
    just app-sync-check
    just doc-dates
    just shellcheck
    just verify-vendored-lexer
    just grammar-generate-check
    just grammar-test
    just wasm-check
    just deps-check
    just book
    @printf '%s\n' "$(git status --porcelain | shasum | cut -d' ' -f1)" > .git/gate-fast-passed

# THE SLOW HALF: compilation, tests, lints, the book. 12-15 minutes.
#
# Most of it is rustdoc building one merged doctest binary per crate. That is
# inherent, and it is the half that catches what `just test` cannot see.
gate-slow:
    just test-all
    just clippy
    just clippy-spec
    @printf '%s\n' "$(git status --porcelain | shasum | cut -d' ' -f1)" > .git/gate-slow-passed

# THE PRE-PUSH GATE: both halves, which is everything CI runs that one machine
# can run.
#
# It is split because the whole thing exceeds this project's 900-second command
# ceiling, so an agent cannot invoke it in a single call and would otherwise be
# pushed toward running some-but-not-all by hand, which is the exact failure
# this gate exists to end. Run `gate-fast` then `gate-slow` when a ceiling
# binds; run `gate` when nothing stops you.
#
# The test set is delegated to `test-all` rather than restated. An earlier cut
# inlined it and immediately produced two recipes both claiming to be "what CI
# runs", already diverged in both directions.
#
# What is NOT here: the cross-platform matrix, which needs three operating
# systems. `just ci-gate-sync` fails if anything else drifts out.
gate: gate-fast gate-slow
    @echo "[gate] both halves passed; stamps written for this tree"

# Gate, then push. Use this instead of `git push`.
#
# The gate runs BEFORE git opens its connection, deliberately: a long run
# between connection and transfer can stall the push past GitHub's SSH idle
# timeout. That is also why the pre-push hook holds only the fast checks.
push *ARGS:
    just gate
    git push {{ARGS}}

# Regenerate symbol registry outputs for grammar and Rust consumers.
symbols-gen:
    node {{ justfile_directory() }}/spec/symbols/validate_symbol_registry.js
    node {{ justfile_directory() }}/spec/symbols/generate_grammar_symbol_sets.js
    node {{ justfile_directory() }}/spec/symbols/generate_rust_symbol_sets.js

# Fail when a doc's `Last modified` header is older than the doc itself.
#
# A ratchet, not a sweep: `scripts/doc-dates-baseline.txt` records the pages
# already stale when this was introduced (116 of them, 56 in the book), and the
# check fails on any NEW one and on any baseline entry that has been fixed but
# left listed, so the list can only shrink. Do not bulk-stamp dates to empty it;
# read the page first.
doc-dates:
    python3 {{ justfile_directory() }}/scripts/check_doc_dates.py
# Regenerate every artifact derived from spec/ (tests, fixtures, registries).
#
# One command for what used to be four hand-typed `cargo run --manifest-path`
# invocations, each carrying its own `--output-dir`. The destinations are
# constants in the registry now, so a generator cannot be aimed at the wrong
# tree. Review the diff before committing.
spec-gen:
    cargo run --quiet --manifest-path {{justfile_directory()}}/spec/Cargo.toml --bin spec_gen

# Report whether every generated artifact is current. Writes nothing.
#
# This is what `every_generated_artifact_is_current` runs; use it for the same
# answer without waiting for the test binary.
spec-check:
    cargo run --quiet --manifest-path {{justfile_directory()}}/spec/Cargo.toml --bin spec_gen -- --check


# What state is the spec system in? Derived from the same code the gates use.
#
# Answers the questions that used to need a grep: how many specs there are and
# what they declare, how many examples are verified, how many are DEFERRED, how
# many assert nothing at all, the CLAN CHECK parity counts, and which gate
# checks which artifact.
spec-status:
    cargo run --quiet --manifest-path {{ justfile_directory() }}/spec/Cargo.toml --bin spec_status

# Regenerate every site that carries the CHAT form-marker inventory.
#
# Loading the registry validates it, so there is no separate validate step: a
# generator cannot run over an unchecked registry. The gate that fails when a
# committed artifact disagrees is `generated_form_marker_sites_are_current` in
# spec/tools, which calls these same renderers.
form-markers-gen:
    cargo run --manifest-path {{ justfile_directory() }}/spec/Cargo.toml --bin gen_form_markers

# Verify the committed re2c lexer matches lexer.re (and everything it includes).
#
# There is NO CI job for this: no workflow installs re2c, so this is the only
# check that exists, and it has to be run by hand after any change to lexer.re
# or to the generated form-marker code set it includes. It takes under a second.
verify-vendored-lexer:
    #!/usr/bin/env bash
    set -euo pipefail
    cd {{ justfile_directory() }}/crates/talkbank-parser-re2c
    regenerated="$(mktemp)"
    trap 'rm -f "$regenerated"' EXIT
    re2rust -W -Wno-nondeterministic-tags --input-encoding utf8 --utf8 \
        --no-generation-date --conditions -o "$regenerated" src/lexer.re
    if cmp -s "$regenerated" src/generated/lexer.rs; then
        echo "vendored lexer is current"
    else
        echo "STALE: src/generated/lexer.rs does not match src/lexer.re." >&2
        echo "Regenerate it with the invocation in build.rs, in the same commit." >&2
        exit 1
    fi

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
