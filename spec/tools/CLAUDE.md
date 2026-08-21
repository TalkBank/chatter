# spec/tools - Core Generators Crate

**Status:** Current
**Last modified:** 2026-08-21 10:03 EDT

## Read the book first

[`book/src/architecture/spec-system.md`](../../book/src/architecture/spec-system.md)
is the authoritative description of the spec system and
[`book/src/contributing/spec-workflow.md`](../../book/src/contributing/spec-workflow.md)
is the procedure. `just spec-status` reports the live state. This file is the
crate-level detail only.

## Overview
Rust generators that turn CHAT specs into tests and documentation artifacts.
This crate lives in the separate `spec/Cargo.toml` workspace alongside
`spec/runtime-tools`, which owns corpus candidate selection and live parser/model validation
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

## Binaries

`just --list` names every wired spec command with the question it answers, and
`ls spec/*/src/bin` shows everything that exists. **This section used to carry
four tables**, and between them they omitted `ca_census` and `spec_status` while
four other files in the tree carried their own differing copies of the same
list. `spec/docs/ERROR_SPEC_FORMAT.md` now holds the taxonomy, which is the part
that is not derivable.

What is worth knowing HERE, because it is about this crate rather than about the
list:

- **`spec/tools` must stay usable without the live parser/model crates.** That
  is the whole reason for the split: ordinary spec generation should not pull
  runtime parser dependencies. Anything needing the live `ErrorCode` enum, the
  parser or the model belongs in the sibling `spec/runtime-tools`, which is why
  `spec_gen` has a half in each.
- **`fix_spec_layers` is DELETED** (R4, 2026-08-21). It rewrote an authored
  `layer` field by running the parser; the field is gone and which stage
  catches a rule is observed in `spec/observations/` instead.

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
