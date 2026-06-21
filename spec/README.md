# spec, CHAT Specification

**Last modified:** 2026-05-29 18:36 EDT

## Overview

Markdown specification files define valid constructs and error cases for CHAT.
`spec/tools/` turns these specs into tree-sitter corpus tests, Rust tests, and
documentation. Runtime-aware bootstrap and validation helpers live in the
sibling `spec/runtime-tools/` crate.

**Specs are the source of truth.** Generated artifacts should never be edited
by hand.

## Structure

```
spec/
├── constructs/           Valid CHAT examples (164 specs)
│   ├── header/           Header constructs
│   ├── main_tier/        Main tier constructs
│   ├── tiers/            Dependent tier constructs
│   ├── utterance/        Utterance-level constructs
│   └── word/             Word-level constructs
├── errors/               Error specs (197 files, 181 error codes)
├── symbols/              Shared symbol registry (JSON + generators)
├── tools/                Core generator crate in the spec workspace
│   ├── src/bin/          Spec-to-artifact entry points
│   └── templates/        Tera templates for wrapping test fragments
├── runtime-tools/        Runtime-aware bootstrap/validation tooling
│   └── src/bin/          Live parser/model-aware entry points
└── docs/                 Format reference and guides
    ├── ERROR_SPEC_FORMAT.md   ← Comprehensive spec format reference
    └── WRITING_ERROR_SPECS.md ← Quick workflow guide
```

## Key Commands

```bash
# This repo does not have a root `make test-gen` wrapper.
# Regenerate the tree-sitter corpus tests:
cargo run --manifest-path spec/tools/Cargo.toml --bin gen_tree_sitter_tests -- \
  --output-dir grammar/test/corpus \
  --template-dir spec/tools/templates

# Regenerate the generated Rust parser tests (constructs + parser-layer errors):
cargo run --manifest-path spec/tools/Cargo.toml --bin gen_rust_tests -- \
  --output-dir crates/talkbank-parser-tests/tests/generated

# Regenerate the validation fixture corpus + manifest.json (validation-layer errors):
cargo run --manifest-path spec/tools/Cargo.toml --bin gen_validation_corpus -- \
  --corpus-dir crates/talkbank-parser-tests/tests/error_corpus/validation_errors

# Regenerate local error-reference pages from error specs:
cargo run --manifest-path spec/tools/Cargo.toml --bin gen_error_docs

# Validate spec format
cargo run --manifest-path spec/runtime-tools/Cargo.toml --bin validate_error_specs

# Check error coverage
cargo run --manifest-path spec/tools/Cargo.toml --bin coverage \
  -- --spec-dir spec --errors
```

## Current Coverage

| Metric | Count |
|--------|-------|
| Construct specs | 164 |
| Error specs (total) | 197 files |
| Error codes covered | 181/181 (100%) |
| Error specs with CHAT examples | 169 |
| Documented stubs (untriggerable) | 12 |

## Workflows

See `docs/ERROR_SPEC_FORMAT.md` for the complete format reference, including
metadata fields, layer semantics, code block info strings, and template usage.

See `docs/WRITING_ERROR_SPECS.md` for the practical step-by-step workflow.
See `docs/CURATION_WORKFLOW.md` for the mine -> curate -> generate workflow for construct specs.

## See Also

- `tools/CLAUDE.md`: Core generator crate details
- `runtime-tools/`: Runtime-aware spec tooling
- `CLAUDE.md` (spec directory), AI assistant guidance
- `../crates/talkbank-parser-tests/CLAUDE.md`: Parser test crate

---
