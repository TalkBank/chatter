+++
code = 'E721'
name = 'GRA non-sequential index'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'tier'
source = 'error_corpus/validation_errors/E721_gra_non_sequential.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	I want cookie .
%mor:	pro|I v|want n|cookie .
%gra:	1|3|NSUBJ 3|0|ROOT 2|3|OBJ 4|3|PUNCT
@End
'''
+++

**Last updated:** 2026-04-04 08:15 EDT

## Description

`%gra` tier indices must be sequential (1, 2, 3, ..., N). Non-sequential indices indicate a malformed dependency structure.

## Expected Behavior

The validator should report E721 because the `%gra` indices are not sequential: position 2 has index 3 (expected 2).

**Trigger**: See example above

## CHAT Rule

See CHAT manual sections on dependent tier formats (%mor, %gra, %pho, etc.). Each tier type has specific syntax requirements. The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- The previous example had a trailing ` .` on the `%gra` tier which caused tree-sitter to produce E316 (unparsable content). `%gra` tiers do not have terminators, removed the trailing period to allow the tier to parse correctly so the E721 structural validation can fire.
- Review and enhance this specification as needed
