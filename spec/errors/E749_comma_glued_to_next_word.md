# E749: Comma glued to the following word

## Description

A comma on a speaker tier must be followed by a space or end-of-line
(CLAN CHECK error 92, "Item ',' must be followed by space or
end-of-line.", check.cpp 4309-4320). Writing `hey ,you` glues the comma
to the next word. The rule fires only when the next in-order item is a
word starting at the byte immediately after the comma; constructs that
put any other character after the comma (group `<`, overlap marks, CA
marks) are not flagged, matching CLAN's CA exemptions conservatively.

## Metadata

- **Error Code**: E749
- **Category**: Main tier separators
- **Level**: utterance
- **Layer**: validation
- **Status**: implemented

## Example 1

**Expected Error Codes**: E749

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hey ,you .
@Comment:	ERROR: the comma is glued to the word after it
@End
```

## Expected Behavior

- **Parser**: Succeeds; `hey` / comma / `you` all parse with spans.
- **Validator**: Reports E749 at the comma. Detection is span
  adjacency (comma span end == next word span start) over the in-order
  content walk, so commas inside groups are covered too. Spans equal to
  the dummy span are skipped (the re2c oracle fills dummy separator
  spans and instead mirrors this rule as a token-stream scan).

## CHAT Rule

CHAT punctuation is space-delimited; see the CHAT manual on the main
tier and separators. Parity entry:
`crates/talkbank-parser-tests/tests/check_parity/manifest.json`
CHECK 92.
