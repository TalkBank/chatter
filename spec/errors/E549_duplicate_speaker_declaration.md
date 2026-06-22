# E549: Duplicate speaker declaration

## Description

The same speaker code is declared more than once in the `@Participants` header.
Each participant must be declared exactly once; a repeated speaker code is a
declaration error.

## Metadata

- **Error Code**: E549
- **Category**: header_validation
- **Level**: header
- **Layer**: validation

## Example 1

**Expected Error Codes**: E549

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child, CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello .
@End
```

## Expected Behavior

- **Parser**: Should succeed, the syntax is valid
- **Validator**: Should report E549, because `CHI` is declared twice in the
  `@Participants` header

## CHAT Rule

Each participant is declared exactly once in `@Participants`. Declaring the same
speaker code twice is an error.

This closes a chatter/CLAN-CHECK parity gap: CLAN CHECK reports error 13
("Duplicate speaker declaration") for this construct, which chatter previously
accepted.

Reference: <https://talkbank.org/0info/manuals/CHAT.html>
