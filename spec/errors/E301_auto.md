+++
code = 'E301'
name = 'Empty speaker code'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'utterance'
source = 'E3xx_main_tier_errors/E301_empty_speaker.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*:	hello world .
@End
'''
+++

## Description

Empty speaker code

## Expected Behavior

The appropriate error should be reported; which stage catches it is observed in the snapshot, not declared.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
