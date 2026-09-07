# Panic audit: talkbank-lsp

**Status:** Reference
**Last updated:** 2026-09-06 23:24 EDT

See [README](README.md) for the shared policy. This review covers the
explicit initialization and execute-command routing panic surfaces. It does
not establish that every implicit indexing or allocation panic is impossible.

## Language-service initialization

Thread-local parser and highlighter initialization uses `OnceCell<Result<...>>`.
The initialized result is returned by the cell; there is no independent
`Option` followed by an initialization `expect`. Initialization failures stay
cached and propagate through the existing diagnostic/request error boundary.

The highlighter needs mutable access. Its `RefCell` uses `try_borrow_mut()`;
nested access returns `HighlightFailed` and the outer request can continue.
The existing repeated-highlighting test reproduced a `RefCell` panic before
this change and now covers nested refusal followed by successful outer use.

## Execute-command routing

Decoded `ExecuteCommandRequest` contains a document, participant or CHAT-op
request enum. Each service accepts only its own enum and matches it
exhaustively. The separate family tag, flat request dispatch and three
`unreachable!` fallbacks are removed. Adding a request to one family now
requires handling it in that service; another family's requests do not type
check at the service boundary.

The JSON-RPC command names, payload decoding and response shapes are unchanged.
Tests remain exempt from the production panic lints through the crate's
existing test-only allowances.

## Verification

The library tests exercise request decoding and language services. Strict
Clippy checks the production panic policy; the full release/push gate also
runs the existing stdio protocol suite. See [README](README.md#verification)
for the shared verification policy.
