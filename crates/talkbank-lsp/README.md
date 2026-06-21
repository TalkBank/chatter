# talkbank-lsp

**Status:** Current
**Last modified:** 2026-06-15 20:54 EDT

[Language Server Protocol](https://microsoft.github.io/language-server-protocol/) implementation for [CHAT format](https://talkbank.org/0info/manuals/CHAT.html).

## Overview

`talkbank-lsp` is both a library (reusable IDE server implementation) and a
standalone stdio binary (`talkbank-lsp`) for CHAT transcription files via the
Language Server Protocol. It uses tree-sitter for incremental parsing and the
`talkbank-model` validation pipeline for real-time diagnostics.

## Features

- **Diagnostics**: real-time validation errors and warnings as you type
- **Hover**: alignment timing, speaker info, and error explanations
- **Completion**: speaker codes, header keywords, and coding symbols
- **Code actions**: quick fixes for auto-fixable validation errors
- **Semantic highlighting**: tree-sitter-query-driven token coloring (`src/highlight.rs` + `queries/highlights.scm`, exposed via `semantic_tokens.rs`)
- **Document formatting**: canonical CHAT normalization
- **Go to definition / references**: navigate speaker and tier relationships

## Editor Integration

Any editor with Language Server Protocol support can use `talkbank-lsp`.
Point your editor's LSP client at the `talkbank-lsp` binary (stdio
transport) and start it with:

```bash
talkbank-lsp
```

The server communicates over stdio using the standard LSP JSON-RPC protocol.

## License

MIT OR Apache-2.0.
