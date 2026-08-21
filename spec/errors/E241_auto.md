+++
code = 'E241'
name = 'Auto-generated from corpus'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'word'
source = 'error_corpus/validation_errors/E241_illegal_untranscribed.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: Untranscribed markers must be xxx, yyy, or www
@Comment:	Invalid: 'xx' - Only xxx, yyy, www are allowed
*CHI:	xx .
@End
'''
+++

## Description

Auto-generated from corpus

## Expected Behavior

The parser should successfully parse this CHAT file, but validation should report the error.

**Trigger**: See example above

## CHAT Rule

See CHAT manual sections on word-level syntax and special markers. The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
