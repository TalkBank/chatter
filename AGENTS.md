# Chatter agent guidance

**Last modified:** 2026-09-10 00:45 EDT

Canonical guidance for all coding agents. `CLAUDE.md` imports this file; read
applicable nested guidance and the task-relevant references below.

## Scope and safety

- This is the canonical, self-contained CHAT format implementation. Keep
  grammar, spec, parsing, validation and generic transforms here. Corpus-specific
  workflow belongs downstream; use documented corpus-agnostic input seams.
- All content is public. Follow `CONTRIBUTING.md`; do not introduce private
  identities, paths, hosts, operational records or data.
- Keep the existing checkout and unrelated changes. Push/release needs explicit
  maintainer authorization, which persists within the stated scope. Squash
  unpublished commits since the last push; preserve published history. Never
  force-push, bypass hooks, push to `archive` or change repository visibility.
- Adjudicate diagnostics before changing data: distinguish tool defects from
  invalid input using the spec and real evidence. Recovery grammar acceptance
  is not validity. Do not change expectations merely to make a test pass;
  ask only when a semantic decision lacks a specification or ruling.
- Generated parser, traversal, registry, fixtures, schema and audit artifacts
  must be regenerated from their sources. Schema doc comments are inputs too.
  `release.yml` comes from cargo-dist; change its source configuration.
- No panics in long-lived code. Preserve the 16 MiB program thread and workspace
  lint inheritance. Use spec examples/reference fixtures, not ad hoc CHAT files.

## Development workflow

Authorized work includes implementation, investigation, review and verification;
no repeated design approval is needed. Use types to express invariants and
transitions. Validating constructors still need tests; keep policy, serialization
and external-boundary tests. Remove only tests whose checks are truly redundant.
Documentation/configuration edits receive proportional verification.

`just test` is the inner loop. Changes to grammar, spec, registries or generated
inputs require `just regen`, then tests. Review the final diff for reuse,
simplicity, efficiency, abstraction boundaries and typestate before committing.
Preserve the production-Rust evidence and breaking-changelog hooks.

Before an authorized push, run `just gate` on final content. Reuse its receipt
only while the checked inputs/tooling remain valid. `just test` omits doctests
and cannot replace the gate. Never run concurrent cargo-family commands in the
same workspace. Clippy and feature-off builds belong to `just release-lint`,
not an expanded per-push gate. Release sequence: `just fmt`,
`just release-lint`, `just gate`, reviewed squash of unpublished work, authorized
push, verified CI, then `just release-tag X.Y.Z`.

## Task references

| Task | Read |
| --- | --- |
| Build, generated files, commit/release gates and book | [Development](docs/agent-reference/development.md) |
| Typestate, AST ownership, cache keys and coding rules | [Design](docs/agent-reference/design.md) |
| Validity, architecture, cache policy, LSP and nested guidance | [Architecture and validity](docs/agent-reference/architecture-and-validity.md) |

This entry point resolves workflow conflicts in the references. Dated examples
are evidence, not current version or deployment claims. Generic skills do not
create extra approval gates or authorize publishing.
