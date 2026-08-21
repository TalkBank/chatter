+++
code = 'E542'
name = 'Unsupported @ID Sex Value'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|3;06.|badsex|||Target_Child|||
*CHI:	hello world .
@End
'''
+++

## Description

An `@ID` header contains a sex field value that is not one of the recognized values. The file parses successfully but the unsupported value is stored as `Unsupported(String)` and flagged during validation.

## Expected Behavior

- **Parser**: Should succeed, syntax is valid
- **Validator**: Should report E542, unsupported @ID sex value 'badsex'

## CHAT Rule

The `@ID` header sex field accepts values: `male`, `female`. Any other value is flagged as unsupported.

Reference: <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>

## Notes

This is a warning-level diagnostic. Unsupported sex values are preserved in the model for roundtrip fidelity.
