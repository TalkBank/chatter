# E514: Empty corpus field in @ID

## Description

The corpus field (2nd field) of an `@ID` header is blank. The `@ID` header is
`lang|corpus|code|age|sex|group|SES|role|education|custom|`, and the corpus name
is required: a blank corpus is invalid.

## Metadata

- **Error Code**: E514
- **Category**: header_validation
- **Level**: header
- **Layer**: validation

## Example 1

**Expected Error Codes**: E514

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng||CHI|||||Target_Child|||
*CHI:	hello .
@End
```

## Expected Behavior

- **Parser**: Should succeed, the `@ID` line is syntactically valid (the empty
  field is a blank between two pipes).
- **Validator**: Should report E514, because the corpus field (between the 1st
  and 2nd pipe) is empty.

## CHAT Rule

Each `@ID` header names its corpus in the 2nd field. The corpus name must not be
empty.

This closes a chatter/CLAN-CHECK parity gap: CLAN CHECK reports error 63
("Missing Corpus name") for this construct, which chatter previously accepted.

Reference: <https://talkbank.org/0info/manuals/CHAT.html>
