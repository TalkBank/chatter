# Grammar Patterns and Review

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/spec/docs/GRAMMAR_PATTERNS.md)

The [Grammar Workflow](../../book/src/contributing/grammar-workflow.md) owns
grammar editing, regeneration and affected verification. The grammar describes
executable syntax, including recovery shapes; acceptance by it is not semantic
CHAT validity.

## Follow the owning representation

- Edit `grammar/grammar.js`, not generated parser C or node metadata.
- Add reviewed construct/error examples through
  [Spec Workflow](../../book/src/contributing/spec-workflow.md).
- Let the generated typed traversal own CST structure and source association.
  Handwritten lowering should consume its associated carriers rather than
  reimplementing raw child-kind searches.
- Preserve missing, unexpected and recovered states. Kind or source admission
  does not prove syntactic completeness.
- Keep lexical policy and model validation at their existing owners; do not
  move a semantic rule into grammar recovery merely to satisfy an example.

[Parsing](../../book/src/architecture/parsing.md) describes the current
consumer migration boundary and recovery APIs. Do not infer full migration
from a successful path or a generated file's presence.

## Review a grammar change end to end

Review the intended syntax, generated CST shape, handwritten lowering,
diagnostics and typed serialization together. Inspect the real wrapped input
before writing an expected tree. Authored fragment CSTs are not automatically
compared with generated full-document CSTs.

Use the root's consolidated regeneration workflow, then focused behavioral
checks and explicit observation-diff adjudication. A stale generated parser,
a finite equivalence result or generated currency cannot establish complete
grammar correctness. Generator source access and independent regeneration
remain separate from ordinary builds using committed generated files.
