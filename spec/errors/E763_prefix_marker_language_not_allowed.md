# E763: prefix marker in a language that does not use it

## Description

The prefix marker `#` separates a bound prefix from its stem (Hebrew
`ha# kelev`, Arabic `l# walad`). A legally-positioned marker is only
meaningful in a language whose orthography glues prefixes to stems; elsewhere
it is a stray character, usually a typo or a conversion artifact.

The gate reads the WORD's resolved language, never the file's `@Languages`
header. This is the same policy the digits rule (`E220`) applies, and for the
same reason: an English-headed file may legitimately contain a Hebrew word,
and that word brings its own rules with it. A word carrying its own `@s:`
marker carries its own language.

## Metadata
- **Status**: implemented
- **Last updated**: 2026-07-26 22:49 EDT

- **Error Code**: E763
- **Category**: Word validation
- **Level**: word
- **Layer**: validation

## Example 1

**Trigger**: a word-final marker in a language that does not use it.

**Expected Error Codes**: E763

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	sun# dog .
@Comment:	ERROR: English does not write the prefix marker
@End
```

## Example 2

**Trigger**: a word-internal marker in a language that does not use it.

**Expected Error Codes**: E763

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	sun#shine is nice .
@Comment:	ERROR: presence of the marker is what is gated, not its position
@End
```

## Example 3

**Trigger**: a marked word tagged as a switch to a language without the marker.

**Expected Error Codes**: E763

```chat
@UTF8
@Begin
@Languages:	heb, eng
@Participants:	CHI Target_Child
@ID:	heb, eng|corpus|CHI|||||Target_Child|||
*CHI:	ha#@s:eng kelev .
@Comment:	ERROR: the word resolves to eng, which does not use the marker
@End
```

## Expected Behavior

- **Parser**: unaffected. The marker is ordinary word text.
- **Validator**: reports E763 at the offending word, naming the resolved
  language(s). Policy decisions mirror `E220` exactly:
  - Omission words (`0word`) are skipped; the leading `0` is CHAT notation.
  - An `Unresolved` language yields an empty candidate set and the check is
    skipped, because a check that cannot know the language must not guess.
  - Mixed and ambiguous codes are permissive: if ANY candidate language uses
    the marker, the word passes.
- **Suppression**: a word whose marker is illegally positioned is reported by
  `E762` only. Reporting this code on top would name a consequence rather
  than the defect.

## CHAT Rule

Languages that write the prefix marker: `heb`, `ara`. Every other language
flags it as E763.

Wild-data impact at adoption (typed survey over every `#`-bearing corpus file,
2026-07-26): 70,654 of 70,668 attestations are Hebrew or Arabic, namely
word-final 26,811 Arabic (`l#`, `ka#`, `ta#`, AarssenBos frogs), word-final
8,041 Hebrew (`ha#`, `we#`, `še#` in Ravid; `ה#`, `ו#`, `ב#` in BermanLong),
and word-internal 35,802, all BermanLong glued forms across 268 files. The
remaining 14 are isolated word-final strays: `sun` 7, `cat` 2, `fra` 1,
`nld` 1, `bul` 1, `jpn` 1, `deu` 1. All 14 are defects and join the
data-cleanup queue.

No exception is made for `@s`-marked words. The single corpus case where a
marked word's resolved language differs from its file default is
`ה#מרקט@s:eng` in BermanLong, and that annotation plus its two siblings
(`פליי@s:eng`, `חזר@s:yid`) are data errors: Hebrew words in Hebrew script
tagged as a code switch that did not happen. The rule correctly flags them.

Word-internal markers remain legal wherever the language allows the marker at
all. Rejecting them outright is a separate change, blocked on normalizing
BermanLong's 35,802 glued forms; until then this rule is what keeps them from
appearing in languages that never had them.
