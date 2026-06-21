# Spec Tooling and Generation Pipeline

**Status:** Current
**Last updated:** 2026-05-19 17:38 EDT

## Objective
Make `spec/` the reliable language-contract source while keeping generation
deterministic, maintainable, and appropriately scoped.

The goal is to separate:

- grammar artifact generation
- validation/error-doc generation
- parser semantic testing (fragment and full-file)

Anything that still looks like bootstrap-era synthetic fragment orchestration
is now audit-only unless a doc says it remains operational.

## Open structural concerns

- `spec/tools` still carries bootstrap-era Rust parser/model
  dependencies that create circular or awkward workflow coupling.
- Contributor workflows still over-assume that `make test-gen` is
  the right reaction to every parser-related change.

## Current Generation Pipeline

```text
spec constructs/errors
  -> spec validators
  -> generated grammar corpus tests
  -> generated rust parser/validation tests
  -> generated error docs
  -> coverage dashboards and quality reports
```

That pipeline is still useful, but it is too broad to remain the single mental
model for parser testing.

## Desired Post-Bootstrap Split

```text
grammar specs/templates
  -> generated tree-sitter corpus tests

error specs
  -> generated validation/parser error tests
  -> generated error docs

fragment semantic fixtures and invariants
  -> fragment-level parser tests

reference corpus / curated full files
  -> parser parity tests
```

## Structural Reorganization for `spec/tools` (proposed, not yet implemented)

The intent here is to narrow `spec/tools`'s mission back to spec-driven
artifact generation and validation rather than leaving it as a
bootstrap-era staging ground for parser semantics. A proposed module
split:

- `input` (markdown/spec parsing)
- `ir` (normalized internal representation)
- `emit` (grammar tests, rust tests, docs)
- `validate` (schema and semantic checks)
- `sync` (grammar node-types and symbol-registry checks)

Current layout (`crates: bin/, generated/, lib.rs, output/, spec/,
templates/`) has not been migrated to this shape. Treat this section
as a design target for future work rather than a description of the
current source tree.

## Legacy vs Active

Keep these active:

- grammar corpus generation
- error doc generation
- symbol registry sync/validation
- affected regeneration when a spec or grammar input truly changed

Treat these as legacy audit paths:

- synthetic tree-sitter fragment wrappers
- bootstrap-era parser equivalence rituals

## Determinism Requirements
1. Stable ordering of generated outputs.
2. Stable formatting of generated code/docs.
3. Re-runs without source changes produce no diffs.

## Drift Prevention Controls
- Node type compatibility check:
  - `spec/tools` must compile and run against current generated node constants.
- Registry compatibility check:
  - all symbol categories used in specs and grammar must be known in registry.
- Generation integration check:
  - full generation pass with clean tree must produce zero diff.
- Boundary check:
  - generated grammar/docs flows should not silently become the sole authority
    for fragment parsing semantics.

## Authoring Experience (proposed, not yet implemented)

Spec authoring would benefit from:

- Strict but simple spec templates for constructs and errors.
- A `spec lint` command for immediate feedback (missing fields,
  invalid tags, malformed examples, unknown error codes).
- Clearer documentation of when `make test-gen` is actually needed and
  when a small direct test is the right answer instead.

The `spec lint` binary does not yet exist; the strict-validation work
that exists today happens implicitly through `make test-gen` failures
plus the spec validators in `spec/tools/src/bin/`.

## Versioning and Metadata
Each spec file should include:
- ownership,
- status (`draft`, `accepted`, `deprecated`),
- parser/validation scope,
- linked tests and generated outputs.

## Acceptance Criteria
- `spec/tools` is green and deterministic.
- Every generation target has explicit provenance from source specs.
- Drift between node types, specs, and generators is blocked in CI.
- Spec contributors have a documented and automated happy path.
- Small grammar changes no longer force a giant regeneration ritual by default.
- Fragment parsing semantics are tested outside the generation pipeline.
