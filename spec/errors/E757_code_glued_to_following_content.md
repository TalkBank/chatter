# E757: Bracketed code glued to the following content

## Description

A bracketed code's closing `]` directly attached to the start of the
next word with no space (`hello [/]x`) is invalid (CLAN CHECK error 19,
"Illegal use of delimiter in a word." / "Or a SPACE should be added
after it."). Bracketed codes are free-standing items and must be
space-delimited from what follows. The parse itself is unambiguous
(the retrace closes at `]` and `x` becomes a separate word), which is
exactly why this is a STYLE rule: sloppy but readable source that must
still be rejected so the corpus stays canonically spaced.

## Metadata

- **Error Code**: E757
- **Category**: Main tier separators
- **Level**: utterance
- **Layer**: validation
- **Status**: implemented

## Example 1

**Expected Error Codes**: E757

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello [/]x .
@Comment:	ERROR: the retrace code is glued to the following word
@End
```

## Expected Behavior

- **Parser**: Succeeds; the retrace group and the following word both
  parse with real spans.
- **Validator**: Reports E757 at the glued word. Detection is span
  adjacency over the top-level content sequence (a bracketed
  construct's span end == the next item's span start). Dummy spans
  are skipped (the re2c oracle mirrors the rule in its own front end).

## CHAT Rule

Bracketed codes are space-delimited items. Parity entry:
`crates/talkbank-parser-tests/tests/check_parity/manifest.json`
CHECK 19. Wild-data impact at adoption: zero kept files
(2026-07-16 scan).
