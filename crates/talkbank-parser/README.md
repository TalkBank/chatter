# talkbank-parser

**Status:** Current
**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/crates/talkbank-parser/README.md)

Parsing implementations for CHAT format using tree-sitter.

## Overview

This crate is the Tree-sitter backend for [CHAT format](https://talkbank.org/0info/manuals/CHAT.html),
converting source text into the `ChatFile` AST defined by
[talkbank-model](https://github.com/TalkBank/chatter/blob/main/crates/talkbank-model/README.md). It uses the
[tree-sitter-talkbank](https://github.com/TalkBank/chatter/blob/main/grammar/README.md) grammar
crate from the same repository to produce a concrete syntax tree (CST), then
converts it into the model's abstract types.

It is a peer of `talkbank-parser-re2c`. Tree-sitter was prototyped first and is
the current CLI default; neither fact grants it ownership of the shared model.
Both backends can drive changes to shared syntax and recovery types. See
[backend ownership](https://github.com/TalkBank/chatter/blob/main/book/src/architecture/parser-backends.md#equal-backends-one-shared-model).

Key features:

- **Error recovery**: The GLR-based tree-sitter parser recovers from syntax
  errors and produces partial results, making it suitable for editor
  integration (LSP) and interactive use.
- **Explicit parser handle**: Create a `TreeSitterParser` once and reuse it
  for all parsing in a scope. No hidden global state.
- **Granular parsing**: `TreeSitterParser` methods parse at any level:
  individual words, tiers, headers, or complete files.

## Usage

Start with the [compiled library recipes](https://github.com/TalkBank/chatter/blob/main/book/src/chatter/integrating/library-usage.md)
for dependency setup, parsing, validation, traversal and serialization.

`TreeSitterParser::new()` is fallible. Its inherent `parse_chat_file()` returns
a shared `talkbank_model::ParseProduct` (also re-exported here): `Built` retains both a model and its diagnostics, while
`Unbuildable` retains diagnostics without a model. Match both variants and
inspect the diagnostics; building a model is not source acceptance. The
`expect_built()` helper is for tests, panics on `Unbuildable`, and discards
diagnostics on `Built`. It is not a production validation shortcut.

The shared `ChatParser` trait and fragment APIs instead return `ParseOutcome`
and stream diagnostics to an `ErrorSink`. Fragment origins and option context
must describe the actual caller input. See
[parser backends](https://github.com/TalkBank/chatter/blob/main/book/src/architecture/parser-backends.md) for the
API distinction and compatibility contract. Use the transform crate's checked
pipeline when the consumer needs admitted source and model validity rather
than recovery-oriented parsing.

## License

MIT OR Apache-2.0.
