# spec/tools - Core Generators Crate

**Status:** Current
**Last modified:** 2026-06-16 20:33 EDT

## Overview
Rust generators that turn CHAT specs into tests and documentation artifacts.
This crate lives in the separate `spec/Cargo.toml` workspace alongside
`spec/runtime-tools`, which owns runtime-aware bootstrap/mining/validation
tasks.

## Key Commands
```bash
# From repo root:
cargo run --manifest-path spec/tools/Cargo.toml --bin gen_tree_sitter_tests -- \
  --output-dir grammar/test/corpus \
  --template-dir spec/tools/templates

cargo run --manifest-path spec/tools/Cargo.toml --bin gen_rust_tests -- \
  --output-dir crates/talkbank-parser-tests/tests/generated

cargo run --manifest-path spec/tools/Cargo.toml --bin gen_validation_corpus -- \
  --corpus-dir crates/talkbank-parser-tests/tests/error_corpus/validation_errors

cargo run --manifest-path spec/tools/Cargo.toml --bin gen_error_docs
cargo test
```

## Binary Reference

### Core Workflow (used regularly by contributors)

| Binary | Purpose |
|--------|---------|
| `gen_tree_sitter_tests` | Generate tree-sitter corpus tests from `spec/constructs/` |
| `gen_rust_tests` | Generate Rust parser tests from `spec/errors/` |
| `gen_validation_corpus` | Generate the validation fixture corpus + `manifest.json` from `spec/errors/` (data-driven runner) |
| `gen_error_docs` | Generate error documentation from `spec/errors/` |
| `validate_spec` | Validate a single spec file |

### Analysis (useful for maintainers)

| Binary | Purpose |
|--------|---------|
| `corpus_node_coverage` | Report which tree-sitter node types are covered by the reference corpus |
| `gen_coverage_dashboard` | Generate HTML coverage dashboard |
| `coverage` | Report spec coverage statistics |

### Bootstrap / Migration (one-off tools, rarely needed)

| Binary | Purpose |
|--------|---------|
| `corpus_to_specs` | Migrate legacy `tests/error_corpus/` fixtures to spec format |
| `enhance_specs` | Batch-enhance specs with CHAT manual links and descriptions |
| `fix_spec_layers` | One-off migration to fix layer classifications |
| `perturb_corpus` | Generate perturbed corpus files for fuzz-like testing |

### Runtime-Aware Sibling Crate

`spec/runtime-tools` owns the tools that need the live Rust parser/model crates:
- `validate_error_specs`
- `bootstrap`
- `bootstrap_tiers`
- `extract_corpus_candidates`

## Architecture
```
src/
├── bin/           Entry points
├── spec/          Spec file loaders and parsers
├── output/        Output formatters (tree-sitter corpus, Rust tests, docs)
├── generated/     Generated symbol sets (do not edit)
└── templates/     Tera templates for wrapping test fragments in valid CHAT
```

## Testing
```bash
cargo test
```

## See Also
- [spec/CLAUDE.md](../CLAUDE.md): specification structure and workflows
- [spec/errors/README.md](../errors/README.md): error spec format reference
