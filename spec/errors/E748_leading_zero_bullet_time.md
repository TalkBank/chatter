# E748: Leading zero in bullet timestamp

## Description

A media bullet timestamp is written with a leading zero before another
digit (for example `012_200`). CHAT bullet times are plain
millisecond integers; a leading zero is an illegal time representation
(CLAN CHECK error 90, `check_getMediaTagInfo` res 3). A bare `0`
timestamp (for example `0_200`) is legal: the rule fires only
when a `0` is followed by another digit.

## Metadata

- **Error Code**: E748
- **Category**: Media bullets
- **Level**: tier
- **Layer**: parser
- **Status**: implemented

## Example 1

**Expected Error Codes**: E748

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Media:	session, audio
*CHI:	hey . 012_200
@Comment:	ERROR: start time 012 has a leading zero
@End
```

## Example 2

**Expected Error Codes**: E748

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Media:	session, audio
*CHI:	hey . 100_012
@Comment:	ERROR: end time 012 has a leading zero
@End
```

## Expected Behavior

- **Parser**: Reports E748 on the bullet whose timestamp carries the
  leading zero. The bullet's numeric value still parses (recovery is
  preserved; the diagnostic makes the file invalid).
- **Validator**: No separate validation-layer check; the raw digit text
  exists only in the source, so the parser is the only layer that can
  see the leading zero.

## CHAT Rule

Bullet times are integer milliseconds. CLAN CHECK rejects a time
component matching `0[0-9]` as an "Illegal time representation inside
a bullet." (code 90). Parity entry:
`crates/talkbank-parser-tests/tests/check_parity/manifest.json`
CHECK 90.
