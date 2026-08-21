+++
code = 'E356'
name = 'UnmatchedUnderlineBegin'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'utterance'
source = 'error_corpus/validation_errors/E356_unmatched_underline_begin.cha'
claim = 'violates'
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: Unmatched underline begin marker
*CHI:	hello \u0002\u0001world .
@End
"""
+++

## Description

An underline begin marker was found without a matching underline end marker
in the same utterance. Underline markers (used in CA transcription to mark
stressed syllables) must occur in matched begin/end pairs within a single
utterance.

## Expected Behavior

Validation should report E356. The underline begin control character
(`\x02\x01`) has no matching end character (`\x02\x02`) within the
same utterance.

## Notes

- Underline markers are control characters used in CA (Conversation
  Analysis) transcription. They are represented as `\x02\x01` (begin)
  and `\x02\x02` (end) in the raw text.
- Found primarily in `ca-data/Jefferson/` corpus files where stressed
  syllables are underlined.
