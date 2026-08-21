+++
code = 'E319'
name = 'UnparsableLine'
kind = 'Invalidity'
status = 'not_implemented'

[[example]]
level = 'utterance'
claim = { subsumed_by = 'E602' }
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||female|||Target_Child|||
*CHI:	hello .
%%%broken line content here
@End
'''
+++

## Description

A line could not be classified as a header, utterance, or dependent tier.
This is a fallback error emitted when tree-sitter produces an ERROR node
for a line whose children cannot be identified as either a header or
utterance context.

## Expected Behavior

The parser should report E319 for the unrecognizable line. Tree-sitter
wraps the line in an ERROR node whose children don't match header or
utterance patterns, triggering this fallback.

## Notes

- This is a parser recovery fallback; it fires when more specific error
  analysis (E320 for headers, E321 for utterances) cannot classify the line.
- The tree-sitter grammar may parse some unrecognizable lines as
  `unsupported_line` (→ E326) rather than ERROR nodes (→ E319).
- **Status note**: The example above triggers E602 (malformed dependent tier)
  rather than E319. The tree-sitter grammar routes `%%%` lines through the
  dependent tier path. Triggering E319 requires an ERROR node whose children
  can't be classified as header or utterance context, difficult to produce
  with a specific example because tree-sitter's error recovery is robust.
