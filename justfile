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
mdbook_version := "0.5.4"
mdbook_mermaid_version := "0.17.1"
lychee_version := "0.24.2"
book_tools_root := justfile_directory() + "/.tooling/book-tools"

# The spec workspace is a SECOND cargo workspace, so every spec binary needs its
# manifest named. This was spelled out per recipe and had drifted into two
# spellings of `justfile_directory()`. Two recipes stay expanded on purpose and
# cannot use this: `spec-ca-census` adds `--release` (it runs over a corpus) and
# `form-markers-gen` deliberately omits `--quiet`. Do not "tidy" either into
# `spec_run`; doing so silently drops what makes them different.
#
# Recipe descriptions use the `[doc("...")]` attribute where the comment block
# is more than one line, because `just --list` shows only the LAST comment line
# and would otherwise render a sentence fragment.
spec_run := "cargo run --quiet --manifest-path " + justfile_directory() + "/spec/Cargo.toml --bin"
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
test-all: test test-spec
    cargo test --doc --workspace
    cargo test -p talkbank-derive --features ui-tests --tests ui_tests

# The spec workspace, which `--workspace` above does not reach.
#
# `spec/tools` generates the tree-sitter corpus tests, the Rust parser tests
# and the validation fixture corpus, so a break here stops the regeneration
# every other gate depends on. It went untested by CI and by this justfile
# until 2026-08-04.
[doc("Run the spec workspace's own tests.")]
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
# Measured on the development workstation, 2026-08-04, crates recompiled per alternation:
#
#                          shared target/     own target dir
#   check-feature-off        68  (17.7 s)      1  ( 1.7 s)
#   the next `just test`    133  (43.3 s)      5  (20.2 s)
#
# So an alternation went from 201 crate rebuilds to 6, and from about 61 s to
# 22 s. The cost is disk: target/feature-off is 1.4 GB beside target/'s 13 GB,
# plus one cold build of it (181 crates, 21 s).

[doc("Build with default features off, which CI does and `just test` cannot.")]
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

# Every test that needs a real corpus on disk, which is every `#[ignore]`d test in
# the re2c integration binary. They are ignored because no CI runner has a
# corpus, and a test that cannot run should be absent rather than green: before
# this was made explicit, thirteen of them ran in the default suite and either
# printed "Skipping" or worked on an empty input set, both of which cargo
# reports as a pass.
#
# Requires TALKBANK_DATA, set to a directory holding the `*-data` corpus repos.
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
    #
    # `-D warnings` because an EXIT CODE OF ZERO BESIDE A WARNING IS NOT A
    # GATE. This recipe exited 0 with 28 warnings on 2026-08-26, four of them
    # introduced by the commit immediately before, and nobody read them; the
    # same blindness one layer down had let a Rust BINDING pattern that matched
    # every child ship, reported by rustc at warn level, into a maintainer's
    # transcript. A warning we intend to keep gets a scoped `#[allow]` naming
    # the reason, which is a decision in the source rather than noise in a log.
    cargo clippy --workspace --all-targets --locked -- -D warnings

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

# Regenerate the node-type constants and fail if the committed output moved.
#
# `node_types.rs` says "DO NOT EDIT THIS FILE MANUALLY" and names this script.
# The script was left behind in talkbank-tools when the CHAT core was extracted
# to chatter on 2026-05-29 and deleted there on 2026-06-21 as a "dead CHAT-core
# script", so for three months the banner named a generator this repo did not
# have. Nothing could regenerate and nothing checked, and the file drifted to 8
# missing kinds and 6 constants for kinds the grammar no longer had. Recovering
# the script is only half the fix; this is the half that keeps it true.
node-types-check:
    #!/usr/bin/env bash
    set -euo pipefail
    # Generate to a TEMPORARY file and compare, never `>` straight over the
    # tracked one. The redirection truncates BEFORE node runs, so a generator
    # that crashes leaves an EMPTY `node_types.rs` in the working tree, and the
    # failure then surfaces from `git diff` as "the file changed" instead of
    # from node as "the generator broke". `grammar-generate-check` above does
    # not have this shape only because `tree-sitter generate` writes its own
    # outputs rather than being redirected into a tracked path.
    generated="$(mktemp)"
    trap 'rm -f "$generated"' EXIT
    node scripts/generate-node-types.js > "$generated"
    if ! diff -u crates/talkbank-parser/src/node_types.rs "$generated"; then
        echo "error: node_types.rs is stale. Regenerate with:" >&2
        echo "  node scripts/generate-node-types.js > crates/talkbank-parser/src/node_types.rs" >&2
        exit 1
    fi
    # The docs file is the HAND-WRITTEN half, and it drifted too: it carried
    # six entries for kinds the grammar no longer had, the same six the Rust
    # file did. Gating only the generated output would keep checking the half
    # nobody edits.
    node scripts/check-node-type-docs.js

# The grammar's own corpus tests and editor queries.
grammar-test:
    cd grammar && tree-sitter test
    cd grammar && for q in queries/*.scm; do tree-sitter query "$q" ../corpus/reference/edge-cases/postcodes-and-gems.cha >/dev/null || exit 1; done

# Clippy over the spec workspace, which `just clippy` does not reach.
clippy-spec:
    cargo clippy --manifest-path spec/Cargo.toml --all-targets --locked -- -D warnings

# The model and re2c parser must keep compiling for wasm32 (no C toolchain).
wasm-check:
    cargo check -p talkbank-model -p talkbank-parser-re2c --target wasm32-unknown-unknown --locked

# Every tracked shell script, at shellcheck's default severity.
shellcheck:
    bash scripts/lint/shellcheck-all.sh

# Prove the breaking-change changelog gate fires, and stays quiet when it
# should. Seven cases against a throwaway repo, well under a second.
breaking-changelog-test:
    bash scripts/lint/test-breaking-needs-changelog.sh

# Prove the red-evidence commit gate fires, and stays quiet when it should.
#
# The gate refuses a commit that changes production Rust while staging no test,
# spec, corpus or fixture beside it, unless the message names what was red in a
# `Red:` trailer. Twelve cases, both directions, including the near miss that
# the sibling gate's test taught: a filename merely CONTAINING "test"
# (`attests.rs`) is production code, not a test.
evidence-gate-test:
    bash scripts/lint/test-production-rust-needs-evidence.sh

# RELEASE-TIME REPORT, deliberately not a gate: name every breaking commit
# after REF that did not touch CHANGELOG.md. Its verdict depends on history
# rather than on one commit, so a developer cannot make it pass before pushing.
# Read it while preparing a release; the commit-msg hook is what prevents new
# ones. Default REF is the newest reachable tag.
breaking-changelog ref=`git describe --tags --abbrev=0`:
    bash scripts/lint/breaking-needs-changelog.sh --since {{ref}}

# Dependency policy.
deps-check:
    cargo deny --locked check

# Point git at the tracked hooks. Run once per clone.
#
# `.git/hooks` does not survive a clone, so an untracked hook is a gate that
# exists on exactly one machine. The one this replaces was untracked, ran two
# checks, and printed "fast gate passed" while letting a broken push through.
#
# REPORTS WHAT IT DISPLACES; see `scripts/report-displaced-hooks.sh`, which is
# what a person actually reads at the moment it matters.
install-hooks:
    git config core.hooksPath .githooks
    @scripts/report-displaced-hooks.sh
    @echo "hooks installed: pre-push now requires a gate stamp (see just gate)"

# Assert CI and the gate cannot describe different checks.
ci-gate-sync:
    python3 scripts/check_ci_gate_sync.py

# THE ONE PRE-PUSH GATE. Static checks plus every test CI runs, one stamp.
#
# It used to be two halves, `gate-fast` and `gate-slow`, and the slow half
# compiled the same code four or five times (tests, clippy, a feature-off
# build in its own target dir, and a second cargo workspace for spec/, each
# a separate cargo unit sharing no artifacts): 10 to 13 minutes, run nine
# times in one day. Clippy and the feature-off build are release-time now
# (`release-lint`), and per-push CI runs exactly what this runs, so a green
# gate here IS a green CI. `git rev-parse --git-dir`, never a literal `.git`:
# in a worktree `.git` is a file.
gate:
    just fmt-check
    just actionlint
    just ci-gate-sync
    just rust-sync-check
    just app-sync-check
    just doc-dates
    just shellcheck
    just breaking-changelog-test
    just evidence-gate-test
    just verify-vendored-lexer
    just grammar-generate-check
    just node-types-check
    just grammar-test
    just wasm-check
    just deps-check
    just book
    just test-all
    @bash scripts/tree-stamp.sh > "$(git rev-parse --git-dir)/gate-passed"

# RELEASE-TIME LINT. Run before the release squash, never per push: each of
# these is a separate cargo unit that recompiles the workspace, and a finding
# in any of them is a defect to fix in the same session, not a gate on daily
# work. `release-lint.yml` runs the same recipe on a tag.
release-lint:
    just fmt-check
    just clippy
    just clippy-spec
    just check-feature-off

# Gate, then push. Use this instead of `git push`.
#
# The gate runs BEFORE git opens its connection, deliberately: a long run
# between connection and transfer can stall the push past GitHub's SSH idle
# timeout. That is also why the pre-push hook only READS the gate's stamp and
# runs no checks of its own.
push *ARGS:
    just gate
    git push {{ARGS}}

# Regenerate symbol registry outputs for grammar and Rust consumers.
# The generator list is DISCOVERED, not written here. The drift gate
# (`generated_symbol_sets_are_current`) globs the same `generate_*.js`, so a
# generator cannot be gated-but-never-run, which is what a hand-written list one
# recipe away from a glob eventually produces.
[doc("Regenerate every artifact derived from the symbol registry.")]
symbols-gen:
    node {{ justfile_directory() }}/spec/symbols/validate_symbol_registry.js
    for g in {{ justfile_directory() }}/spec/symbols/generate_*.js; do node "$g" || exit 1; done

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
# REGENERATE EVERY DERIVED ARTIFACT, in dependency order, in ONE command.
#
# Seven artifacts are derived from the grammar, the spec and the registries,
# each with its own currency test that fails the gate when it is stale. Run one
# at a time they are discovered SERIALLY: on 2026-08-27 a single grammar edit
# took six full gate runs, each failing on the next stale artifact
# (traversal, node types, inventory, observation snapshot, ...). After any
# change under grammar/, spec/ or the registries: `just regen`, then
# `just test`, once.
regen:
    cd grammar && tree-sitter generate
    node scripts/generate-node-types.js > crates/talkbank-parser/src/node_types.rs
    just traversal-gen
    just symbols-gen
    just form-markers-gen
    cargo run --quiet -p talkbank-parser-tests --example gen_conformance_inventory
    just spec-gen

# The typed CST traversal, from a CLEAN tree-sitter-grammar-utils checkout.
# The generator stamps its own `git describe` into the output header and the
# currency test refuses `-dirty`, so a dirty generator checkout is refused
# here rather than discovered after the run. Edition and toolchain are read
# from this workspace's own manifests, never restated.
traversal-gen:
    #!/usr/bin/env bash
    set -euo pipefail
    tsgu="${TSGU_DIR:-$HOME/tree-sitter-grammar-utils}"
    if [ ! -d "$tsgu" ]; then
        echo "error: tree-sitter-grammar-utils not found at $tsgu (set TSGU_DIR)" >&2
        exit 1
    fi
    if [ -n "$(git -C "$tsgu" status --porcelain)" ]; then
        echo "error: $tsgu has uncommitted changes; the generator would stamp -dirty" >&2
        exit 1
    fi
    edition=$(sed -n 's/^edition = "\(.*\)"/\1/p' Cargo.toml)
    toolchain=$(sed -n 's/^channel = "\(.*\)"/\1/p' rust-toolchain.toml)
    (cd "$tsgu" && cargo build --quiet --release --example generate_typed_traversal -p tree-sitter-node-types)
    "$tsgu/target/release/examples/generate_typed_traversal" \
        grammar/src/grammar.json grammar/src/node-types.json \
        --edition "$edition" --toolchain "$toolchain" \
        > crates/talkbank-parser/src/generated_traversal.rs

[doc("Regenerate every artifact derived from spec/.")]
spec-gen:
    {{ spec_run }} spec_gen

# Report whether every generated artifact is current. Writes nothing.
#
# This is what `every_generated_artifact_is_current` runs; use it for the same
# answer without waiting for the test binary.
[doc("Are the committed generated artifacts current with the specs?")]
spec-check:
    {{ spec_run }} spec_gen -- --check


# What state is the spec system in? Derived from the same code the gates use.
#
# Answers the questions that used to need a grep: how many specs there are and
# what they declare, how many examples are verified, how many are DEFERRED, how
# many are deferred, the CLAN CHECK parity counts, and which gate
# checks which artifact.
[doc("What state is the spec system in?")]
spec-status:
    {{ spec_run }} spec_status

# Spec coverage: which codes have specs, which specs demonstrate their own
# code, and which specs nothing distinguishes.
#
# The undemonstrated list is the one to act on: an entry declaring E316 means
# the mined input does not parse, so the rule is unreachable and the fix is in
# the parser; a specific other code usually means the fixture is simply wrong.
[doc("Which codes have specs, and which specs demonstrate their own code.")]
spec-coverage:
    {{ spec_run }} coverage -- --errors --spec-dir {{ justfile_directory() }}/spec

# Do the error specs' examples actually produce the codes they declare?
#
# Runs the live parser and validator over every example, which is the question
# no amount of reading the spec files can answer.
[doc("Do the error specs' examples produce the codes they declare?")]
spec-validate-examples:
    {{ spec_run }} validate_error_specs -- --check-codes

# Which grammar node types does `corpus/reference/` actually exercise?
#
# A renderer; the computation is `generators::node_coverage` and has its own
# test. Use it when adding grammar rules, to see what the corpus never reaches.
[doc("Which grammar node types does the reference corpus exercise?")]
spec-node-coverage:
    {{ spec_run }} corpus_node_coverage

# A per-mark attestation census for Conversation Analysis notation.
#
# CA is the one region of CHAT chatter has never specified, and the rules it has
# were each added because something broke. This measures what transcribers
# actually do, per mark, so the specification can be written from evidence.
# Reads CHAT meaning only from the typed AST.
[doc("Per-mark attestation census for CA notation. Takes a corpus root.")]
spec-ca-census *ARGS:
    cargo run --quiet --release --manifest-path {{ justfile_directory() }}/spec/Cargo.toml --bin ca_census -- {{ ARGS }}

# Generate error-triggering CHAT by perturbing valid corpus files.
#
# The adversarial half of spec work: CHECK parity is found by CONSTRUCTING
# invalid input chatter wrongly accepts, never by running over valid corpora.
[doc("Generate error-triggering CHAT by perturbing valid corpus files.")]
spec-perturb *ARGS:
    {{ spec_run }} perturb_corpus -- {{ ARGS }}

# Find representative CHAT files in the data repos, for corpus curation.
spec-corpus-candidates *ARGS:
    {{ spec_run }} extract_corpus_candidates -- {{ ARGS }}

# Regenerate every site that carries the CHAT form-marker inventory.
#
# Loading the registry validates it, so there is no separate validate step: a
# generator cannot run over an unchecked registry. The gate that fails when a
# committed artifact disagrees is `generated_form_marker_sites_are_current` in
# spec/tools, which calls these same renderers.
[doc("Regenerate every site carrying the CHAT form-marker inventory.")]
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
    expected="$(python3 -c 'import tomllib; print(tomllib.load(open("{{ justfile_directory() }}/re2c-version.toml", "rb"))["re2c"]["version"])')"
    actual="$(re2rust --version | awk '{print $2}')"
    if [[ "$actual" != "$expected" ]]; then
        echo "error: re2rust $actual is installed; expected exactly $expected from re2c-version.toml" >&2
        exit 1
    fi
    cd {{ justfile_directory() }}/crates/talkbank-parser-re2c
    regenerated="$(mktemp)"
    trap 'rm -f "$regenerated"' EXIT
    re2rust -W -Wno-nondeterministic-tags --input-encoding utf8 --utf8 \
        --no-generation-date --conditions -o "$regenerated" src/lexer.re
    if cmp -s "$regenerated" src/generated/lexer.rs; then
        echo "vendored lexer is current under re2rust $actual"
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

# Prove that the mdbook and mdbook-mermaid actually on PATH (the repo-local
# root first, then whatever CI installed) are the pinned versions. Without
# this a stale repo-local install is invisible: on 2026-09-01 the pin said
# 0.5.x while `.tooling/book-tools` still held mdbook 0.4.52, and the only
# symptom was the git-dates preprocessor not running (0.4.x gives it no cwd).
book-tools-check:
    #!/usr/bin/env bash
    set -euo pipefail
    export PATH="{{ book_tools_bin }}:$PATH"
    fail=0
    for pair in "mdbook={{ mdbook_version }}" "mdbook-mermaid={{ mdbook_mermaid_version }}" "lychee={{ lychee_version }}"; do
        tool="${pair%%=*}"; want="${pair#*=}"
        if ! command -v "$tool" >/dev/null 2>&1; then
            echo "book-tools-check: $tool is not installed; run: just book-install-tools" >&2; fail=1; continue
        fi
        got="$("$tool" --version 2>/dev/null | head -n 1 | sed -E 's/^[^0-9]*([0-9]+\.[0-9]+\.[0-9]+).*$/\1/')"
        if [ "$got" != "$want" ]; then
            echo "book-tools-check: $tool is $got, the pin is $want; run: just book-install-tools" >&2; fail=1
        fi
    done
    exit "$fail"

# Build the book and link-check it with the repo-local pinned toolchain.
# mermaid renders diagrams; lychee validates internal links on the built
# HTML (--offline skips web links; --root-dir resolves the 404 page's '/').
# The git-dates preprocessor (book.toml) stamps every page with git-derived
# "last changed" dates; its tests run first, and `verify` then proves the
# rendered front page carries the same dates git reports, so a build in which
# the preprocessor silently did not run cannot pass. Needs full git history.
# mdbook runs with the book directory as its cwd (never `mdbook build book`
# from the repo root): 0.4.x hands a preprocessor no cwd of its own, so the
# `../scripts/...` command in book.toml only resolves from inside book/.
book: book-tools-check
    python3 -m unittest {{ justfile_directory() }}/scripts/test_mdbook_git_dates.py
    cd {{ justfile_directory() }}/book && PATH="{{ book_tools_bin }}:$PATH" mdbook build
    python3 {{ justfile_directory() }}/scripts/mdbook_git_dates.py verify --book-root {{ justfile_directory() }}/book --page introduction.md {{ justfile_directory() }}/book/build/index.html
    PATH="{{ book_tools_bin }}:$PATH" lychee --offline --root-dir {{ justfile_directory() }}/book/build {{ justfile_directory() }}/book/build

# Serve the book locally with the repo-local pinned mdBook toolchain.
book-serve: book-tools-check
    cd {{ justfile_directory() }}/book && PATH="{{ book_tools_bin }}:$PATH" mdbook serve
