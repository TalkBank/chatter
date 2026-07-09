# E751: Pause glued to the preceding word

## Description

A pause marker opening directly attached to the end of a word with no
space (`hello(.)`) is invalid (CLAN CHECK error 57, "Please add space
between word and pause symbol: '('.", check.cpp 4437). Pauses are
free-standing content items and must be space-delimited from words.

## Metadata

- **Error Code**: E751
- **Category**: Main tier separators
- **Level**: utterance
- **Layer**: validation
- **Status**: implemented

## Example 1

**Expected Error Codes**: E751

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello(.) there .
@Comment:	ERROR: the pause is glued to the word before it
@End
```

## Expected Behavior

- **Parser**: Succeeds; `hello` / pause / `there` all parse with spans.
- **Validator**: Reports E751 at the pause. Detection is span
  adjacency (previous word span end == pause span start) over the
  in-order content walk. Dummy spans are skipped (the re2c oracle
  mirrors the rule as a token-stream scan instead).

## CHAT Rule

Pauses are space-delimited items; see the CHAT manual on pauses.
Parity entry:
`crates/talkbank-parser-tests/tests/check_parity/manifest.json`
CHECK 57 (CLAN additionally emits (48) on the same construct).
