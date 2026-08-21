+++
code = 'E307'
name = 'Auto-generated from corpus'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'utterance'
source = 'E3xx_main_tier_errors/E307_invalid_speaker_chars.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	A:B Child
@ID:	eng|corpus|A:B|||||Child|||
*A:B:	hello .
@End
'''
+++

## Description

Auto-generated from corpus

## Expected Behavior

The appropriate error should be reported; which stage catches it is observed in the snapshot, not declared.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
