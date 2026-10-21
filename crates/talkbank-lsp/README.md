# talkbank-lsp

**Status:** Current
**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/crates/talkbank-lsp/README.md)

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

Follow the book's [Editor Integration tutorial](../../book/src/chatter/user-guide/editor-integration.md)
for installation, an actual Neovim configuration, an edit/diagnostic exercise
and troubleshooting. Use stdio transport, `*.cha` files and the `chat` language
identifier. Let the client launch the selected executable; running it alone in
a terminal waits for protocol input rather than opening an editor.

Standard features depend on the client's capabilities. Specialized `talkbank/*`
commands require client-side integration; they are not automatically exposed
by every generic LSP client.

## Lifecycle verification

The [LSP testing guide](../../book/src/contributing/lsp-testing.md) owns the
manual drill, logging limits, focused process-test commands and shutdown/exit
contract. The server library remains a thin protocol adapter over shared CHAT
semantics; embedders are responsible for their runtime and logging setup.

## License

MIT OR Apache-2.0.
