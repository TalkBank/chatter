# E764: prefixed form glued to the preceding word

## Description

The `&` prefixes introduce a word of their own: a filler (`&-um`), a nonword
(`&~gaga`), or a phonological fragment (`&+fr`). Each is a separate main-tier
word and must be separated from what precedes it by a space:

```
*CHI:	the dog &-um barked .
```

Written without that space, `dog&-um` still parses, and it parses as TWO
words, because `&` cannot continue a word. So a missing space silently
manufactures a word boundary and nothing complains. That is the exact
accidental-juxtaposition class this audit exists for: the transcriber sees one
token, the corpus contains two, and no diagnostic distinguishes the typo from
the intent.

This is a STYLE rule in the E749/E751/E757 family: the parse is unambiguous,
which is precisely why the source must be canonically spaced instead of
relying on the reader to notice.

Glued omission (`dog0is`) is NOT this rule: `0` is ordinary word text, so that
shape produces a single malformed word and is already rejected (E220).

## Metadata
- **Status**: implemented
- **Last updated**: 2026-07-29 19:09 EDT

- **Error Code**: E764
- **Category**: Main tier separators
- **Level**: utterance
- **Layer**: validation
- **Kind**: Style

## Example 1

**Trigger**: a filler glued to the preceding word.

**Expected Error Codes**: E764

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	the dog&-um barked .
@Comment:	ERROR: a missing space silently split this into two words
@End
```

## Example 2

**Trigger**: a nonword glued to the preceding word.

**Expected Error Codes**: E764

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	the dog&~gaga barked .
@Comment:	ERROR: the nonword is a word of its own and needs a space
@End
```

## Example 3

**Trigger**: a phonological fragment glued to the preceding word.

**Expected Error Codes**: E764

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	the dog&+fr barked .
@Comment:	ERROR: the fragment is a word of its own and needs a space
@End
```

## Expected Behavior

- **Parser**: unaffected. Both words parse with real spans, exactly as they do
  today; no grammar, serialization, or roundtrip behavior changes.
- **Validator**: reports E764 once at the prefixed word's span, with a
  suggestion to add the space. Detected by SPAN ADJACENCY (the prefixed word
  starts at the byte where the preceding word ends), the same mechanism as
  E751 and E757, so dummy spans (the re2c oracle) are skipped and that front
  end mirrors the rule as its own token-stream scan.

## CHAT Rule

Main-tier items are space-delimited. A `&-`, `&~`, or `&+` form is a word, so
it takes a space like any other word; the absence of one is invalid even
though the parse is unambiguous.

Juxtaposition-matrix cell 6, ruled REJECT 2026-07-18. Wild-data impact at
adoption: ZERO main-tier occurrences in the kept corpus (the four whole-file
hits are all on non-main tiers), so no kept file is affected.

Parity note: CLAN CHECK has no rule for this shape. chatter is deliberately
stricter, on the modernization principle that accidental juxtaposition is the
historical bug worth closing (Java Chatter silently accepting `hello(.)` as
word plus pause is the founding case, now E751).
