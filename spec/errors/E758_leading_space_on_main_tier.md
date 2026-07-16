# E758: Leading space before main-tier content in a non-CA file

## Description

A space between the tier's tab delimiter and the first content item
(`*CHI:<tab><space>dog .`) is invalid in a file without `@Options: CA`
(CLAN CHECK error 123, "Illegal character '<space>' found in tier
text. If it CA, then add \"@Options: CA\""). CA transcripts use
space-based column alignment after the tab, so the rule is exempted
there; every wild occurrence of the construct (457 kept files,
2026-07-16 scan) is in a CA file, confirming the boundary.

## Metadata

- **Error Code**: E758
- **Category**: Main tier structure
- **Level**: utterance
- **Layer**: validation
- **Status**: implemented

## Example 1

**Expected Error Codes**: E758

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	 dog .
@Comment:	ERROR: space after the tab in a non-CA file
@End
```

## Expected Behavior

- **Parser**: Succeeds; the extra whitespace is tolerated by the
  grammar and the content parses with real spans.
- **Validator**: File-level check (needs the `@Options` headers):
  when the file does not declare the CA option, reports E758 on any
  main tier whose first content item starts after the expected
  content position (speaker code + colon + tab). Files declaring
  `@Options: CA` are exempt. Tiers with leading discourse linkers
  (`+,`, `++`, `+"`, ...) or a `[- CODE]` utterance-language precode
  opt out: both live as span-less model fields, so their bytes are
  indistinguishable from whitespace in the gap arithmetic. Tiers
  whose first content item is span-less likewise opt out.

## CHAT Rule

One tab separates the tier label from content; extra leading
whitespace is CA-only. Parity entry:
`crates/talkbank-parser-tests/tests/check_parity/manifest.json`
CHECK 123.
