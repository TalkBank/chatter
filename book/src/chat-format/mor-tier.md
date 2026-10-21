# The %mor Tier: Morphological Analysis

**Status:** Reference
**Last updated:** {{git-dates:page}}

The `%mor` tier records morphological analysis. Chatter parses, represents,
validates and serializes that analysis; it does not itself infer the correct
analysis from speech. A successful parse or alignment check does not certify
linguistic accuracy.

## Format Overview

```text
*CHI:	I want cookies .
%mor:	pron|I-Prs-Nom-S1 verb|want-Fin-Ind-Pres-S1 noun|cookie-Plur .
```

A morphological word has the shape `POS|lemma[-Feature]*`:

- `POS` is a category string, such as `noun`, `verb` or `pron`.
- `|` separates the category from the lemma.
- The lemma is the lexical payload.
- Each `-` introduces a feature, such as `Plur` or `Number=Plur`.

Items are space-separated. A MOR item contains a main morphological word and
zero or more post-clitics. The tier ends with a typed terminator.

## The UD MOR Format

UD-style annotation often uses lowercase category names and flat feature
values, as in the example above. This is not a claim that Chatter's model is
a complete Universal Dependencies interchange format.

The parser also accepts colon-refined categories such as `pro:sub` and
`v:aux`. `PosCategory` retains the entire tag; `category()` returns its
head and `subcategories()` iterates the refinements. Do not discard those
refinements merely because a particular producer emits flat UPOS-like tags.

### Multi-Word Tokens (Clitics)

```text
*CHI:	it's red .
%mor:	pron|it~aux|be-Fin-Ind-Pres-S3 adj|red .
```

Here the first MOR item has two morphological words: its main word and one
post-clitic separated by `~`. It still occupies one main-to-MOR item slot,
but contributes two MOR chunks for `%gra`.

### Terminator

`MorTier` requires a `Terminator`; its constructors cannot represent an
absent MOR terminator. Missing terminators in raw input are rejected during
parsing, not filled in by the model. Terminator value agreement with the main
tier is checked separately from item counts.

The terminator contributes one chunk to `%gra`. Thus the example above has
two MOR items and four chunks: `pron|it`, the auxiliary post-clitic,
`adj|red`, and the period.

## Feature Values and Keys

`MorFeature` stores an optional key and a value. These examples describe
constructor behavior, not whole-document admission:

| Constructor input | Key | Value | Serialized string |
| --- | --- | --- | --- |
| `MorFeature::new("Plur")` | absent | `Plur` | `Plur` |
| `MorFeature::new("Number=Plur")` | `Number` | `Plur` | `Number=Plur` |
| `MorFeature::new("Int,Rel")` | absent | `Int,Rel` | `Int,Rel` |

`new` splits at the first `=`. `flat` stores a keyless value explicitly;
`with_key_value` stores both parts verbatim. None of these constructors
consults a UD feature inventory. Empty values remain representable and are
checked by MOR content validation.

Commas remain part of the value string; Chatter does not turn `Int,Rel`
into a set of two semantic values. Likewise, it does not reconstruct missing
feature keys or interpret a producer's composite code such as `S3`.
A consumer needs the producer's convention for those interpretations.

Accepting keyed features syntactically does not prove that changing a
producer from flat to keyed features is compatible with every consumer.

## Compound Lemma Boundaries

The lemma is stored as an opaque `MorStem`. The current grammar permits
characters such as `=`, `!`, `_`, and `#` in interior positions,
with stricter restrictions on the first character. Structural delimiters
such as `|`, `-`, `~`, `+`, whitespace, period and question mark
still constrain tokenization; arbitrary Unicode text is not automatically
a valid MOR lemma.

Chatter does not give an interior `=` a separate translation-gloss field.
For example, a retained `perro=dog` lemma is one string, not a typed pair of
lemma and translation. Nor does the model interpret an interior marker as a
language-specific compound boundary.

A downstream generator may normalize or remove characters before Chatter
sees them. That is a separate transformation contract: inspect its exact
implementation and output rather than attributing that behavior to this
parser. Parsing cannot recover information already removed by a producer.

## Data Model

Use the public types and accessors rather than copying their declarations:

| Type | Responsibility |
| --- | --- |
| `MorTier` | Tier kind, item collection, required terminator and source span |
| `Mor` | Main morphological word and ordered post-clitics |
| `MorWord` | Category, lemma and ordered features |
| `MorFeature` | Optional feature key and value |
| `PosCategory` / `MorStem` | Interned string values |

`MorTier::items()` exposes item order; `items_mut()` exposes the existing
items as a mutable slice. `chunks()` exposes main words, post-clitics and the
terminator in their serialized order. `count_chunks()` includes all of them.
Use the chunk projection for GRA consumers rather than counting only items.

The model's ability to hold a value is not proof that it is syntactically
valid CHAT or a linguistically valid analysis. See the
[validation architecture](../architecture/errors-and-validation/validation.md) for admission boundaries.

## Grammar and Parser

The grammar owner is `grammar/grammar.js`, particularly `mor_content`,
`mor_word`, `mor_pos`, `mor_lemma` and `mor_feature_value`. The
Tree-sitter consumer lives under
`crates/talkbank-parser/src/parser/tier_parsers/mor/`; the model lives under
`crates/talkbank-model/src/model/dependent_tier/mor/`.

The grammar recognizes features as surface strings. The model splits keyed
features; this does not validate their UD meaning. Recovery diagnostics must
be considered separately from any model produced by a parser.

Reference fixtures, spec examples and focused parser/model tests provide
different correctness evidence. Merely accepting a legacy-looking string
does not establish full support for a historical MOR dialect, its semantic
structure, or every downstream analysis.

## Validation

- **E711:** MOR content validation checks empty POS, lemma and feature
  values in main words and post-clitics. This is not a UD vocabulary check.
- **E705 / E706:** too few / too many MOR items for the main-tier MOR
  projection. That projection is not the raw content array: retraces,
  replacements and tag markers need the documented domain rules.
- **E716:** mismatched terminator values.
- **E720:** MOR chunk count differs from GRA relation count, including the
  MOR terminator and post-clitics.

E707 is a historical presence-mismatch category, not a currently emitted
raw-input MOR alignment diagnostic. The retained E707 examples are subsumed
by E305 because missing terminators are handled before that alignment check.

See [dependent tiers](dependent-tiers.md) for tier domains and GRA checks, and
the [validation error guide](../chatter/user-guide/validation-errors.md) for diagnostic lookup.

## JSON Serialization

Features serialize as strings whether flat or keyed. The terminator is an
object, not a punctuation string. This selected-field example illustrates one
MOR item; it is not a complete transcript:

```json
{
  "tier_type": "Mor",
  "items": [
    {
      "main": {
        "pos": "noun",
        "lemma": "cookie",
        "features": ["Plur"]
      }
    }
  ],
  "terminator": { "type": "period" }
}
```

Use the revision-matched generated schema for complete shapes and the
[JSON integration guide](../chatter/integrating/json-schema.md) for validation
and import boundaries. Preserving surface strings does not promise lossless
conversion of arbitrary UD or traditional MOR analyses.
