# Developer Verification Checks

**Status:** Current
**Last modified:** 2026-05-30 20:13 EDT

This page defines the current local verification expectations for
`TalkBank/chatter`.

There is **not yet** a repo-local `make verify` wrapper in this checkout. Use
the concrete commands below instead.

## Core local sweep

Run this from the repository root before opening or merging substantial changes:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo build --workspace --all-targets --locked
cargo nextest run --workspace
cargo test --doc
```

## Surface-specific additions

Add the checks that match the surface you changed:

- **Grammar changes**

  ```bash
  cd grammar && tree-sitter generate && tree-sitter test
  ```

- **Spec tooling changes**

  ```bash
  cargo build --manifest-path spec/tools/Cargo.toml
  cargo build --manifest-path spec/runtime-tools/Cargo.toml
  ```

- **Parser / model / alignment / serialization changes**

  ```bash
  cargo nextest run -p talkbank-parser-tests -E 'test(parser_equivalence)'
  cargo nextest run -p talkbank-parser-tests --test roundtrip_reference_corpus
  ```

See [Setup](setup.md) and [Spec Workflow](spec-workflow.md) for the
surface-specific regeneration guidance.

## When to Run

- Always before creating a PR.
- Always before merging parser, spec-tool, grammar, or generated-artifact
  changes.
- Again after rebasing if upstream changed the same surface.

## Additional Engineering Checks

Run these in addition to the core sweep when touching parser/model code:

1. `cargo test -p talkbank-parser --test test_parse_health_recovery`
2. `cargo nextest run -p talkbank-parser-tests --test parser_equivalence_files`

These protect against regressions in:

- parser recovery without sentinel fabrication
- parse-health taint propagation
- parser semantic equivalence

## Failure Policy

- If any required check fails, do not merge.
- Fix the failing check or scope down the change.
- If the failure is unrelated and pre-existing, document it in the PR and open a
  blocker issue.

## Recommended Fast Loop During Development

Use narrower loops while iterating, then run the full sweep before final review.
For a broad Rust verification pass:

```bash
cargo test --workspace
```

For grammar-only edits, prefer the smallest relevant loop first:

```bash
cd grammar && tree-sitter test
cargo nextest run -p talkbank-parser
```

Only reach for spec/symbol regeneration when the change truly affects generated
artifacts; do not treat regeneration as a substitute for choosing the right
regression test.
