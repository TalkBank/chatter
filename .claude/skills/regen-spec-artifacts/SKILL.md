---
name: regen-spec-artifacts
description: Regenerate generated tests, fixtures and registries after touching spec/constructs/ or spec/errors/ (adding a construct, an error-code spec, or editing spec examples). Use after ANY spec change.
allowed-tools: Bash, Read, Edit, Write, Grep, Glob
---

# Spec Artifact Regeneration

Specs are the source of truth; tests, fixtures and registries are GENERATED
from them and are never hand-written. Error-code tests flow ONLY through
`spec/errors/E###_*.md`.

## After any spec change

```bash
just spec-gen      # rewrite every committed artifact from the specs
just spec-check    # or ask whether the committed copies are current
```

That is the whole regeneration step. It covers all four committed artifacts:

| Artifact | Committed at |
|---|---|
| tree-sitter corpus tests | `grammar/test/corpus/generated/` |
| Rust test bodies | `crates/talkbank-parser-tests/tests/integration/generated/` |
| validation fixtures + `manifest.json` | `crates/talkbank-parser-tests/tests/error_corpus/validation_errors/` |
| `DiagnosticKind` registry | `crates/talkbank-model/src/errors/generated_diagnostic_kind.rs` |

Then:

```bash
timeout 600 cargo test -p talkbank-parser -p talkbank-parser-tests
```

If the grammar also changed, the grammar-change skill's sequence runs FIRST:
generate before testing anything.

## Things that used to need care and no longer do

- **There is no output path to pass, and no stray tree to check for.** Every
  destination is a constant in `spec/tools/src/artifacts.rs`. This step
  previously carried a warning that a defaulted `--output-dir` could create a
  root-level generated tree, which a contributor had to catch by reading
  `git status`. The registry made that unrepresentable rather than warned about.
- **You do not have to know which generator your change affects.** One command
  rebuilds everything, and the artifacts are byte-deterministic, so an
  unaffected one produces no diff.
- **You do not have to remember to run it.** The
  `every_generated_artifact_is_current` gate fails if a committed artifact
  disagrees with the specs. `just spec-check` is the same check, without
  waiting for the test binary.

## Not part of `spec-gen`

`gen_form_markers` has its own registry and its own drift gate
(`just form-markers-gen`).

## Where the rest lives

- Spec-file format, what every field does, what checks what:
  `book/src/architecture/spec-system.md`.
- The procedure, including the layer-migration checklist and auditing E316
  catch-alls: `book/src/contributing/spec-workflow.md`.
- Live state, derived rather than written down: `just spec-status`.

Reference corpus files must be valid CHAT (roundtrip gate); invalid examples
belong in `spec/errors/`.
