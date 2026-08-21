+++
code = 'E747'
name = 'Blank line not allowed'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'file'
title = 'Blank line between utterances'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hi .

*CHI:	bye .
@End
'''
+++

## Description

CHAT does not allow blank lines anywhere in the transcript (CLAN CHECK 91).
The grammar represents a blank line as a structural `blank_line` node (a
lone newline at a line boundary), so the parser emits this diagnostic
directly from the tree rather than by scanning the source text.

## Expected Behavior

- **Parser**: Should succeed (recovers around the blank line; neither
  surrounding utterance is dropped from the model)
- **Validator**: Should report E747 at the blank line's own span, with the
  message "Blank lines are not allowed"

## CHAT Rule

Blank lines are not permitted anywhere in a CHAT transcript. See CHAT
manual on file structure: <https://talkbank.org/0info/manuals/CHAT.html>

## Notes

- Emit site: `crates/talkbank-parser/src/parser/chat_file_parser/chat_file/document_lowering.rs`.
- Only a `blank_line` node that sits at a genuine line boundary (starts the
  file, or is immediately preceded by another newline) is reported; under
  error recovery a `blank_line` node can otherwise cover just the trailing
  newline of a non-blank malformed line, and reporting E747 there would
  point the reader at a blank line that does not exist.
- Pinned end-to-end (exact byte span and message) by
  `blank_line_between_utterances_produces_e747` in
  `crates/talkbank-parser/tests/integration/line_dispatch_characterization.rs`.
