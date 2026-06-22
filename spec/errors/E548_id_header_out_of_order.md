# E548: @ID header out of order

## Description

An `@ID` header does not immediately follow the `@Participants` / `@Options`
headers (or another `@ID`). The `@ID` block must come directly after
`@Participants` (and the optional `@Options`), with no other header in between.
A changeable header such as `@Comment` between `@Participants`/`@Options` and
the `@ID` block is an ordering violation.

## Metadata

- **Error Code**: E548
- **Category**: header_validation
- **Level**: header
- **Layer**: validation

## Example 1

**Expected Error Codes**: E548

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@Comment:	a changeable header before the @ID block
@ID:	eng|corpus|CHI|3;06.|male|||Target_Child|||
*CHI:	hello world .
@End
```

## Expected Behavior

- **Parser**: Should succeed, the syntax is valid
- **Validator**: Should report E548, because `@Comment` appears between
  `@Participants` and the `@ID` block, so `@ID` does not immediately follow
  `@Participants` / `@Options`

## CHAT Rule

The `@ID` headers must immediately follow `@Participants` (and the optional
`@Options`), with no other header in between; subsequent `@ID` headers follow
one another. A changeable header (for example `@Comment`) inserted between
`@Participants`/`@Options` and the `@ID` block is an ordering violation.

The distinct case of an `@ID` appearing *before* `@Participants` is reported
separately as E543. This rule closes a chatter/CLAN-CHECK parity gap: CLAN CHECK
reports error 126 ("@ID header must immediately follow @Participants: or
@Options header") for this construct, which chatter previously accepted.

Reference: <https://talkbank.org/0info/manuals/CHAT.html>

## Notes

This was found by the behavioral parity method (a fixture CLAN CHECK flags as
126, confirmed not caught by chatter), not by the keyword-mapping audit that had
spuriously certified 126 as complete.
