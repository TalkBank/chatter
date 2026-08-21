+++
code = 'E501'
name = 'Auto-generated from corpus'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'header'
source = 'error_corpus/E5xx_header_errors/E501_duplicate_header.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@End
'''

[[example]]
level = 'header'
source = 'error_corpus/E5xx_header_errors/E512_participant_no_role.cha'
claim = { subsumed_by = 'E513' }
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI
@ID:	eng|corpus|CHI|||||CHI|||
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
