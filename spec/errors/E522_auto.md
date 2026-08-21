+++
code = 'E522'
name = '@Participants header cannot be empty'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'header'
source = 'E5xx_header_errors/E506_empty_participants.cha'
claim = { subsumed_by = 'E342' }
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	

@End
'''

[[example]]
level = 'header'
source = 'E5xx_header_errors/E522_missing_id_for_participant.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Ruth Target_Child, MOT Mother
@ID:	eng|corpus|MOT|||||Mother|||
*CHI:	hello .
*MOT:	hi there .
@End
'''
+++

## Description

@Participants header cannot be empty

## Expected Behavior

The appropriate error should be reported; which stage catches it is observed in the snapshot, not declared.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
