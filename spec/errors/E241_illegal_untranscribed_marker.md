+++
code = 'E241'
name = "Illegal Untranscribed Marker 'xx'"

[[example]]
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	I said xx today .
@End
'''

[[example]]
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@Options:	CA
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	I said xx today .
@End
'''
[[example]]
level = 'word'
claim = 'violates'
notes = 'The first utterance is structurally clean and retains E241 even when a separately owned second utterance has malformed morphology. Fixing xx must preserve that recovery region byte-for-byte.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	I said xx today .
*CHI:	hello .
%mor no_tab_separator
@End
'''
+++

## Description

The marker 'xx' is used for untranscribed speech, but this is not allowed in CHAT. The correct marker for untranscribed speech is 'xxx' (three x's).

## Expected Behavior

- **Parser**: Accepts 'xx' as a word. The partial-recovery example separately
  rejects its malformed morphology tier; this must not be mistaken for recovery
  in the earlier, structurally clean utterance.
- **Validator**: Should report E241 - 'xx' is not a valid untranscribed marker

## CHAT Rule

Untranscribed speech must be marked with 'xxx' (three x's). Single 'x' represents a single unintelligible phoneme, 'xx' is not a valid CHAT marker. Use 'xxx' for untranscribed speech segments.

## Notes

This is a semantic validation error that occurs at the word level. The parser accepts 'xx' as a valid word syntactically, but the validator must check that it follows CHAT conventions. Two-letter 'xx' combinations are sometimes mistakenly used instead of the correct 'xxx' marker.
