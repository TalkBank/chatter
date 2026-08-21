+++
code = 'E202'
name = 'Missing form type after @'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'word'
source = 'E2xx_word_errors/E202_empty_word.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello@ world .
@End
'''
+++

## Description

Missing form type after @

## Expected Behavior

The appropriate error should be reported; which stage catches it is observed in the snapshot, not declared.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
