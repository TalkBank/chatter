# JSON Output Reference

**Status:** Reference
**Last updated:** {{git-dates:page}}

This document describes the structure of JSON produced by `chatter convert to-json`.
For the formal JSON Schema, see [JSON Schema](json-schema.md).
Examples below are selected fields, not complete transcript fixtures. Ellipses
are schematic placeholders, not JSON syntax; omitted fields and source spans
must not be inferred from these illustrations.

## Quick Start

```bash
# Default: parse + validate + align, pretty-printed, schema-checked
chatter convert to-json file.cha

# Write to file
chatter convert to-json file.cha -o file.json

# Skip validation (parse only, faster)
chatter convert to-json file.cha --skip-validation

# Skip alignment only
chatter convert to-json file.cha --skip-alignment
```

Validation and alignment are **on by default**. Use `--skip-validation`
or `--skip-alignment` to opt out.

## Top-Level Structure

```json
{
  "lines": [ ... ]
}
```

A `ChatFile` preserves file order in `lines`. It also serializes the
`participants`, `languages`, and `options` metadata projections and, when
present, `media`. Each line has a `line_type` discriminator:

| `line_type` | Description |
|-------------|-------------|
| `"header"` | File header (`@Begin`, `@Languages`, `@Participants`, etc.) |
| `"utterance"` | Main tier + dependent tiers + alignment |

`@Comment:` is a `"header"` line whose `header.type` is `"comment"`, not
a third line variant. Its payload is structured `content` (`BulletContent`),
not a plain `text` field.

## Word Fields

Words are the fundamental unit. Every word in the main tier `content` array
carries these fields:

| Field | Type | Always? | Description |
|-------|------|---------|-------------|
| `type` | `"word"` | yes | Discriminator |
| `raw_text` | string | yes | Current CHAT spelling derived from typed structure, including markers |
| `cleaned_text` | string | yes | NLP-ready text (shortenings restored, markers stripped) |
| `content` | array | yes | Structured breakdown of word parts (see below) |
| `category` | string | no | `"omission"`, `"filler"`, `"nonword"`, `"fragment"`, `"ca_omission"` |
| `form_type` | string | no | Special form code: `"c"`, `"d"`, `"f"`, `"x"`, etc. |
| `lang` | object | no | Language marker (see Language-Switched example) |
| `untranscribed` | string | no | `"unintelligible"` (xxx), `"phonetic"` (yyy), `"untranscribed"` (www) |

Word content items use `"content"` for the text value:

```json
{ "type": "text", "content": "dog" }
```

### Computed Fields

`raw_text`, `cleaned_text` and `untranscribed` are computed views, not
independently authoritative input fields. Imported values cannot override the
typed content, category, form, language or part-of-speech markers. Import still
requires model validation; ignoring a stale display field does not admit
malformed structural fields.

`raw_text` is not a byte-exact source slice. In particular, recovery may retain
a partial typed word while separately reporting rejected source material.
Source-coordinate consumers must retain the original source and use its spans;
they must not index derived spelling with offsets into that source.

Rust callers use `Word::new(location, WordText)` for a nonempty plain-text model, then
explicit typed builders for markers. Nonempty construction alone is not full
CHAT admission. For external CHAT/ASR tokens, use `ChatParser::parse_word`
and handle rejection explicitly instead of falling back to unchecked construction.
There are no independent raw/cleaned constructor arguments and no `set_raw_text`.
`raw_text()` returns an owned string; use `WriteChat::write_chat` or
`Display` when streaming avoids an intermediate allocation. JSON serialization
streams that display projection directly. The cleaned-text cache is
invalidated by content mutation; no raw-text cache can become stale after
public marker mutation.

- **`cleaned_text`**: Concatenates spoken `Text`, `Shortening`, and `Phonetic`
  elements from `content`. Text inside CA segment-repetition delimiters
  (`↫ ... ↫`) is retained in the model but excluded from this projection.
  Excludes lengthening markers (`:`), stress markers, CA elements, overlap points,
  compound markers, and underline markers. Example: `sit(ting)` → `"sitting"`.

- **`untranscribed`**: Present only when `cleaned_text` is `"xxx"`, `"yyy"`, or `"www"`.

## Word Examples

### Simple Word

```text
dog
```
```json
{
  "type": "word",
  "raw_text": "dog",
  "cleaned_text": "dog",
  "content": [{ "type": "text", "content": "dog" }]
}
```

### Filler

```text
&-uh
```
```json
{
  "type": "word",
  "raw_text": "&-uh",
  "cleaned_text": "uh",
  "content": [{ "type": "text", "content": "uh" }],
  "category": "filler"
}
```

### Untranscribed

```text
xxx
```
```json
{
  "type": "word",
  "raw_text": "xxx",
  "cleaned_text": "xxx",
  "content": [{ "type": "text", "content": "xxx" }],
  "untranscribed": "unintelligible"
}
```

### Compound

```text
ice+cream
```
```json
{
  "type": "word",
  "raw_text": "ice+cream",
  "cleaned_text": "icecream",
  "content": [
    { "type": "text", "content": "ice" },
    { "type": "compound_marker", "content": {} },
    { "type": "text", "content": "cream" }
  ]
}
```

### Omission

```text
0she
```
```json
{
  "type": "word",
  "raw_text": "0she",
  "cleaned_text": "she",
  "content": [{ "type": "text", "content": "she" }],
  "category": "omission"
}
```

### Nonword

```text
&~baba
```
```json
{
  "type": "word",
  "raw_text": "&~baba",
  "cleaned_text": "baba",
  "content": [{ "type": "text", "content": "baba" }],
  "category": "nonword"
}
```

### Special Form

```text
doggy@c
```
```json
{
  "type": "word",
  "raw_text": "doggy@c",
  "cleaned_text": "doggy",
  "content": [{ "type": "text", "content": "doggy" }],
  "form_type": "c"
}
```

### Language-Switched

```text
maison@s:fra
```
```json
{
  "type": "word",
  "raw_text": "maison@s:fra",
  "cleaned_text": "maison",
  "content": [{ "type": "text", "content": "maison" }],
  "lang": { "type": "explicit", "code": "fra" }
}
```

The `lang` field has variants: `{"type": "shortcut"}` (bare `@s`),
`{"type": "explicit", "code": "fra"}` (`@s:fra`), and
`{"type": "multiple", "code": ["eng", "zho"]}` (`@s:eng+zho`).
The distinct `"ambiguous"` variant uses the same array shape for
`@s:eng&zho`; it does not mean the languages were mixed.

A multi-word switch is an ANNOTATION on the group, not a field on each word:
`<how to do it> [@s]` serializes as an annotation
`{"type": "code_switch", "kind": "shortcut"}`, and `[@s:hin]` as
`{"type": "code_switch", "kind": "explicit", "code": "hin"}`. The words inside
omit `lang` unless they carry a suffix of their own, so a consumer
reading only `lang` will under-report language switches. Read
`language_metadata` instead, which is where the resolved answer lives for
every word regardless of which mark produced it.

## Utterances

An utterance line contains:

```json
{
  "line_type": "utterance",
  "main": {
    "speaker": "CHI",
    "content": {
      "content": [ ... ],
      "terminator": { "type": "period" },
      "bullet": { "start_ms": 0, "end_ms": 3042 }
    }
  },
  "dependent_tiers": [ ... ],
  "alignments": { ... },
  "utterance_language": { "status": "resolved_default", "code": "eng" },
  "language_metadata": { ... }
}
```

Key structural points:

- The utterance body is under `"main"`, not `"utterance"`.
- `content`, `terminator`, and `bullet` are nested inside `main.content`.
- `terminator` is an object with a `type` field (`"period"`, `"question"`,
  `"exclamation"`, etc.), not a bare string.
- `bullet` (utterance-level timing) is inside `main.content`, **omitted
  when absent** (not present as `null`).
- `dependent_tiers`, `alignments`, `utterance_language`, and
  `language_metadata` are top-level siblings of `main`. Empty
  `dependent_tiers` and `alignments` are omitted when there is nothing
  to report.

### `language_metadata`

The `language_metadata` field is omitted when language analysis has not run,
as in a parse-only export. If present, inspect `language_metadata.status`.
An explicit `"uncomputed"` value likewise means the analysis has not run;
it is not an empty result and must not be
replaced with a guessed language. A `"computed"` value contains `metadata`,
with one entry per word in main-tier traversal order under
`language_metadata.metadata.word_languages`:

```json
"language_metadata": {
  "status": "computed",
  "metadata": {
    "tier_language": "zho",
    "word_languages": [
      { "languages": { "Single": "zho" }, "source": "default" },
      { "languages": { "Single": "eng" }, "source": "word_shortcut" }
    ]
  }
}
```

The explicit uncomputed form is `{"status": "uncomputed"}`, with no `metadata`
field; normal transcript serialization omits that state instead.
Computation also does not guarantee that a language could be resolved: consume
each entry's language state rather than substituting the tier language.
The `languages` wire forms are case-sensitive: `{"Single": "eng"}`,
`{"Multiple": ["eng", "zho"]}`, `{"Ambiguous": ["eng", "zho"]}`, or the
string `"Unresolved"`. These differ from the lowercase word-suffix `lang` tags.

Properties of computed metadata:

- **Every word at any depth**, including words inside quotations,
  phonological groups, sign groups and retraces. A retraced word was spoken
  and has a language, so it gets an entry.
- **The produced form only.** For `dog [: cat]` the entry describes `dog`,
  what the speaker actually said, not the correction.
- **`source` says which mark decided the language.** A span and a word suffix
  can resolve to the identical CODE, so a consumer asking "did the transcriber
  mark this word, or the stretch around it?" can only answer from this field.
  Precedence is innermost-first: the word's own mark beats an enclosing span,
  which beats the utterance. The values are enumerated with a description each
  in `schema/chat-file.schema.json`, generated from the enum; they are
  deliberately not copied here, because a copy would go stale.
- **Position is the array subscript**, and nothing else. There is no index
  field: read it with the equivalent of `enumerate()`. In particular this
  order is **not** an alignment index. The tier domains disagree about what
  they count (`%mor` excludes retraces, `%pho` counts them), so correlating
  with `%mor` or `%gra` positions must go through `alignments`, not through
  this order. There is no `word_index` field, because it would be derivable and misleading.

### Content Items

`main.content.content` is a heterogeneous array. Each item has a `type` discriminator:

| Type | Description |
|------|-------------|
| `"word"` | A word token (see Word Fields above) |
| `"event"` | Non-verbal action (`&=laughs`) |
| `"pause"` | Timed or untimed pause (`(.)`, `(0.5)`) |
| `"group"` | Bracketed group (`<word word>`) |
| `"separator"` | Tag markers, linkers, etc. |

## Dependent Tiers

When present, `dependent_tiers` is an **array** of tagged objects:

```json
"dependent_tiers": [
  {
    "type": "Mor",
    "data": {
      "tier_type": "Mor",
      "items": [
        {
          "main": { "pos": "pron", "lemma": "I" }
        },
        {
          "main": { "pos": "verb", "lemma": "want", "features": ["Fin", "Ind", "Pres"] }
        }
      ],
      "terminator": { "type": "period" }
    }
  },
  {
    "type": "Gra",
    "data": {
      "tier_type": "Gra",
      "relations": [
        { "index": 1, "head": 2, "relation": "NSUBJ" },
        { "index": 2, "head": 0, "relation": "ROOT" }
      ]
    }
  }
]
```

| `type` | Tier | Description |
|--------|------|-------------|
| `"Mor"` | `%mor` | Morphological analysis (POS tags, lemmas, features, clitics) |
| `"Gra"` | `%gra` | Grammatical relations (dependency arcs) |
| `"Pho"` | `%pho` | Phonological transcription |
| `"Sin"` | `%sin` | Gesture and sign annotation |
| `"Wor"` | `%wor` | Word-level timing (items with `inline_bullet`) |
| Other | `%xxx` | User-defined dependent tiers |

### %wor Tier

The `Wor` tier contains word items with timing:

```json
{
  "type": "Wor",
  "data": {
    "items": [
      {
        "kind": "word",
        "raw_text": "hello",
        "cleaned_text": "hello",
        "content": [{ "type": "text", "content": "hello" }],
        "inline_bullet": { "start_ms": 100, "end_ms": 300 }
      }
    ],
    "terminator": { "type": "period" }
  }
}
```

`WorItem` uses `"kind"` instead of the main-tier content discriminator
`"type"`. A `%wor` tier can also contain untimed separators, for example
`{"kind": "separator", "text": ","}`; not every item is a timed word.

## Alignment Data

When validation runs (the default), the `alignments` object contains:

- `units`: per-tier index arrays (for internal bookkeeping)
- Named tier pairs (e.g., `mor`, `gra`) with alignment mappings

```json
"alignments": {
  "units": {
    "main_mor": [{"index": 0}, {"index": 1}],
    "main_pho": [{"index": 0}, {"index": 1}],
    "main_sin": [{"index": 0}, {"index": 1}],
    "main_wor": [{"index": 0}, {"index": 1}],
    "mor": [{"index": 0}, {"index": 1}]
  },
  "mor": {
    "pairs": [
      { "source_index": 0, "target_index": 0 },
      { "source_index": 1, "target_index": 1 }
    ],
    "errors": []
  }
}
```

For `%mor`, each pair links positions in the main-tier alignable sequence
(`source_index`) and the MOR item sequence (`target_index`). These are not
raw content-array or language-metadata indices. Other tier pairs have their
own domains: `%gra` aligns MOR chunks to grammatical relations; `%wor`
uses `wor_timings`, a timing sidecar, rather than a structural alignment pair.
Placeholder pairs can have a null index on one side. `errors` contains any
alignment-level diagnostics (count mismatches, etc.) and is `[]` when
alignment validated cleanly.

## Headers

Headers use the `header` object with a `type` discriminator:

| Type | Header | Key Fields |
|------|--------|------------|
| `"utf8"` | `@UTF8` | none |
| `"begin"` | `@Begin` | none |
| `"end"` | `@End` | none |
| `"languages"` | `@Languages` | `codes` |
| `"participants"` | `@Participants` | `entries` (`speaker_code`, `details`) |
| `"id"` | `@ID` | `language`, `corpus`, `speaker`, `role`, `age`, `sex`, ... |
| `"media"` | `@Media` | `filename`, `media_type`, `status` |
| `"comment"` | `@Comment` | `content` (`BulletContent`) |
| `"date"` | `@Date` | `date` |
| `"options"` | `@Options` | `options` (array of strings) |

See the [JSON Schema](json-schema.md) for the complete list of header types and fields.

Participant declarations keep the observed name and role inside `details`:

```json
{
  "speaker_code": "CHI",
  "details": { "state": "complete", "role": "Child" }
}
```

The `"complete"` state may also include a `name`. A declaration containing only
a speaker code has `"details": {"state": "missing_role"}`; it does not acquire
a role from an `@ID` header or a default value. This is a recovery state, not a
valid declaration. The top-level `participants` metadata projection uses
`code` instead of `speaker_code` and also carries `details` alongside `id`.
Deserialization and schema shape checks do not replace transcript validation.

## Timing

Utterance-level timing appears in `main.content.bullet`:

```json
"bullet": {
  "start_ms": 1234,
  "end_ms": 5678
}
```

Word-level timing (from %wor tier) appears in `inline_bullet` on individual
words within the `Wor` dependent tier.
