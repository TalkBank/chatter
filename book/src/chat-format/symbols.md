# Symbols

**Status:** Reference
**Last modified:** 2026-08-21 13:42 EDT

CHAT uses a rich set of symbols for transcription conventions. This
page documents the symbol categories and the symbol registry that
drives both the grammar and the Rust crates. The
[symbol registry](https://github.com/TalkBank/chatter/blob/main/spec/symbols/symbol_registry.json)
(`spec/symbols/symbol_registry.json`) is the source of truth, when
this page and the registry disagree, the registry wins.

## Symbol Registry

The authoritative symbol definitions live in `spec/symbols/symbol_registry.json`. This JSON file is the single source of truth, it generates:

- Character sets for the tree-sitter grammar (`grammar.js`)
- Rust constants for the model and validation crates
- Validation rules for the spec tool

After any change to the symbol registry, run:

```bash
just symbols-gen
```

## Symbol Categories

### Terminators

Punctuation that ends an utterance:

| Symbol | Name | Usage |
|--------|------|-------|
| `.` | Period | Declarative |
| `?` | Question | Interrogative |
| `!` | Exclamation | Exclamatory |
| `+...` | Trailing off | Incomplete utterance |
| `+..?` | Trailing-off question | Question trails off |
| `+/.` | Interruption | Speaker interrupted by another |
| `+//.` | Self-interruption | Speaker interrupts self |
| `+/?` | Interrupted question | Question interrupted |
| `+!?` | Broken question | Exclamation-question |
| `+"/.` | Quoted new line | Quotation continues on next line |

### CA and Disfluency Symbols

The tables below are GENERATED from `spec/symbols/symbol_registry.json`, which
is the single owner of what each symbol means. The Rust types
`CAElementType` and `CADelimiterType`, the grammar's character constants and
these tables all come from the same record, so they cannot disagree.

**The category names describe a PARSING ROLE, not a provenance.** A
`ca_element_symbol` attaches to a word token; a `ca_delimiter_symbol` brackets
a stretch. Ask a symbol's `notation_family()` for provenance; never read it off
the name of the array the symbol sits in. That confusion is what once filed the
blocking and segment-repetition disfluency marks as Conversation Analysis
notation.

{{#include generated/ca-symbols.md}}

### CA arrow separators

These are own-node separators between words rather than word-attachments, and
the parser splits them as their own nodes. They are NOT yet registry-owned, and
this table is still hand-written. They are not untyped: five of them are
`Separator` variants in `talkbank-model`, whose glyph table is hand-written
again in `WriteChat` and in several places across the grammar and the re2c
backend. Bringing them into the registry is the same move the two families
above have already made, and it is outstanding work rather than a decision.

| Symbol | Codepoint | Meaning |
|--------|-----------|---------|
| `→` | U+2192 | Level pitch contour |
| `↗` | U+2197 | Rising to mid |
| `↘` | U+2198 | Falling to mid |
| `⇗` | U+21D7 | Rising to high |
| `⇘` | U+21D8 | Falling to low |
| `↖` `↙` `←` | U+2196, U+2199, U+2190 | Registered as separators; named in neither the CHAT manual's symbol table nor CLAN's symbol enum. |

### Word Segment Characters

Characters that are forbidden at the start of words, forbidden in the rest of words, or forbidden throughout. These define the lexical boundaries of what constitutes a "word" in CHAT.

The grammar uses these sets to construct the word-matching regex patterns. Characters like `[`, `]`, `<`, `>`, `(`, `)` are structural delimiters and cannot appear inside words.

### Event Segment Characters

Characters forbidden in event descriptions (`&=event` content). Events have slightly different lexical rules than words.

## Language Codes

CHAT uses ISO 639-3 three-letter language codes in `@Languages` headers and `@s:` word markers:

```chat
@Languages:	eng, fra
*CHI:	I want a croissant@s:fra .
```

Common codes: `eng` (English), `fra` (French), `deu` (German), `spa` (Spanish), `zho` (Mandarin), `jpn` (Japanese).

## Special Markers

### @ Markers (Word-Level)

The form-marker set has ONE owner:
`spec/form_markers/form_marker_registry.json`. The `FormType` enum, both
directions of its marker mapping, the re2c lexer's code set and the table below
are all generated from it, so a marker cannot exist in one and not another.

{{#include generated/form-markers.md}}

Every meaning above is taken from the "Special Form Markers" table in the CHAT
manual, and each links to that marker's own anchor there. They were corrected
wholesale on 2026-08-11: six had been glossed with plausible expansions of the
letters rather than their actual meanings, so `@k` read as "kinship" (it is
"kana", multiple letters), `@p` as "proper name" (it is a phonologically
consistent form), `@sl` as "slang" (it is signed language), `@sas` as
"second attempt success" (it is sign and speech), `@g` as "gemination" (it is
the general special form), and `@ls` as "letter sequence" (it is the letter
plural; the sequence is `@k`). If you find another that disagrees with the
manual, the manual wins.

`@a` was removed on 2026-08-11. The corpus authority eliminated it from every
file on 2024-09-03 together with `@e` and `@lp`; the other two were dropped
from chatter at the time and `@a` was overlooked. It has no main-tier
occurrences in any corpus, and appears in neither `depfile.cut` nor the
manual's table.

The second-language qualifier `@s:LANG` is a separate construct (see
the L2 morphotag section of the Batchalign book); it is not part of
`FormType`.

### & Markers (Events and Fillers)

| Prefix | Meaning |
|--------|---------|
| `&=` | Paralinguistic event (e.g., `&=laughs`) |
| `&-` | Filler (e.g., `&-um`) |
| `&+` | Phonological fragment (e.g., `&+sh`) |
| `&~` | Nonword (e.g., `&~mama`) |
| `&*` | Other speaker's speech event (e.g., `&*MOT:word`, speech attributed to another speaker) |

### Scope Markers

| Marker | Meaning |
|--------|---------|
| `[/]` | Partial retrace, speaker repeats the same words |
| `[//]` | Full retrace, speaker restarts with different words |
| `[///]` | Multiple retracing, multiple false starts |
| `[/-]` | Reformulation, speaker rephrases with different structure |
| `[*]` | Error |
| `[?]` | Best guess |
| `[>]` | Overlap follows |
| `[<]` | Overlap precedes |
| `[= text]` | Explanation |
| `[: text]` | Replacement |
