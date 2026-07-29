# E757: Bracketed code glued to the following content

## Description

A bracketed code's closing `]` directly attached to the start of the
next word with no space (`hello [/]x`, `hello [!]x`) is invalid (CLAN
CHECK error 19, "Illegal use of delimiter in a word." / "Or a SPACE
should be added after it.").

EVERY bracketed code counts, not only retraces: a scoped annotation
(`[!]`, `[*]`, `[= text]`) attaches to the word BEFORE it, so material
glued after its `]` is a separate item just as it is after `[/]`, and the
missing space is the same defect. Until 2026-07-29 only the retrace
family was detected, which left the annotation shapes silently accepted
(juxtaposition-matrix cell 8, ruled REJECT 2026-07-18). Bracketed codes are free-standing items and must be
space-delimited from what follows. The parse itself is unambiguous
(the retrace closes at `]` and `x` becomes a separate word), which is
exactly why this is a STYLE rule: sloppy but readable source that must
still be rejected so the corpus stays canonically spaced.

## Metadata
- **Last updated**: 2026-07-29 19:51 EDT

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

## Example 2

**Trigger**: a scoped annotation's closing bracket glued to the next word.

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello [!]there .
@Comment:	ERROR: the annotation code is glued to the following word
@End
```

**Expected Error Codes**: E757

## Example 3

**Trigger**: an explanation code's closing bracket glued to the next word.

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	bobo [= toy]there .
@Comment:	ERROR: the explanation code is glued to the following word
@End
```

**Expected Error Codes**: E757

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
(2026-07-16 scan for the retrace family; the 2026-07-18 matrix scan
found `][letter` unattested anywhere, so extending to every bracketed
code adds no kept-file fallout either).
