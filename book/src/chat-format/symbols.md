# Symbols

**Status:** Reference
**Last modified:** {{git-dates:page}}

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
just regen
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
| `+!?` | Broken question | Broken-off question |
| `+"/.` | Quoted new line | Quotation continues on next line |
| `+".` | Quoted period | Quoted utterance with period |
| `+//?` | Self-interrupted question | Speaker interrupts their own question |
| `+.` | Break for coding | Coding break |

### CA and Disfluency Symbols

The tables below are GENERATED from `spec/symbols/symbol_registry.json`, which
is the single owner of what each symbol means. The Rust types
`CAElementType` and `CADelimiterType`, the grammar's character constants and
these tables come from the same record. Regeneration and currency checks keep
the checked-in consumers synchronized; a shared generator does not by itself
prove that generated files are current.

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

### Word Segment Characters

Characters that are forbidden at the start of words, forbidden in the rest of words, or forbidden throughout. These define the lexical boundaries of what constitutes a "word" in CHAT.

The grammar uses these sets for ordinary word segments, not for every component
of a structured word. For example, parentheses delimit a shortening inside a
word: `walkin(g)`. See [word syntax](word-syntax.md); a segment exclusion must
not be treated as a ban on that character anywhere in the complete word.

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
are generated from it. Regenerate and check the derived files together when
changing the registry.

{{#include generated/form-markers.md}}

The registry records its authorities: the CHAT manual supplies meanings,
while the sanctioned set follows `depfile.cut`, with disagreements recorded
on the relevant row. Consult those records rather than inferring a meaning
from the marker's letters. `@a`, `@e` and `@lp` are not members of this
registry; that local inventory makes no claim about their frequency in corpora.

The second-language qualifier `@s:LANG` is a separate construct (see
[word syntax](word-syntax.md)); it is not part of
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
