# Panic audit: talkbank-model

**Status:** Reference
**Last updated:** 2026-06-13 21:07 EDT

See [README](README.md) for the shared policy. This page records the
crate-specific panic surface.

## Surface

Two inline `#[allow(clippy::unwrap_used)]` sites, both in
`src/validation/cross_utterance/mod.rs`, both the same pattern.

- The orphaned-top and the companion loop each `continue` early when
  `orphan.region.index.is_none()`. Reaching the line below that guard
  therefore proves the `Option` is `Some`, so
  `orphan.region.index.unwrap()` cannot fail. The inline comment names the
  control-flow invariant ("`is_none()` short-circuits the `continue` above;
  reaching this line guarantees `Some(...)`").

Everything else in the data model, validation, and alignment layers returns
typed errors (`thiserror` domain types) or reports through the validation
sink rather than panicking.

Test code is exempt via `#![cfg_attr(test, allow(...))]` in `src/lib.rs`.

## Verification

```bash
cargo clippy -p talkbank-model --lib --bins --locked -- -D warnings
```
