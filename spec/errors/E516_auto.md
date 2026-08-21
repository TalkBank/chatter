+++
code = 'E516'
name = 'Auto-generated from corpus'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'header'
source = 'error_corpus/validation_errors/E516_empty_date.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Date:	
@Comment:	ERROR: Date header cannot be empty
*CHI:	hello .
@End
'''
+++

## Description

Auto-generated from corpus

## Expected Behavior

The parser should reject this CHAT input and report a parse error at the location of the invalid syntax.

**Trigger**: See example above

## CHAT Rule

See CHAT manual sections on file headers and metadata. Headers like @Participants, @Languages, and @ID have specific format requirements. The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
