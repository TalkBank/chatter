+++
code = 'E768'
name = '@Media filename cannot be written and read back unchanged'
kind = 'Invalidity'
status = 'unreachable_from_chat'
status_note = "The rule IS implemented and does fire; `unreachable_from_chat` says only that no `.cha` input can trigger it, so this spec carries no `## Example` and owes a named out-of-corpus test instead. That test is `media_filename_from_json_is_reported` in `talkbank-transform`'s integration tests, which drives the path that does reach the rule. This spec is why the status exists. It first shipped as `implemented` with no example, which made `parse_markdown` reject it and `load_all` downgrade the rejection to a stderr warning, so the spec vanished from the corpus generator and the `implemented_codes_without_examples` gate, whose entire job is to catch an implemented rule shipping untested, never saw it. Both halves are now fixed: the loader fails closed on a spec it cannot parse, and this state is representable instead of being faked with a `Status` that lies."
+++

## Description

`@Media` names a file, then a comma, then the media type. The filename is
delimited by that comma, so a handful of strings cannot survive a round trip
through the header even though nothing stops a tool from putting them there.
The exact set is owned by `MediaFilenameProblem`, whose variants each carry
their own explanation; it is deliberately not restated here, because a second
copy drifts. It did: the first draft of this file listed four problems and
omitted `Empty`, which `problem_with` returns and this rule reports.

A remote URL may legitimately contain a comma, so the quoted form
(`@Media:\t"https://example.org/a,b.mp3", audio`) accepts one; the bare form
does not.

Programmatic construction in Rust is already closed: `MediaFilename::parse` is
the type's only constructor and rejects the same set, sharing one rule
definition, and one diagnostic, with this check.

## Expected Behavior

- **Parser (tree-sitter)**: unreachable. The grammar ends the filename at the
  comma, so no byte sequence in a `.cha` file can produce a violating value; a
  file containing `@Media:\ttake1,take2, audio` parses as filename `take1` and
  reports E316/E531 instead.
- **Parser (re2c)**: unreachable, for the same reason; the lexer splits on the
  comma.
- **Validator**: `check_media_filename_representable` walks the `@Media`
  headers and asks each filename to report its own problems, via
  `MediaFilename::report_representability_issues`. The rule and its diagnostic
  both live on the newtype, so a second caller cannot pick a different code,
  severity or wording for the same fact.

**How the rule is actually reached, stated precisely because the obvious answer
is wrong.** The value can only enter through a `ChatFile` deserialized from
JSON, since deserialization is deliberately lenient at the serde boundary
across this codebase. But **`chatter from-json` does NOT validate**: it
deserializes and calls `to_chat_string()` directly, so that command does not
fire this rule today. The reachable seam is the library one, `ChatFile::validate`
on a deserialized value, which is what the regression test drives. Whether
`from-json` should validate is an open question and not something this rule
assumes.

## CHAT Rule

The `@Media` filename is delimited by the comma that introduces the media type.
