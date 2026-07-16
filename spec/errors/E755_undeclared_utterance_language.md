# E755: Utterance language not declared in `@Languages`

## Description

A `[- CODE]` precode marks a whole utterance as being in another
language: substantial language presence in the transcript. The
`@Languages` header declares the transcript's substantial languages,
so an utterance-level language missing from it leaves the header
misrepresenting the transcript. Matches CLAN CHECK error 152
("Language is not defined on @Languages header tier."). Ruled
2026-07-15 (maintainer decision,
docs/design/2026-07-15-at-s-language-declaration-decision.md, part 3):
declaration IS required at utterance level, deliberately UNLIKE
word-level `@s:CODE` insertions, which remain free (part 1 of the
same ruling; the corpus grounding found 0 of 7,167 precode-bearing
files violate this invariant while 854 files legitimately use
undeclared word-level codes).

## Metadata

- **Error Code**: E755
- **Category**: header_validation
- **Level**: utterance
- **Layer**: validation
- **Status**: implemented

## Example 1

**Trigger**: `[- fra]` utterance in a transcript declaring only `eng`.

**Expected Error Codes**: E755

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	[- fra] bonjour .
@Comment:	ERROR: utterance language fra is not declared
@End
```

## Expected Behavior

- **Parser**: Succeeds; the precode parses as the utterance's
  language code.
- **Validator**: Reports E755 at the utterance when its `[- CODE]`
  language is absent from `@Languages`. Word-level `@s:CODE` markers
  are deliberately NOT subject to this rule.

## CHAT Rule

`@Languages` declares the transcript's substantial languages;
utterance-level presence is substantial. Parity entry:
`crates/talkbank-parser-tests/tests/check_parity/manifest.json`
CHECK 152.
