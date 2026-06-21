# E999: Unknown error

**Last modified:** 2026-05-30 19:04 EDT

## Description

Unknown error

## Metadata

- **Error Code**: E999
- **Status**: not_implemented
- **Note**: UnknownError is an internal catch-all emitted in 0 validation sites; no triggering CHAT example exists.
- **Category**: Alignment count mismatch
- **Level**: file
- **Layer**: validation

## Example 1

**Source**: `E4xx_alignment_errors/terminator_alignment.cha`
**Trigger**: `%mor` omits its terminator, so the pipeline now reports missing-terminator and alignment-skipped diagnostics
**Expected Error Codes**: E305, E600

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
@Comment:	Note: Terminators (. ! ?) align for %mor and %gra
*CHI:	I want cookie .
%mor:	pro|I v|want n|cookie
@Comment:	ERROR: %mor missing terminator (should end with .)
@Comment:	Main tier: 3 words + terminator = 4 alignable
@Comment:	Mor tier: Only 3 items (missing terminator)
@End
```

## Expected Behavior

The current parser/model pipeline should report the diagnostics it actually
emits for this malformed `%mor` tier: missing terminator on the tier itself,
plus alignment skipped due to parse-taint.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
