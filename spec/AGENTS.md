# Specification agent guidance

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/spec/AGENTS.md)

The [repository guidance](../AGENTS.md) governs workflow and publication.
This file adds spec-specific ownership and review rules; it does not introduce
automatic checks or replace the contributor documentation.

## Canonical references

- [Spec System](../book/src/architecture/spec-system.md): schemas, claims,
  generated artifacts and their acceptance limits.
- [Spec Workflow](../book/src/contributing/spec-workflow.md): authoring,
  bootstrap, regeneration and verification.
- [Error Spec Format](docs/ERROR_SPEC_FORMAT.md): diagnostic taxonomy and
  rules that types cannot express.
- [Generator guidance](tools/AGENTS.md): implementation boundaries.

Use `just spec-status` when a live derived status is needed. Reuse retained
source-bound results where appropriate; a status report is not the authority
for changing a rule.

## Authored inputs and generated outputs

Specs own the examples and claims from which their registered tests, fixtures
and error-reference pages are generated. They are not the source of every
handwritten integration test or manual grammar case.

- `spec/codes/error-codes.toml` defines diagnostic codes, variants, documentation,
  kinds and implementation status. Spec files document a code; they do not
  duplicate its registry metadata.
- `spec/constructs/` holds valid fragment examples and reviewed expected CSTs.
  Derive expectations from real parsing and inspect them against the intended
  construct; do not invent trees or bless observed output without review.
- `spec/errors/` holds diagnostic examples with explicit `violates`, `legal`
  or `subsumed_by` claims. A legal claim means this rule does not fire, not
  that every rule accepts the transcript.
- `spec/tools/templates/` holds the fragment-wrapping templates. Follow the
  canonical workflow's format; do not maintain a second schema example here.
- `spec/observations/` records generated per-stage observations. Every changed
  observation needs adjudication; it does not rewrite the intended claim.

The generated artifact table in Spec System is derived from the owning
registry. Do not keep another hand-maintained destination list here.
`grammar/test/corpus/generated/` is generator-owned; unchanged files are
retained and obsolete files are pruned only after generated-directory admission.
It is **not wiped on every run**. The `.generated-output-dir` marker and
ownership/link checks protect that operation.
`grammar/test/corpus/manual/` is hand-maintained and never a generated
destination. Never hand-edit generated outputs or redirect a generator into a
shared authored tree.

## Review and verification

Use the canonical recipes from the repository root: `just spec-gen` owns
registered spec artifacts; `just spec-check` checks their currency without
rewriting them. Changes to generated inputs follow the complete `just regen`
workflow and affected behavioral verification described in the root guidance.
Documentation-only edits need proportional documentation checks, not regeneration.

Keep the spec workspace distinct from the root Cargo workspace. Explicit
manifest selection avoids running the wrong tests; the contributor workflow
documents `cargo test --manifest-path spec/Cargo.toml --workspace`.
Do not launch concurrent Cargo commands in the same workspace or interpret
artifact currency as behavioral, feature-completeness or release acceptance.

A fragment's authored `Expected CST` is not currently compared automatically
with the emitter's parsed full-document CST. Review that correspondence
explicitly. Generated tests and the observation snapshot are finite evidence,
not proof of complete CHAT coverage.

When a grammar change alters which diagnostics an example emits, review its
claim and rationale before changing either. Preserve valid controls and
reviewed subsumption cases. Do not delete an example merely because its original
diagnostic disappeared or change registry status to improve a count. Deferred
implementation does not excuse contradicted legal/subsumption claims.

Preserve unrelated dirty and staged work when a command fails. Investigate the
failure and repair owned inputs; do not automatically revert or delete outputs.
Publication and independent-operator acceptance remain separate from local
generation and testing.
