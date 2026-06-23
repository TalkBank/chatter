# E550: Trailing comma in @Participants

## Description

The `@Participants` header ends with a trailing comma: a stray comma after the
last participant, with no participant following it. The participant list is
comma-separated (`CHI Target_Child, MOT Mother`), so a comma with nothing after
it is a dangling separator. This is distinct from an empty `@Participants`
header; the header has participants, it just has an extra comma at the end.

## Metadata

- **Error Code**: E550
- **Category**: header_validation
- **Level**: header
- **Layer**: parser

## Example 1

**Expected Error Codes**: E550

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child, MOT Mother,
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
*CHI:	hello .
@End
```

## Expected Behavior

- **Parser**: Should report E550. The grammar parses the participant list and
  the trailing comma surfaces as a tree-sitter `ERROR` node inside the
  `@Participants` header; the parser must report that ERROR rather than
  silently dropping it.
- **Validator**: No additional validation error is required; the parse-stage
  diagnostic is sufficient.

## CHAT Rule

The participant list in `@Participants` is a comma-separated list of
participants. A separator must be followed by another participant; a trailing
comma after the last participant is not allowed.

This closes a chatter/CLAN-CHECK parity gap: CLAN CHECK reports error 100
("Commas at the end of PARTICIPANTS tier are not allowed.") for this construct,
which chatter previously accepted by silently discarding the dangling comma.

Reference: <https://talkbank.org/0info/manuals/CHAT.html>
