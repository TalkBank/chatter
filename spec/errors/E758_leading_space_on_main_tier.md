# E758: Leading space before main-tier content in a non-CA file

## Description

A space between the main tier's tab delimiter and its first real
element (`*CHI:<tab><space>dog .`) is invalid in a file without
`@Options: CA` (CLAN CHECK error 123, "Illegal character '<space>'
found in tier text. If it CA, then add \"@Options: CA\""). CA
transcripts use space-based column alignment after the tab, so the rule
is exempted there; every wild occurrence of the construct (457 kept
files, 2026-07-16 scan) is in a CA file, confirming the boundary.

Detection is exact source-span comparison, the same paradigm the rest
of the source-spacing family (E751 / E757 / comma spacing) already
uses: the tier's first real element either starts at the byte after the
single tab, or there is a gap. The first real element is the earliest
of the leading discourse linker (`+,`, `++`, `+"`, ...), the `[- code]`
utterance-language precode, or the first content item. Because linkers
and the precode now carry their own source spans, a leading space
before them is measurable too, so the previous opt-out (which skipped
any tier carrying a linker or precode) is gone.

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

- **Parser**: Succeeds; the extra whitespace before ordinary content is
  tolerated by the grammar and the content parses with real spans. (A
  leading space directly before a linker or `[- code]` precode is a
  DIFFERENT case: the grammar does not yet accept it as a clean linker /
  precode, so it currently surfaces as E316 parse recovery, not E758.
  Closing that as E758 needs grammar work and is deliberately deferred.)
- **Validator**: File-level check (needs the `@Options` headers): when
  the file does not declare the CA option, reports E758 on any main
  tier whose first real element (leading linker, precode, or first
  content item, by source span) starts more than one byte (the single
  tab) after the tier separator. Files declaring `@Options: CA` are
  exempt. A tier whose first element carries no real span (the re2c
  oracle's dummy spans, or a span-less content-first item) opts out
  because there is nothing to measure against.

## CHAT Rule

One tab separates the tier label from content; extra leading
whitespace is CA-only. Parity entry:
`crates/talkbank-parser-tests/tests/check_parity/manifest.json`
CHECK 123.

## Planned extension (ruling H.1, not yet implemented)

The same "one tab, then content" convention governs `@headers` and
`%`-dependent tiers, so leading whitespace after their tab is also
E758 (ruling H.1). Extending the check there requires giving each
dependent-tier line and header its own after-separator span (the model
currently stores only whole-line spans, whose start is the `%`/`@`, not
the first content byte). That is a separate, larger model change; see
`docs/proposals/2026-07-18-source-spacing-ground-truth.md`.
