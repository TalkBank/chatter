+++
code = 'E509'
name = '@Media header cannot be empty'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'header'
source = 'E5xx_header_errors/E509_empty_media.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Media:
@End
'''
+++

## Description

@Media header cannot be empty

## Expected Behavior

The appropriate error should be reported; which stage catches it is observed in the snapshot, not declared.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
