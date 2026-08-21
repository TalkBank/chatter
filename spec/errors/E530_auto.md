+++
code = 'E530'
name = 'Lazy gem inside background'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'header'
source = 'validation_gaps/lazy-gem-inside-bg.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
@Bg:activity
@Comment:	We are inside a @Bg/@Eg scope
@G:	playing with blocks
@Comment:	ERROR: @G (lazy gem) should not be allowed inside @Bg/@Eg scope
@Eg:activity
@End
'''
+++

## Description

Lazy gem inside background

## Expected Behavior

The appropriate error should be reported; which stage catches it is observed in the snapshot, not declared.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
