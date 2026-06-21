# Panic audit: talkbank-parser

**Status:** Reference
**Last updated:** 2026-06-13 21:07 EDT

See [README](README.md) for the shared policy. This page records the
crate-specific panic surface.

## Surface

**None.** `talkbank-parser` has zero production `#[allow(clippy::...)]`
panic sites. The crate sets the panic-shape lints to `deny` and meets them
with no exceptions: every fallible path (CST traversal, node dispatch,
span resolution) returns a typed error or reports through the `ErrorSink`
rather than unwrapping. Unrecognized CST nodes go through
`unexpected_node_error()`, not `unreachable!()`.

Test code is exempt via `#![cfg_attr(test, allow(...))]` in `src/lib.rs`.

## Verification

```bash
cargo clippy -p talkbank-parser --lib --bins --locked -- -D warnings
```

Green with no inline allows. A new `unwrap()`/`panic!()` anywhere in the
production tree fails the build, which is the intended ratchet: this crate
stays panic-free.
