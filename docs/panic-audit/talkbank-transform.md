# Panic audit: talkbank-transform

**Status:** Reference
**Last updated:** 2026-06-14 19:57 EDT

See [README](README.md) for the shared policy. This page records the
crate-specific panic surface.

## Surface

The crate's production code carries **no** `unwrap()` / `expect()` /
`panic!` sites. The deny-level lints (`unwrap_used`, `expect_used`,
`panic`, `unreachable`, `todo`, `unimplemented`; see `Cargo.toml`
`[lints.clippy]`) pass with **zero** inline `#[allow]` exceptions. The
only such calls live in test code (exempted crate-wide by the
`#![cfg_attr(test, allow(...))]` in `lib.rs`) and in rustdoc examples.

The crate previously hosted a large justified production panic surface
in the Batchalign ML modules: ASR post-processing, retokenization,
neural morphosyntax, forced-alignment decisions, and evaluation. Those
modules were extracted out of chatter, so the production panic surface
is now empty. A contributor who adds an `unwrap()` / `expect()` to
production code gets a hard clippy error, and there is no allowlist of
justified exceptions to maintain.

## Verification

```bash
cargo clippy -p talkbank-transform --lib --bins --locked -- -D warnings
```
