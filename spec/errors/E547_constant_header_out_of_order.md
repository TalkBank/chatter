# E547: Constant participant header out of order

## Description

A constant participant-specific header (`@Birth of`, `@Birthplace of`, or
`@L1 of`) does not immediately follow the `@ID` block. These headers must come
directly after the `@ID` headers, before any changeable header such as
`@Comment`, `@Date`, `@Situation`, or `@Types`. A changeable header between the
`@ID` block and a constant participant header is an ordering violation.

## Metadata

- **Error Code**: E547
- **Category**: header_validation
- **Level**: header
- **Layer**: validation

## Example 1

**Expected Error Codes**: E547

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|3;06.|male|||Target_Child|||
@Comment:	a changeable header before the constant headers
@Birth of CHI:	15-DEC-1970
*CHI:	hello world .
@End
```

## Expected Behavior

- **Parser**: Should succeed, the syntax is valid
- **Validator**: Should report E547, because `@Birth of CHI` appears after
  `@Comment`, so it does not immediately follow the `@ID` block

## CHAT Rule

The constant participant-specific headers `@Birth of`, `@Birthplace of`, and
`@L1 of` must immediately follow the `@ID` headers, before any changeable
header. A changeable header (for example `@Comment` or `@Date`) between the
`@ID` block and a constant participant header is an ordering violation.

This closes a chatter/CLAN-CHECK parity gap: CLAN CHECK reports error 127
("Header must follow @ID: or @Birth of or @Birthplace of or @L1 of header") for
this construct, which chatter previously accepted.

Reference: <https://talkbank.org/0info/manuals/CHAT.html>

## Notes

A common real-world source of this violation is a tool that inserts a
provenance `@Comment` immediately after the `@ID` block, displacing the
`@Birth of` header that should sit there.
