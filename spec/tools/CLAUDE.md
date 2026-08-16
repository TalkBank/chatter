# spec/tools - Core Generators Crate

**Status:** Current
**Last modified:** 2026-08-16 12:39 EDT

## Read the book first

[`book/src/architecture/spec-system.md`](../../book/src/architecture/spec-system.md)
is the authoritative description of the spec system and
[`book/src/contributing/spec-workflow.md`](../../book/src/contributing/spec-workflow.md)
is the procedure. `just spec-status` reports the live state. This file is the
crate-level detail only.

## Overview
Rust generators that turn CHAT specs into tests and documentation artifacts.
This crate lives in the separate `spec/Cargo.toml` workspace alongside
`spec/runtime-tools`, which owns runtime-aware bootstrap/mining/validation
tasks.

## Key Commands
```bash
# From repo root:
just spec-gen      # regenerate every artifact derived from spec/
just spec-check    # report staleness, writing nothing

cargo test
```

Generation lives in `src/artifacts.rs`, one `Artifact` row per committed
artifact, carrying its destination as a constant and a `build` that returns the
files rather than writing them. That is what lets the gate compare without
writing, and what stops a generator being pointed at the wrong directory.

## Binary Reference

### Core Workflow (used regularly by contributors)

| Binary | Purpose |
|--------|---------|
| `spec_gen` (in `spec/runtime-tools`) | Every artifact in the registry: tree-sitter corpus tests, Rust test bodies, the validation fixture corpus + `manifest.json`, and the `DiagnosticKind` registry. `just spec-gen` / `just spec-check`. |
| `gen_form_markers` | Generate the model enum, re2c code set and book table from the form-marker registry (`just form-markers-gen`) |
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
- `spec_gen` (the whole registry; its own half needs the live `ErrorCode` enum)
- `validate_error_specs`
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
