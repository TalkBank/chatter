# Parser implementation guidance

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/crates/talkbank-parser/PARSING_PATTERNS.md)

The maintained parsing guidance lives in the book. Start with:

- [Parser, model, and API contracts](../../book/src/architecture/parser-model-contracts.md)
  for recovery versus admission, diagnostic source identity, and boundary review.
- [Parser backends](../../book/src/architecture/parser-backends.md)
  for whole-file and fragment APIs and backend compatibility limits.
- [Library usage](../../book/src/chatter/integrating/library-usage.md)
  for source-included examples rather than copied API snippets.
- [Spec workflow](../../book/src/contributing/spec-workflow.md)
  for authored regression cases and generated artifacts.

Use the repository [agent guidance](../../AGENTS.md) and
[development reference](../../docs/agent-reference/development.md) for verification
and release policy. This routing page does not define a separate parser workflow.
