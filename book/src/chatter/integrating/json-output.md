# JSON Output Reference

**Status:** Reference
**Last updated:** 2026-05-11 23:45 EDT

This document describes the structure of JSON produced by `chatter to-json`.
For the formal JSON Schema, see [JSON Schema](json-schema.md).

## Quick Start

```bash
# Default: parse + validate + align, pretty-printed, schema-checked
chatter to-json file.cha

# Write to file
chatter to-json file.cha -o file.json

# Skip validation (parse only, faster)
chatter to-json file.cha --skip-validation

# Skip alignment only
chatter to-json file.cha --skip-alignment
```

Validation and alignment are **on by default**. Use `--skip-validation`
or `--skip-alignment` to opt out.

## Top-Level Structure

```json
{
  "lines": [ ... ]
}
```

A `ChatFile` is a flat list of `lines`. Each line has a `line_type` discriminator:

| `line_type` | Description |
|-------------|-------------|
| `"header"` | File header (`@Begin`, `@Languages`, `@Participants`, etc.) |
| `"utterance"` | Main tier + dependent tiers + alignment |
| `"comment"` | `@Comment:` lines |

## Word Fields

Words are the fundamental unit. Every word in the main tier `content` array
carries these fields:

| Field | Type | Always? | Description |
|-------|------|---------|-------------|
| `type` | `"word"` | yes | Discriminator |
| `raw_text` | string | yes | Exact text from the transcript, including all CHAT markers |
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

`cleaned_text` and `untranscribed` are **computed from `content`** during
serialization. They do not exist as stored fields in the data model.

- **`cleaned_text`**: Concatenates `Text` and `Shortening` elements from `content`.
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
    { "type": "compound_marker", "content": { "span": { "start": 0, "end": 1 } } },
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
      "terminator": "."
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
| `"Sin"` | `%sin` | Syntax tier |
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

Note that %wor items use `"kind"` instead of `"type"` for their discriminator,
since `"type"` is used by the tier envelope.

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

Alignment links each main-tier word (`source_index`) to its corresponding
dependent-tier item (`target_index`) by position. `errors` contains any
alignment-level diagnostics (count mismatches, etc.) and is `[]` when
alignment validated cleanly.

## Headers

Headers use the `header` object with a `type` discriminator:

| Type | Header | Key Fields |
|------|--------|------------|
| `"utf8"` | `@UTF8` |, |
| `"begin"` | `@Begin` |, |
| `"end"` | `@End` |, |
| `"languages"` | `@Languages` | `codes` |
| `"participants"` | `@Participants` | `entries` (speaker_code, name, role) |
| `"id"` | `@ID` | `language`, `corpus`, `speaker`, `role`, `age`, `sex`, ... |
| `"media"` | `@Media` | `filename`, `media_type`, `status` |
| `"comment"` | `@Comment` | `text` |
| `"date"` | `@Date` | `date` |
| `"options"` | `@Options` | `options` (array of strings) |

See the [JSON Schema](json-schema.md) for the complete list of header types and fields.

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
