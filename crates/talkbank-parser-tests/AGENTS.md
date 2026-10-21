# Parser-tests crate agent guidance

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/crates/talkbank-parser-tests/AGENTS.md)

Read the [root guidance](../../AGENTS.md) first. This file adds the test
crate's ownership rules and explains its gates; it does not create separate
gates of its own beyond those registered in `src/gate.rs`.

## What this crate is

The one crate that depends on every parser and the model, so it is where a
test that needs CHAT text turned into a model belongs. `tests/integration/`
is a single integration binary of many modules (spec-derived corpora,
fragment contracts, recovery, diagnostics, transform corpora, public error
types); `tests/check_parity/` holds the closed CHECK assessment manifest;
`tests/error_corpus/` and `tests/parser_suite/` are the generated spec
fixtures and the parser suite. `src/` holds the gates, the conformance
inventory, snapshot and fixture helpers, and the `from_source` builders that
turn reference text into models for other tests.
Reference: [testing](../../book/src/contributing/testing.md),
[correctness architecture](../../book/src/contributing/correctness-architecture.md),
[quality gates](../../book/src/contributing/quality-gates.md).

## Where a test goes

- A test over a parsed model goes here, named for the contract it pins
  (`*_from_source.rs`, `*_corpus.rs`). A test over a value no parser produces
  stays with the type that admits it, in its own crate.
- Inputs come from `spec/` examples (lowered to fixtures by `just regen`) and
  `corpus/reference/`; never an ad hoc `.cha`. Every error code tests through
  `spec/errors/`. A re2c-versus-tree-sitter divergence is a missing spec.
- A test that would pass by stating its input twice (building the AST it then
  judges) is what the fabricated-AST ratchet exists to refuse; parse the
  input instead.
- Snapshots (`insta`) are adjudicated in the commit, never regenerated
  blindly; `INSTA_FORCE_PASS` hides a mismatch, so check for `.snap.new`
  files after any run that sets it.

## The gates

`src/gate.rs` registers every gate in `ALL`; `tests/integration/gates.rs`
runs each one over the real tree, then plants each declared violation and
requires the gate to fail for the planted reason. A gate reads only through
the `Tree` it is handed (`gate_discipline` checks this), so a probe can plant
into it. A limit a probe cannot reach is an `UnprovenRule`, mirrored in
`gates.rs`, and the mirror test fails when the two lists drift.

Three gates are ratchets with reviewed baselines held in a `const`:

| Gate | Baseline | Counts |
| --- | --- | --- |
| `fabricated_ast` | `CEILING` per crate | `new_unchecked` and `from_checked_literal` in code, not comments or strings |
| `fabricated_ast` | `PARSER_UNLOCATED_CEILING` | production `Location::Unlocated` in the parser crates (tree-sitter 0; re2c measured), text before the first `#[cfg(test)]` |
| `content_catch_alls` | `UNPROTECTED` | content-enum `_ =>` arms in files not yet under the wildcard lint |
| `error_code_demonstration` | `UNDEMONSTRATED` | codes whose own spec file does not demonstrate them |

A ratchet fails in both directions: above the ceiling is a regression, below
it is unbanked progress, and a row for a crate the walk never saw is a
rename or deletion nobody recorded. Lower a ceiling in the commit that earned
it; the gate prints the replacement row verbatim. There is no `--write`.

## Verification

`cargo test -p talkbank-parser-tests --test integration <filter>` is the
focused loop; the unfiltered target includes the gates and the spec corpora.
`just test` runs the workspace. The reference-corpus combination coverage
gate is required at pre-push and declines to judge a checkout without the
corpus. Coverage captures (`just coverage`, `just coverage-attribution`)
measure canonical public workflows; generated code is reported separately.
