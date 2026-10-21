# Curating Construct Specs from Candidate Data

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/spec/docs/CURATION_WORKFLOW.md)

Candidate discovery and public spec authoring are separate steps. A corpus
occurrence, successful parser recovery or candidate-tool result is not a
standalone validity oracle.

## Discovery

Use `just spec-corpus-candidates` with an explicitly selected input scope;
inspect its current help for available options. Do not assume a sibling data
directory exists or run an unbounded corpus search. Keep candidate outputs
outside authored and generated release trees.

Use only material authorized for inspection. Private corpus content and
identifiers must not be copied into public specs, fixtures or documentation.
Retain any required provenance in its appropriate authorized location.

## Review and promotion

1. Identify the relevant construct and justify its intended behavior against
   the specification or a reviewed ruling.
2. Write a small synthetic example under `spec/constructs/` that preserves
   the important interaction without copying private transcript content.
3. Inspect the complete wrapped input's actual CST and review the relevant
   subtree before writing `Expected CST`. A tree produced by the current
   parser can still contradict the intended construct.
4. Follow [Spec Workflow](../../book/src/contributing/spec-workflow.md) for
   generation and focused verification.

The emitter currently derives a fragment's full-document CST from parsing;
it does not automatically compare that result with the authored fragment
expectation. Keep that manual correspondence review explicit.

Mutation candidates follow the separate
[seed and claim review](../../book/src/contributing/spec-workflow.md#mutation-candidates-are-not-golden-tests).
A mutation name does not establish an expected diagnostic.

## Ownership

- Candidate artifacts are unreviewed discovery material, not release fixtures.
- `spec/constructs/` and `spec/errors/` hold reviewed authored inputs.
- `grammar/test/corpus/generated/` is generator-owned.
- `grammar/test/corpus/manual/` remains hand-maintained.

Do not copy candidate CHAT directly into generated outputs, hand-edit those
outputs, or treat passing generation as full semantic acceptance.
