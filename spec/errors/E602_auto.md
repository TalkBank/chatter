+++
code = 'E602'
name = 'Auto-generated from corpus'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'tier'
source = 'E6xx_dependent_tier_errors/E602_malformed_tier_header.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello .
%mor
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
