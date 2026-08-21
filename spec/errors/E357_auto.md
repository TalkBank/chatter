+++
code = 'E357'
name = 'UnmatchedUnderlineEnd'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'utterance'
source = 'error_corpus/validation_errors/E357_unmatched_underline_end.cha'
claim = 'violates'
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: Unmatched underline end marker
*CHI:	hello \u0002\u0002world .
@End
"""
+++

## Description

An underline end marker was found without a preceding underline begin
marker in the same utterance. The end marker has no open underline to
close.

## Expected Behavior

Validation should report E357. The underline end control character
(`\x02\x02`) appears without a preceding begin character (`\x02\x01`)
on the stack.

## Notes

- Underline markers are control characters used in CA (Conversation
  Analysis) transcription. An orphaned end marker without a matching
  begin is a data error.
