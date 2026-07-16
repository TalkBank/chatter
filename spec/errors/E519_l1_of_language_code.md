# E519: @L1 of language code not in the ISO 639-3 registry

## Description

The `@L1 of SPK` header names a participant's first language. Wild
usage is uniformly ISO 639-3 codes (16 distinct values across 1,158
kept files, all registry-valid), so the field is a language CODE and
is held to the same registry rule as `@Languages` / `@ID` / word-level
switches (maintainer ruling 2026-07-15, part 2).

## Metadata

- **Error Code**: E519
- **Category**: header_validation
- **Level**: header
- **Layer**: validation
- **Status**: implemented

## Example 1

**Trigger**: `@L1 of` carrying an unassigned three-letter code.

**Expected Error Codes**: E519

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@L1 of CHI:	qzz
*CHI:	hi .
@End
```

## Expected Behavior

- **Parser**: Succeeds; the header parses with a typed language code.
- **Validator**: Reports E519 at the header when the code fails the
  shared language-code rule set (shape, placeholder, registry).

## CHAT Rule

Language codes are ISO 639-3 everywhere they appear.
