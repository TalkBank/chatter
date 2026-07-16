# E243: Pipe character in main-tier word text

## Description

The `|` character is the %mor tier's part-of-speech delimiter and has
no meaning in main-tier word text; a word consisting of or containing
a bare pipe (`hello | there`) is invalid (CLAN CHECK error 48,
"Illegal character(s) '|' found."). This is a trigger shape of the
existing E243 (IllegalCharactersInWord) rule, not a new code: the
word scanner already rejects whitespace, bullet markers, control
characters, and private-use code points; the pipe joins that set as a
reserved tier-delimiter character.

## Metadata

- **Error Code**: E243
- **Category**: Word structure
- **Level**: word
- **Layer**: validation
- **Status**: implemented

## Example 1

**Expected Error Codes**: E243

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello | there .
@Comment:	ERROR: bare pipe is not a word
@End
```

## Expected Behavior

- **Parser**: Succeeds; the pipe tokenizes as a word with real spans.
- **Validator**: `check_word_characters` reports E243 on any word
  whose cleaned text contains `|`.

## CHAT Rule

`|` is reserved for dependent-tier morphology syntax. Parity entry:
`crates/talkbank-parser-tests/tests/check_parity/manifest.json`
CHECK 48 (this closes the grounded bare-pipe shape; CHECK 48 has 22
call sites and other shapes are adjudicated separately as they are
grounded). Wild-data impact at adoption: zero kept files with `|` in
main-tier word text (2026-07-16 typed-mirror scan).
