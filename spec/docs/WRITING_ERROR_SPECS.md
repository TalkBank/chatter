# Writing Error Specs, Quick Reference

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/spec/docs/WRITING_ERROR_SPECS.md)

Use [Spec Workflow](../../book/src/contributing/spec-workflow.md#adding-an-error-spec)
for the complete authoring and new-code bootstrap procedure, and
[Error Spec Format](ERROR_SPEC_FORMAT.md) for the format owners.

## Authoring sequence

1. Identify the rule and its existing code in `spec/codes/error-codes.toml`.
   For a new code, add its registry entry and follow the bootstrap sequence
   before writing Rust that references its generated variant.
2. Create `spec/errors/E{NNN}.md` or `E{NNN}_{suffix}.md`. Declare
   `code` and `name` in TOML frontmatter; each `[[example]]` declares
   its own `level`, required `claim` and complete CHAT `chat` block.
   Copy the current example structure from the canonical workflow.
3. Retain a legal control and a focused failing example. Justify the claim
   from the rule or a reviewed ruling, not merely from what the current
   implementation emits. Explain interactions and subsumption in prose.
4. Regenerate through the canonical workflow and inspect actual parser and
   validator observations separately from the authored claim. Run the
   affected behavioral checks; artifact currency alone is not correctness.

There is no `layer` field. `kind` and `status` belong to the code registry,
not the spec frontmatter. `level` belongs to each example, not the whole spec.
Unknown fields are load errors.

## Claims and diagnosis

- `claim = 'violates'`: the spec's own code must fire.
- `claim = 'legal'`: that code must not fire; this is not a declaration
  that every other rule accepts the transcript.
- `claim = { subsumed_by = 'E316' }`: the named alternative must fire
  and this spec's own code must not. The target here illustrates syntax;
  choose the actual target only from reviewed evidence.

If the observation contradicts the claim, investigate the rule, fixture and
implementation. Do not relabel the example or mark the code unimplemented to
make a test pass. Registry deferral does not exempt legal or subsumption
claims from their regression obligations.

An optional example `source` supplies the transcript stem used by
filename-sensitive checks. Preserve that identity when it matters.

## Commands and acceptance

From the repository root, `just spec-validate-examples` checks live example
claims and `just spec-coverage` reports spec/code coverage. They answer
different questions; neither proves complete CHAT coverage. Use the root
guidance and [Spec Workflow](../../book/src/contributing/spec-workflow.md)
for regeneration and proportional verification. Do not create an unrelated
temporary CHAT file as a substitute for the retained spec example.
