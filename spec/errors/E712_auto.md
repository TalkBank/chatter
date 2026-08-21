+++
code = 'E712'
name = 'Auto-generated from corpus'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'tier'
source = 'error_corpus/validation_errors/E712_gra_invalid_word_index.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: Grammar relation word index out of bounds
*CHI:	I want .
%mor:	pro|I v|want .
%gra:	1|2|NSUBJ 5|0|ROOT 3|2|PUNCT
@End
'''
+++

## Description

Auto-generated from corpus

## Expected Behavior

The parser should successfully parse this CHAT file, but validation should report the error.

**Trigger**: See example above

## CHAT Rule

See CHAT manual sections on dependent tier formats (%mor, %gra, %pho, etc.). Each tier type has specific syntax requirements. The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
