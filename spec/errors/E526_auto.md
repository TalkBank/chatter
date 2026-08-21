+++
code = 'E526'
name = 'Auto-generated from corpus'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'header'
source = 'E5xx_header_errors/E526_unmatched_begin_gem.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||||Target_Child|||
@Bg:	episode1
*CHI:	hello world .
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
