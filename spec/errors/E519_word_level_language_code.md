# E519: Word-level language code not in the ISO 639-3 registry

## Description

An explicit word-level language switch (`word@s:CODE`) must name a
real ISO 639-3 language. The code needs NO declaration in `@Languages`
(maintainer ruling 2026-07-15, part 1), but it must exist in the
registry (same ruling, part 2): registry validation is what actually
catches typo'd codes (the historical `cye`/`sp`/`nle` class), and it
reuses E519, the same rule that already guards `@Languages` and `@ID`.

Wild grounding (2026-07-16 probe): all 44 distinct word-level codes in
the kept corpus are registry-valid, so this rule flags nothing today.

## Metadata

- **Error Code**: E519
- **Category**: Main tier words
- **Level**: utterance
- **Layer**: validation
- **Status**: implemented

## Example 1

**Trigger**: `@s:qzz`, an unassigned three-letter code.

**Expected Error Codes**: E519

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hi@s:qzz there .
@Comment:	ERROR: qzz is not an ISO 639-3 language
@End
```

## Expected Behavior

- **Parser**: Succeeds; the word parses with an explicit language
  marker.
- **Validator**: Reports E519 at the word for any explicit word-level
  code (single or multiple/code-mixing) absent from the ISO 639-3
  registry. Declaration in `@Languages` remains NOT required.

## CHAT Rule

Language codes are ISO 639-3 everywhere they appear. Companion rules:
E519 on `@Languages`/`@ID` (header layer), E755 for undeclared
utterance-level `[- CODE]` (which, combined with header-layer E519,
already covers precode registry validity indirectly).
