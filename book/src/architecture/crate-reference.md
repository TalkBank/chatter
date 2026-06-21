# Crate Reference

**Status:** Current
**Last modified:** 2026-06-15 15:00 EDT

Summary of the main crates and packages in `TalkBank/chatter`.

## Foundational crates

### tree-sitter-talkbank

Rust binding crate for the generated TalkBank CHAT tree-sitter grammar. Exposes
`LANGUAGE`, `NODE_TYPES`, and the generated query constants used by editor and
parser integrations.

### talkbank-model

The typed data model for CHAT files. Defines `ChatFile`, `Utterance`, `DependentTier`, `MorTier`, `GraTier`, and all other AST types. Includes validation logic, the `WriteChat` trait for CHAT serialization, serde support for JSON, and `JsonSchema` derivations. Also owns error types (`ParseError`, `ErrorSink` trait, `Span`, `SourceLocation`), diagnostic infrastructure, and `ParseValidateOptions`. Provides a closure-based content walker (`walk_words` / `walk_words_mut`) that centralizes recursive traversal of `UtteranceContent` and `BracketedItem` with domain-aware group gating.

### talkbank-derive

Procedural macros for the model crate (`SemanticEq`, `SemanticDiff`, `SpanShift`, `ValidationTagged`, and the `error_code_enum` macro).

### talkbank-cache

SQLite-backed validation and roundtrip cache used by higher-level validation and
corpus workflows.

### talkbank-parser

The canonical parser. Wraps the tree-sitter C parser and converts the concrete
syntax tree (CST) into `ChatFile` model types. Provides error recovery via
tree-sitter's GLR algorithm and is the parser used by the CLI, LSP, transform
pipelines, and editor tooling.

### talkbank-parser-re2c

Independent alternate parser used as an equivalence oracle against the
tree-sitter parser. Primarily a testing and spec-hardening tool rather than a
first-wave end-user surface.

### talkbank-transform

High-level pipelines: parse+validate, CHAT-to-JSON, JSON-to-CHAT, normalization. Integrates the validation cache, JSON schema validation, and parallel directory validation.

## Application and integration surfaces

### talkbank-cli

The `chatter` CLI binary: validate, normalize, to-json, and corpus management.

### talkbank-lsp

Language Server Protocol server with tree-sitter incremental parsing, real-time diagnostics, and semantic highlighting.

### send2clan

Rust bindings for sending files to the CLAN application (macOS Apple Events,
Windows WM_APP). The crate exposes the safe `send2clan` API directly while
keeping the raw FFI in private modules.

### chatter-desktop

Desktop validation app (Tauri v2, React). Mandates TUI parity with the CLI.

## Test and spec-support crates

### talkbank-parser-tests

Parser tests. Runs the parser over the reference corpus and validates the
results. Also owns spec-generated tests, roundtrip tests, equivalence tests,
and property tests.

### spec/tools

Generator binaries for tree-sitter corpus tests, generated Rust tests, shared
spec artifacts, and error documentation.

### spec/runtime-tools

Runtime-aware spec tooling for validation, bootstrap, and corpus-mining tasks
that should not live in the root Rust workspace.
