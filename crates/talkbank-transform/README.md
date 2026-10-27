# talkbank-transform

**Status:** Current
**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/crates/talkbank-transform/README.md)

Transformation pipelines for [CHAT format](https://talkbank.org/0info/manuals/CHAT.html) (CHAT to JSON, normalization, validation).

## Overview

This crate provides high-level pipeline functions that compose parsing,
validation, and transformation operations for CHAT files. It is the primary
entry point for applications that need to process `.cha` files end-to-end.

Key capabilities:

- **Parse + optional validation**: `parse_and_validate()` returns a mutable
  `ChatFile`; `ParseValidateOptions` determines whether model validation runs
  and whether it includes tier alignment. Use `parse_validated_with_parser()`
  when the caller needs a retained `ValidChatFile` capability instead.
- **CHAT to JSON**: `chat_to_json()` checks serialized JSON against the
  embedded schema. CHAT validation remains a separate option; schema success
  does not establish CHAT validity, parser provenance or original source bytes.
- **Normalization**: `normalize_chat()` returns a checked `Rewrite`, not a
  string. Its source-loss check can refuse serialization even when CHAT is
  valid; it is not a universal byte-preservation or safe-overwrite guarantee.
- **Caching**: `UnifiedCache` provides SQLite-based caching for validation
  and round-trip results for large file collections.
- **Parallel validation**: `validate_directory_streaming()` takes a
  `ValidationRun` and returns an event receiver and cancellation handle.
  Inspect terminal outcomes as well as individual file results; receiving
  events does not establish that every file completed. Streaming here means
  progress/results, not incremental parsing of a file's input bytes.

## Usage

Start with the [library recipes](https://github.com/TalkBank/chatter/blob/main/book/src/chatter/integrating/library-usage.md)
for dependency setup, parsing, validation, traversal and serialization. Their
examples are included from compiled test source rather than copied into this
README. Keep the model, parser and transform dependencies at one coherent
repository revision.

Content-only convenience functions use anonymous transcript identity. If input
came from a file, use the path-aware or named entry point so checks involving
the transcript name are not silently omitted. Parsing failures, internal faults,
model validation failures and incomplete provenance remain distinct outcomes.
Do not discard diagnostics or convert all failures into a successful model.

The [transform pipeline guide](https://github.com/TalkBank/chatter/blob/main/book/src/architecture/transform-pipeline.md)
describes mutable models, retained admission, rewrite limits and runner/cache
ownership. Use fresh output destinations when learning conversion; a returned
model, JSON string or rewrite does not by itself authorize replacing the input.

## License

MIT OR Apache-2.0.
