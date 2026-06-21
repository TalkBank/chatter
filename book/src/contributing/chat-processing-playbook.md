# CHAT Processing Playbook for Developers

**Status:** Current
**Last updated:** 2026-03-23 23:49 EDT

## Objective
Provide an implementation playbook for developers building or extending CHAT parsing,
validation, transformation, and serialization logic.

## Mental Model
Treat CHAT processing as a layered pipeline:
1. Ingest bytes and normalize line boundaries.
2. Parse syntax into structured model with exact spans.
3. Validate semantic rules with structured diagnostics.
4. Transform or enrich model without breaking invariants.
5. Serialize in canonical form.

## Developer Workflow
1. Start from a concrete fixture or corpus case.
2. Add/adjust parser behavior with contract tests first.
3. Add semantic validator rules separately from parser acceptance.
4. Confirm roundtrip and equivalence gates.
5. Update docs for any visible behavior or policy change.

## Tier Dispatch Strategy
Use cheap byte-prefix dispatch before heavy parsing:
- `@` => header candidate,
- `*` => main tier,
- `%` => dependent tier,
- continuation rules and whitespace handled deterministically.

This preserves performance and isolates error contexts earlier.

For downstream `batchalign3` consumers, tier dispatch is only the front door.
The important contract is what happens after dispatch: parse-health taint,
recovery vs rejection, and whether a tier is safe to pass into alignment.

## Word Parsing Rules of Thumb
- Parse suffix markers in strict order (`@...`, `@s...`, `$...`) with explicit precedence.
- Keep `raw_text` exact, `cleaned_text` policy-driven and test-locked.
- Treat CA delimiters and special symbols via centralized symbol sets.
- Never embed ad hoc symbol literals in multiple files.

## Error Handling Contract
- Every parser failure should produce structured diagnostics with:
  - code,
  - severity,
  - span,
  - context,
  - message.
- Avoid silent fallback behavior unless policy explicitly allows it.
- If fallback occurs, emit warning-grade diagnostics where relevant.
- Never fabricate semantic placeholders (empty required text, arbitrary enum default, fake word/chunk) to satisfy type construction.
- Prefer `None`/partial outcome + diagnostics over synthetic model values.

## Span Discipline
- Offsets are absolute across full file content.
- Nested parser helpers must accept base offset and return shifted spans.
- Add tests for boundary and continuation-line spans.

## Performance Policy
- Prefer byte-oriented prechecks for top-level dispatch and simple delimiters.
- Use parser combinators for structural parsing, not for obvious constant-prefix routing.
- Measure parser performance on representative corpus slices before/after major changes.

## Common Failure Patterns and Fixes
- Symptom: semantic mismatch only in snapshots.
  - Fix: compare parser outputs directly and isolate first structural delta.
- Symptom: generated tests pass, corpus fails.
  - Fix: add missing fixture, decide parse-vs-validate placement, lock behavior.
- Symptom: output drift after grammar edit.
  - Fix: run full regeneration and equivalent parser contract suite before merge.

## Batchalign3 Surface Checks

When a change affects the surface used by `batchalign3`, confirm:

- full-file parse equivalence still holds for corpus coverage
- alignment-sensitive downstream tiers still gate on parse-health appropriately

## Review Checklist for Parser PRs
- New or changed behavior has targeted tests.
- Equivalence suite status is attached.
- Snapshot updates are intentional and explained.
- No hidden magic symbols or magic string literals introduced.
- Docs updated where user-visible behavior changes.

## Required Artifacts for Significant Changes
- Design note (architecture decision record in the book).
- Before/after examples.
- Impacted fixtures list.
- Migration implications for integrators.
