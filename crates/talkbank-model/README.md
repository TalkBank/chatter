# talkbank-model

**Status:** Current
**Last modified:** 2026-05-30 07:15 EDT

TalkBank data model and validation for [CHAT format](https://talkbank.org/0info/manuals/CHAT.html).

## Overview

This crate defines the complete abstract syntax tree (AST) for CHAT
(Codes for the Human Analysis of Transcripts), the standard transcription
format for language research used by TalkBank. It provides:

- **Data model**: Rust types for every CHAT construct: files, headers,
  participants, utterances, words, dependent tiers (%mor, %gra, %pho, etc.),
  annotations, and more.
- **Validation**: Multi-layer validation including structural checks,
  cross-tier alignment verification, and semantic consistency rules.
- **Serialization**: Full serde support for JSON round-tripping via the
  `talkbank-transform` crate.

The model is parser-independent: it represents the result of parsing
but does not depend on any particular parser. Both the canonical
tree-sitter parser (`talkbank-parser`) and the alternate re2c parser
(`talkbank-parser-re2c`) produce `ChatFile` values from this crate.

## Key Types

- `ChatFile`: Root AST node representing a complete `.cha` file
- `Utterance`: A single speaker turn with main tier and dependent tiers
- `Word`: Individual word with form, category, and language metadata
- `MorTier` / `GraTier` / `PhoTier`, Morphological, grammatical relation,
  and phonological dependent tiers
- `Header`: File-level metadata (participants, languages, options)

## Usage

`ChatFile` is generic over a `ValidationState` marker
(`NotValidated` after parsing, `Validated` after the validation pass)
so that consumers can statically distinguish parser output from
validated AST values:

```rust,ignore
use talkbank_model::ChatFile;
use talkbank_model::validation::NotValidated;

// ChatFile is typically produced by a parser, not constructed
// directly. See talkbank-transform for the parse-and-validate
// pipeline.
let file: ChatFile<NotValidated> = ChatFile::default();
assert_eq!(file.utterances().count(), 0);
```

The `validate_into` method on `ChatFile<NotValidated>` produces a
`ChatFile<Validated>`, which is the only state where JSON
serialization is available.

## License

MIT OR Apache-2.0.
