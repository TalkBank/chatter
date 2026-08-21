+++
code = 'E507'
name = '@Languages header cannot be empty'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'header'
source = 'E5xx_header_errors/E507_empty_languages.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@End
'''
+++

## Description

@Languages header cannot be empty

## Expected Behavior

The appropriate error should be reported; which stage catches it is observed in the snapshot, not declared.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
