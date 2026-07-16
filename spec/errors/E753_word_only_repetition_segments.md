# E753: Word consisting only of repetition segments

## Description

A word whose entire spoken material sits inside segment-repetition
delimiters (`↫...↫`, U+21AB) marks the repetition of a segment of a
word that is not there: the notation presumes a host word (a stem)
outside the repeated span, as in `↫p↫parents` ("p-, parents"). A fully
wrapped word asserts a repetition of nothing and fails to make sense.
Corresponds to CLAN CHECK error 151 ("This word has only repetition
segments.", check.cpp `check_isThereStem`), which only the GUI CLAN
build enforces; chatter adopts the rule in its own semantics
(maintainer ruling, 2026-07-15).

Any material outside the repeated span counts as a stem, including a
word-category prefix marker (`&-` filler, `&~` nonword, `0` omission):
this matches GUI CHECK's character-level scan, under which any
character outside the arrows suffices. Wild-data grounding: 13,145
repetition-bearing words in the kept corpus carry a stem; the only 2
fully-wrapped tokens are `&-`-prefixed fillers, which this rule
accordingly keeps valid.

## Metadata

- **Error Code**: E753
- **Category**: Main tier words
- **Level**: utterance
- **Layer**: validation
- **Status**: implemented

## Example 1

**Trigger**: the word's only material is inside `↫...↫`.

**Expected Error Codes**: E753

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	↫hi↫ there .
@Comment:	ERROR: the first word is only a repetition segment
@End
```

## Expected Behavior

- **Parser**: Succeeds; the wrapped word parses with its
  segment-repetition delimiters as typed word content.
- **Validator**: Reports E753 at the word. Detection is a typed walk
  of the word's content parts: toggle an inside-span flag on
  `SegmentRepetition` CA delimiters; material parts (text, phonetic,
  shortening) found outside any span constitute a stem; a word with
  repetition delimiters, no outside material, and no word-category
  prefix marker is rejected.
- **Not flagged**: `↫p↫parents` (stem outside), `&-↫w-w-w↫` (filler
  prefix is material outside the arrows).

## CHAT Rule

Segment repetition marks a repeated portion OF a word; see the CHAT
manual on CA segment repetition. Parity entry:
`crates/talkbank-parser-tests/tests/check_parity/manifest.json`
CHECK 151 (no_obligation: unix CHECK cannot emit it; chatter enforces
the construct on its own authority).
