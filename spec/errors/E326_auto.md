+++
code = 'E326'
name = 'UnexpectedLineType'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||female|||Target_Child|||
*CHI:	hello .
this is just a bare line of text
@End
'''
+++

## Description

A line was classified as an unexpected type during file structure parsing.
This covers two sub-cases:

1. **Warning**: A line matched by tree-sitter's `unsupported_line` rule, a
   catch-all for content that doesn't conform to any recognized CHAT line
   shape (header, utterance, or dependent tier).
2. **Error**: A child of a LINE node whose kind is completely unknown to the
   Rust parser, a grammar/parser mismatch.

## Expected Behavior

The parser should report E326 (warning severity) for the bare text line.
Tree-sitter matches it as `unsupported_line` since it doesn't start with
any recognized prefix.

## Notes

- The `corpus/reference/constructs/unsupported.cha` file exercises this
  via an unsupported line, as well as `unsupported_header` and
  `unsupported_dependent_tier`.
- Warning-severity E326 is for `unsupported_line` nodes; error-severity
  E326 is for completely unknown LINE child kinds (parser/grammar mismatch).
