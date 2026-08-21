+++
code = 'E538'
name = 'Unsupported @Recording Quality Value'
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
@ID:	eng|corpus|CHI|||||Target_Child|||
@Recording Quality:	badquality
*CHI:	hello world .
@End
'''
+++

## Description

An `@Recording Quality` header contains a value that is not one of the recognized quality ratings. The file parses successfully but the unsupported value is stored as `Unsupported(String)` and flagged during validation.

## Expected Behavior

- **Parser**: Should succeed, syntax is valid
- **Validator**: Should report E538, unsupported @Recording Quality value 'badquality'

## CHAT Rule

The `@Recording Quality` header accepts values: `1`, `2`, `3`, `4`, `5`. Any other value is flagged as unsupported.

Reference: <https://talkbank.org/0info/manuals/CHAT.html#Recording_Quality_Header>

## Notes

This is a warning-level diagnostic. Unsupported recording quality values are preserved in the model for roundtrip fidelity.
