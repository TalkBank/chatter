# Symbols

**Status:** Reference
**Last modified:** 2026-05-29 18:43 EDT

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

### CA (Conversation Analysis) Symbols

CA notation symbols fall into three parser-distinct categories in
`spec/symbols/symbol_registry.json`. They are not interchangeable,
the grammar treats them as different node kinds.

**CA element symbols** (`ca_element_symbols`) attach to a word, so
`book↑` is a single token whose content carries the symbol:

| Symbol | Meaning |
|--------|---------|
| `↑` | Rising pitch (attaches to a word) |
| `↓` | Falling pitch (attaches to a word) |
| `∙` | Micropause |
| `≠` | Inhalation marker |
| `⁑` `↻` `∾` `⤆` `⤇` `Ἡ` | Other CA element symbols |

**CA arrow separators** (in `word_segment_forbidden_start_symbols`)
are own-node separators between words, not word-attachments. The
parser splits them as their own nodes:

| Symbol | Meaning |
|--------|---------|
| `→` | Level pitch contour |
| `↗` | Rising-to-mid contour |
| `↘` | Falling-to-mid contour |
| `⇗` | Rising-to-high contour |
| `⇘` | Falling-to-low contour |
| `↖` `↙` `←` | Other CA arrow separators |

**CA delimiter symbols** (`ca_delimiter_symbols`) bracket annotated
prosodic regions:

| Symbol | Meaning |
|--------|---------|
| `°` | Quiet speech |
| `∆` `∇` | Higher / lower pitch register |
| `∬` `∮` | Other prosodic-region delimiters |
| `▁` `▔` | Low / high prosodic-region delimiters |
| `⁇` `§` `⁎` `↫` `◉` `☺` `♋` `Ϋ` | Additional registered CA delimiters |

Confirm the current contents of each category by reading
`spec/symbols/symbol_registry.json` directly, that is the file
`just symbols-gen` derives the grammar and Rust constants from.

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

The authoritative form-marker set is `FormType` in
`crates/talkbank-model/src/model/content/word/form.rs`. Current
variants:

| Marker | Meaning |
|--------|---------|
| `@a` | Approximate / phonologically consistent form |
| `@b` | Babbling |
| `@c` | Child-invented form |
| `@d` | Dialect form |
| `@f` | Family-specific form |
| `@fp` | Filled pause (deprecated, use `&-um` etc.) |
| `@g` | Gemination / general special form |
| `@i` | Interjection |
| `@k` | Letter sequence (kinship) |
| `@l` | Single letter |
| `@ls` | Letter plural |
| `@n` | Neologism |
| `@o` | Onomatopoeia |
| `@p` | Proper name |
| `@q` | Metalinguistic reference |
| `@sas` | Second-attempt success |
| `@si` | Singing |
| `@sl` | Slang |
| `@t` | Test word |
| `@u` | Unibet transcription |
| `@wp` | Word play |
| `@x` | Complex / excluded |
| `@z:<label>` | User-defined special form (carries an arbitrary label) |

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
