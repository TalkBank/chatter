# talkbank-lsp

**Status:** Current
**Last modified:** 2026-09-06 06:32 EDT

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
Prebuilt `talkbank-lsp` binaries ship with every
[chatter release](https://github.com/TalkBank/chatter/releases/latest)
(standalone installers and per-platform archives), or build from source
with `cargo build --release -p talkbank-lsp`.

Point your editor's LSP client at the `talkbank-lsp` binary (stdio
transport) and start it with:

```bash
talkbank-lsp
```

The server communicates over stdio using the standard LSP JSON-RPC protocol.

## Lifecycle verification

The editor sends `shutdown`, waits for its response, then sends `exit`.
The standalone server exits with code 0 after successful shutdown and code 1
when `exit` arrives without successful shutdown. The editor may retain its
stdin pipe until the child exits; EOF is not required for this handshake.
The protocol contract is defined by the
[LSP lifecycle specification](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#exit).

Run the process-level regressions against the locally built executable:

```bash
cargo test -p talkbank-lsp --test position_conversion_tests stdio_lifecycle
```

These tests are part of the normal workspace gate. They cover the successful
handshake, exit before initialization or shutdown, rejected shutdown, and EOF,
with bounded waits and cleanup of child processes on failure. They join the
existing integration target to avoid another test binary.

`stdio::LifecycleService` observes successful shutdown and exit at the protocol
service boundary. Its closed session states prevent a rejected or late shutdown
from granting successful exit. Completion wakes the transport even while its
reader is idle. The standalone entrypoint then calls `shutdown_background`:
[Tokio stdin uses a blocking read that cannot be cancelled](https://docs.rs/tokio/latest/tokio/io/fn.stdin.html),
so joining runtime threads before process exit can hang indefinitely. This is
a process-entrypoint policy, not a timeout added to request handlers. Embedders
using `serve_stdio` remain responsible for their runtime's teardown.

## License

MIT OR Apache-2.0.
