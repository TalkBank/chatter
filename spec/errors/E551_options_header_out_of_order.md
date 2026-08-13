# E551: @Options header out of order

## Description

An `@Options` header does not immediately follow `@Participants`. Per the
CHAT spec the optional `@Options` line, when present, comes directly after
`@Participants`, before the `@ID` block or any other header. This check is
gated on `@Participants` already having been seen; `@Options` appearing
*before* `@Participants` is a distinct case reported as E543 instead, so
the two do not double-report the same header.

## Metadata

- **Error Code**: E551
- **Category**: header_validation
- **Level**: header
- **Layer**: validation
- **Kind**: Invalidity
- **Status**: implemented

## Example 1

**Trigger**: `@Options` follows `@ID`, not `@Participants`
**Expected Error Codes**: E551

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Options:	CA
*CHI:	hello world .
@End
```

## Expected Behavior

- **Parser**: Should succeed, syntax is valid
- **Validator**: Should report E551, `@Options` must immediately follow
  `@Participants`, before the `@ID` block or any other header

## CHAT Rule

The `@Options` header, when present, must immediately follow
`@Participants`. This corresponds to CLAN CHECK error 125 ("@Options
header must immediately follow @Participants: header").

Reference: <https://talkbank.org/0info/manuals/CHAT.html>

## Notes

Implementation: `check_options_header_order` in
`crates/talkbank-model/src/validation/header/structure.rs`.
