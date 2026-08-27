+++
code = 'E604'
name = 'Empty GRA relation'

[[example]]
level = 'tier'
source = 'E7xx_tier_parsing/E707_empty_gra_relation.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world .
%gra:	1|2|NSUBJ  2|0|ROOT
@End
'''
+++

## Description

Empty GRA relation

## Expected Behavior

The appropriate error should be reported; which stage catches it is observed in the snapshot, not declared.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
