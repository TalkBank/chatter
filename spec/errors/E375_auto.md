+++
code = 'E375'
name = 'Scoped annotation parse error'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'utterance'
source = 'E3xx_main_tier_errors/E350_unexpected_annotation_node.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Comment:	Note: This may need adjustment after testing
*CHI:	hello [[[[ test ]]]] world .
@End
'''
+++

## Description

Scoped annotation parse error

## Expected Behavior

The appropriate error should be reported; which stage catches it is observed in the snapshot, not declared.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
