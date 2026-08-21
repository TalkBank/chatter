+++
code = 'E252'
name = 'Syntax error - caret at word start'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'word'
source = 'E3xx_main_tier_errors/E303_caret_at_word_start.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Comment:	Caret for pause between syllables must appear MID-WORD (e.g., rhi^noceros)
@Comment:	not at the start of a word
*CHI:	^test .
@End
'''
+++

## Description

Syntax error - caret at word start

## Expected Behavior

The appropriate error should be reported; which stage catches it is observed in the snapshot, not declared.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
