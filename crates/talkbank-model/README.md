# talkbank-model

**Status:** Current
**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/crates/talkbank-model/README.md)

TalkBank data model and validation for [CHAT format](https://talkbank.org/0info/manuals/CHAT.html).

## Overview

This crate defines the shared abstract syntax tree (AST) for supported CHAT
(Codes for the Human Analysis of Transcripts), the standard transcription
format for language research used by TalkBank. It provides:

- **Data model**: Rust types for files, headers,
  participants, utterances, words, dependent tiers (%mor, %gra, %pho, etc.),
  annotations, and more.
- **Validation**: Multi-layer validation including structural checks,
  cross-tier alignment verification, and semantic consistency rules.
- **Serialization**: CHAT writing and serde representations, with
  schema-aware JSON helpers in `talkbank-transform`. Serialization does not
  establish validity or preserve parser provenance.

The model is parser-independent: it represents the result of parsing
but does not depend on any particular parser. The Tree-sitter backend
(`talkbank-parser`) and re2c backend (`talkbank-parser-re2c`) are peers that
produce `ChatFile` values from this crate. Tree-sitter was prototyped first;
it does not own the model's design. Both parsers may drive model changes,
including explicit malformed or recovery states. Prefer extending this shared
representation to maintaining duplicate semantic ASTs. See
[backend ownership](https://github.com/TalkBank/chatter/blob/main/book/src/architecture/parser-backends.md#equal-backends-one-shared-model).

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

Use the [library recipes](https://github.com/TalkBank/chatter/blob/main/book/src/chatter/integrating/library-usage.md)
for runnable parsing, validation, model traversal and serialization examples.
Those examples are included from compiled test source rather than maintained
independently here. The [model architecture](https://github.com/TalkBank/chatter/blob/main/book/src/architecture/chat-model/chat-model.md)
describes checked construction and the mutable/accepted boundary.

`validate_with_policy` selects structural or tier-alignment coverage explicitly.
`talkbank_transform::parse_validated_with_parser` also requires source parsing
without error diagnostics. Optional-validation/recovery APIs return mutable
`ChatFile` and make no validity claim. Both mutable and accepted values can be
serialized; JSON schema validation is distinct from CHAT model validation.

`ValidChatFile` has no deserialization or mutable access path. Consume it with
`into_unchecked()` before editing and validate the resulting model again.
JSON import does not reconstruct parser trust; use the documented checked
construction path when assembling a model rather than parsing source.

## License

MIT OR Apache-2.0.
