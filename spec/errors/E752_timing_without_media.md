# E752: Timing bullets without an `@Media` header

## Description

The transcript carries timing evidence (main-tier bullets, or a
positional `%wor` timing sidecar), but no `@Media` header declares
the media timeline those timestamps index. A timestamp into an
undeclared recording fails to make sense: consumers cannot resolve
what the offsets refer to. This is the inverse direction of E544
(`@Media` declares linkage but no timing evidence exists) and
corresponds to CLAN CHECK error 112 ("Please add \"unlinked\" to
@Media header.", check.cpp 3927, `check_getOLDMediaTagInfo` res==6).

Adjudicated MEANINGFUL 2026-07-14 (per-rule CHECK adjudication):
grounding scan found bullet-bearing corpus files universally carry
`@Media`, so the rule pins an invariant the wild data already holds.

## Metadata

- **Error Code**: E752
- **Category**: header_validation
- **Level**: file
- **Layer**: validation
- **Status**: implemented

## Example 1

**Trigger**: a main-tier utterance carries a timing bullet but the
header block has no `@Media` at all.

**Expected Error Codes**: E752

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hey there .100_2500
@Comment:	ERROR: bullet with no @Media header
@End
```

## Expected Behavior

- **Parser**: Succeeds; the bullet parses as utterance timing.
- **Validator**: Reports E752 once, at file level (the first timing
  surface found), when timing evidence exists and no `@Media` header
  is present. Timing evidence is the same union E544 uses: main-tier
  bullets OR a positional `%wor` timing sidecar. Any `@Media` header
  (with or without a status qualifier) satisfies the requirement;
  whether the status CONTRADICTS the timing is E552's job, and
  whether declared linkage lacks timing is E544's job.

## CHAT Rule

`@Media` declares the recording a transcript's time alignments index;
timing bullets presume that declaration. Parity entry:
`crates/talkbank-parser-tests/tests/check_parity/manifest.json`
CHECK 112.
