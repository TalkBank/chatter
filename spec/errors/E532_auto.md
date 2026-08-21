+++
code = 'E532'
name = 'Invalid participant role'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'header'
source = 'validation_gaps/invalid-participant-role.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child, MOT Mother, INV Investigator, BOB InvalidRole
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
@ID:	eng|corpus|MOT|30;00.|female|||Mother|||
@ID:	eng|corpus|INV|25;00.|female|||Investigator|||
@ID:	eng|corpus|BOB|35;00.|male|||InvalidRole|||
@Comment:	ERROR: "InvalidRole" is not a valid participant role
@Comment:	Valid roles include: Target_Child, Mother, Father, Investigator, etc.
*CHI:	hello .
*MOT:	hi sweetie .
*BOB:	I have an invalid role .
@End
'''
+++

## Description

Invalid participant role

## Expected Behavior

The appropriate error should be reported; which stage catches it is observed in the snapshot, not declared.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
