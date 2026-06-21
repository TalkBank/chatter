# Curating Construct Specs from Mined Data

**Last modified:** 2026-05-29 18:36 EDT

This project uses a strict two-stage pipeline:

1. Mine candidate files from corpus data directory (staging only).
2. Curate small constructed examples into `spec/constructs/` (source of truth).

Do not copy mined `.cha` files directly into release corpus outputs.

## Why

- Mined files are useful for discovery, not for stable publishable tests.
- Curated specs stay small, understandable, and reviewable.
- Generated tree-sitter tests are reproducible from specs only.

## Workflow

1. Mine candidates:

```bash
cargo run --bin extract_corpus_candidates --manifest-path spec/runtime-tools/Cargo.toml -- \
  --data-dir ../data \
  --languages eng \
  --node-types grammar/src/node-types.json \
  --max-lines 200 \
  --max-files 20000 \
  --top 50 \
  --require-rust-parse=true \
  --require-rust-validation=true \
  --validate-alignment=true \
  --json \
  --output spec/tmp/mined/candidates.eng.json
```

2. Curate by hand from those candidates:
- Identify one minimal representative pattern per construct.
- Write or update markdown files in `spec/constructs/*`.
- Prefer minimal examples over raw corpus copies.

3. Regenerate tests:

```bash
cargo run --manifest-path spec/tools/Cargo.toml --bin gen_tree_sitter_tests -- \
  --output-dir grammar/test/corpus \
  --template-dir spec/tools/templates

cargo run --manifest-path spec/tools/Cargo.toml --bin gen_rust_tests -- \
  --output-dir crates/talkbank-parser-tests/tests/generated
```

4. Verify:

```bash
cd grammar && tree-sitter test --overview-only
cargo nextest run -p talkbank-parser-tests
```

## Staging vs release

- Staging artifacts: `spec/tmp/mined/*` (ephemeral, non-release).
- Source of truth: `spec/constructs/*` and `spec/errors/*`.
- Generated release artifacts: `grammar/test/corpus/*`.
