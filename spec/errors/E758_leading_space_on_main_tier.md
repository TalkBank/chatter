# E758: Trailing space in a line's tier separator (non-CA file)

## Description

Every CHAT line has the shape `label:<tab>content`, where the separator
between the label and the content is a colon and exactly one tab. Any
further whitespace after that tab is not content: it is trailing
whitespace of the separator. In a file without `@Options: CA`, a
trailing space there is invalid (CLAN CHECK error 123, "Illegal
character '<space>' found in tier text. If it CA, then add \"@Options:
CA\"").

The rule is uniform across every colon-tab line: the main `*SPK:` tier,
every `@header` (free-text and structured alike), and every
`%`-dependent tier. The separator greedily consumes the colon, the one
required tab, and any trailing spaces; content begins at the first
non-space byte; and a non-CA file with trailing separator spaces reports
E758.

CA transcripts historically column-align content with spaces after the
tab, so the rule is CA-exempt: every wild occurrence of the construct
(457 files in the 2026-07-16 scan) is in a CA file, confirming the
boundary. On roundtrip the separator is always canonicalized to a single
tab (the meaningless spaces are dropped, CA included); the spaces are
never re-emitted, so E758 is a flag, not an auto-fix that mutates
content.

Extra TABS (a second tab after the first) are a DIFFERENT error (CLAN
CHECK 132, "Tabs should only be used to mark the beginning of lines")
and are out of scope here.

## Metadata

- **Error Code**: E758
- **Category**: Tier structure
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
@Comment:	main tier has a trailing separator space
@End
```

## Example 2

**Expected Error Codes**: E758

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	dog .
@Comment:	 free-text header has a trailing separator space
@End
```

## Example 3

**Expected Error Codes**: E758

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	dog .
%com:	 text dependent tier has a trailing separator space
@End
```

## Example 4

**Expected Error Codes**: E758

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	dog .
%mor:	 n|dog .
@End
```

## Example 5

**Expected Error Codes**: E758

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*MOT:	ready ?
*CHI:	 +, dog .
@End
```

## Expected Behavior

- **Parser**: Succeeds on all five lines. The trailing separator spaces
  are consumed by the separator (not the content), so a leading space
  before ordinary content, before a linker or `[- code]` precode, before
  structured `%mor`/`%gra` content, or in a header body all parse
  cleanly with the space held as separator provenance rather than
  producing a recovery node.
- **Validator**: File-level check (needs the `@Options` headers): when
  the file does not declare the CA option, reports E758 on any colon-tab
  line whose separator carries trailing spaces. Files declaring
  `@Options: CA` are exempt.
- **Serializer**: Canonicalizes the separator to a single tab; the
  trailing spaces are never re-emitted.

## CHAT Rule

One tab separates the label from content; any trailing space after it is
CA-only. Parity entry:
`crates/talkbank-parser-tests/tests/check_parity/manifest.json`
CHECK 123.
