# Panic audit: talkbank-lsp

**Status:** Reference
**Last updated:** 2026-06-13 21:07 EDT

See [README](README.md) for the shared policy. This page records the
crate-specific panic surface. The non-test surface was audited 2026-04-29.

## Surface

Six inline `#[allow(clippy::unreachable)]` sites, all the same shape:
request/command routing catch-alls.

- `backend/language_services.rs`, `backend/chat_ops/mod.rs`,
  `backend/participants.rs`, `backend/requests/execute_command.rs`,
  `backend/analysis.rs`: an `ExecuteCommandRoutingService`-style dispatcher
  partitions incoming LSP commands by family and forwards each to a handler
  whose `match` covers only its family. The `_ => unreachable!(...)` arm is
  reached only if the partition and a handler disagree, an internal bug, so
  crashing loudly is correct rather than silently misrouting a request.

Backend initialization failures surface as diagnostics, not panics, and
request handlers degrade gracefully when parser services are unavailable
(per the LSP reliability rules in the crate `CLAUDE.md`).

Test code is exempt via `#![cfg_attr(test, allow(...))]` in `src/lib.rs`.

## Follow-up: typed sub-enums to remove the catch-alls

The `unreachable!` arms exist only because each handler matches against the
**flat** command enum, which contains variants from other families. The
principled removal is a typed partition:

1. Define a sub-enum per family (e.g. `ValidationCommand`, `ChatOpCommand`)
   containing only that family's variants.
2. Have the router convert the flat command into exactly one sub-enum once,
   via a fallible `TryFrom` (the single place that can reject an unknown
   command, returning a typed error, not a panic).
3. Each handler then matches its sub-enum **exhaustively**, with no `_`
   arm and no `unreachable!`.

This turns "this command never reaches this handler" from a runtime
assertion into a compile-time guarantee, and is the same refactor the CLI's
`commands/dispatch.rs` routing would take (it points here). It is recorded
as a follow-up, not yet implemented; the current catch-alls are correct and
covered by the routing invariant above.

## Verification

```bash
cargo clippy -p talkbank-lsp --lib --bins --locked -- -D warnings
```
