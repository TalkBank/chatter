+++
code = 'E519'
name = 'Auto-generated from corpus'

[[example]]
level = 'header'
source = 'E5xx_header_errors/E519_invalid_language_code.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	xxx
@Participants:	CHI Child
@ID:	xxx|corpus|CHI|||||Child|||
*CHI:	hello .
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
