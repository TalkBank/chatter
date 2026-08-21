+++
code = 'W108'
name = 'Auto-generated from corpus'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'file'
source = 'error_corpus/warnings/W108_speaker_not_in_participants.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*MOT:	hello .
@End
'''
+++

## Description

Auto-generated from corpus

## Expected Behavior

The parser should successfully parse this CHAT file, but validation should report the error.

**Trigger**: See example above

## CHAT Rule

See the CHAT manual for format specifications: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
