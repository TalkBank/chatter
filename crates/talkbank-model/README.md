# talkbank-model

**Status:** Current
**Last updated:** 2026-09-04 06:45 EDT

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

`ChatFile` is the mutable AST, used by parsers, editors, recovery and transforms.
`validate_into` consumes it and returns `Result<ValidChatFile, ValidationFailure>`.
The accepted result owns a read-only model and records the rule selection,
alignment coverage, transcript name and warnings. Errors or incomplete parse
provenance return the rejected model and diagnostics for repair.

```rust,ignore
use talkbank_model::{ChatFile, ErrorCollector};
use talkbank_model::model::TranscriptName;

let parsed: ChatFile = /* parser or builder output */;
let valid = parsed.validate_into(&ErrorCollector::new(), TranscriptName::Anonymous)?;
let read_only = valid.document();
// Consume the proof before changing the model:
let mut editable: ChatFile = valid.into_unchecked();
```

`validate_with_policy` selects structural or tier-alignment coverage explicitly.
`talkbank_transform::parse_validated_with_parser` also requires source parsing
without error diagnostics. Optional-validation/recovery APIs return mutable
`ChatFile` and make no validity claim. Both mutable and accepted values can be
serialized; JSON schema validation is distinct from CHAT model validation.

Migration: remove the `ChatFile<S>` parameter and the `NotValidated`, `Validated`
and `ValidationState` imports. Handle `validate_into` rejection instead of
checking an unrelated sink. `ValidChatFile` has no deserialization or mutable
access path. Existing serialized transcript fields are unchanged.

## License

MIT OR Apache-2.0.
