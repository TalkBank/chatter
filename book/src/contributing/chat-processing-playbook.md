# CHAT Processing Playbook for Developers

**Status:** Current
**Last updated:** {{git-dates:page}}

Use this playbook when changing parsing, validation, transformation or
serialization. The [parser/model contract](../architecture/parser-model-contracts.md)
defines the boundaries; [Library Usage](../chatter/integrating/library-usage.md)
provides source-included executable recipes.

## Preserve the source, then choose the boundary

1. Retain the original UTF-8 bytes. Do not normalize line endings or whitespace
   before parsing when diagnostics, edits or unchanged output must refer to
   that source.
2. Parse with the supported API. Keep both the product and its diagnostics;
   a built model is not necessarily valid.
3. Select the required admission policy. Distinguish invalid input, incomplete
   evidence and internal tool failure.
4. Transform only through the appropriate typed boundary. Mutation invalidates
   any proof whose inputs it changes.
5. Validate the required output properties, then serialize or return admitted
   original bytes according to the operation's contract.

Serialization is not a universal source-preservation operation. Conversely,
returning unchanged bytes does not certify validity unless those exact bytes
were admitted. Do not pair cached model evidence with unrelated source.

## Begin with an observed defect

Use a public reference fixture or an authored spec example with a clear
expected outcome. Adjudicate parser versus data defects before editing data.
Do not copy private transcript material into public fixtures or documentation.

Map the connected producer/consumer paths before implementing a repair. State
which invariant should become unforgeable and which checks remain meaningful
at external boundaries. Keep genuine constructor, policy and serialization
tests; a type-level restriction is not permission to delete all tests around it.

For syntax or rule changes, update the source-owned spec and grammar as needed,
regenerate through `just regen`, review the generated differences, and run the
focused contracts. Do not update snapshots solely because the new code emits
different output. See [Spec Workflow](spec-workflow.md).

## Diagnostics and recovery

Handle `ParseProduct` and streaming `ParseOutcome` according to their actual
contracts. Streaming diagnostics are separate from the value; retain the sink's
evidence. Internal failure must remain distinguishable from a CHAT violation,
even if its diagnostic severity is not an error.

Do not replace failed admission with empty required text, guessed enum values,
fake words or zero timestamps. Regressions must inspect the model or serialized
output when fabrication is the defect; checking only that a diagnostic fired
is insufficient. Preserve recoverable surrounding content without admitting the
malformed part as a valid semantic value.

Use checked source/presentation APIs for diagnostics. Not every failure has a
valid source location. Do not fabricate spans merely to satisfy a renderer.
See the [diagnostic contract](../chatter/integrating/diagnostic-contract.md).

## Coordinates and words

Source offsets belong to their source snapshot. Fragment origins, continuation
lines, CRLF, multibyte text and edits are separate boundary cases. Do not
double-shift offsets or index a fragment string with document coordinates.

Word `raw_text` is derived from typed structure, while `cleaned_text` follows
its documented projection. Neither is an independent lexical authority or a
substitute for source bytes. See [computed word fields](../chatter/integrating/json-output.md#computed-fields).
Use existing symbol registries and typed marker/annotation policies instead of
parallel character scanners that imitate CHAT parsing.

## Verify the property that changed

| Defect | Necessary observation |
| --- | --- |
| Wrong acceptance or rejection | Authored legal/invalid expectation, diagnostics and applicable policy |
| Wrong model structure | Actual typed structure or semantic comparison with an authored expectation |
| Lost or invented content | Retained source/model relationship and actual serialized output |
| Wrong coordinates | Source-bound spans and the relevant client coordinate conversion |
| Backend omission of an error | Supported API, isolated invalid control and valid counterpart |
| CLI/LSP/desktop boundary failure | Actual command or protocol boundary, not only an internal helper |

A second parser is evidence, not an oracle. Equal recovery ASTs are not required
merely because both backends reject the same invalid input. A successful roundtrip
does not prove source validity, linguistic correctness or completeness.

Run scoped feedback after a coherent repair batch. Use the
[development checks](dev-checks.md) for the final publication gate and release
sequence; do not add new routine gates from this playbook. Performance changes
need relevant measurements, not blanket byte-dispatch or combinator rules.

## Handoff

Record the exact source and test scope, observed result, remaining limits and
consumer-facing behavior changes. Explain failures rather than hiding skipped
or zero-test selections behind a successful shell exit. Update the owning
documentation and changelog when behavior changes; do not create a second
fixture registry or copy generated reference material into prose.
