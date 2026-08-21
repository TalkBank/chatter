+++
code = 'E524'
name = '@Birth header for unknown participant'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'header'
source = 'E5xx_header_errors/E524_birth_unknown_participant.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Ruth Target_Child
@ID:	eng|corpus|CHI|2;06.00||||Target_Child|||
@Birth of MOT:	01-JAN-2000
*CHI:	hello .
@End
'''
+++

## Description

@Birth header for unknown participant

## Expected Behavior

The appropriate error should be reported; which stage catches it is observed in the snapshot, not declared.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
