# Panic audit: chatter

**Status:** Reference
**Last updated:** [Git history](https://github.com/TalkBank/chatter/commits/main/docs/panic-audit/chatter.md)

See [README](README.md) for the shared policy. This page records the
crate-specific panic surface.

## Surface

Inline `#[allow(clippy::...)]` sites: a few guarded unwraps. Command routing
has none: `commands/dispatch.rs` is one exhaustive match over `Commands`, so
an unrouted command is a compile error rather than an `unreachable!` arm.

- **Guarded unwraps / formatting** (`commands/json.rs`, `commands/clean.rs`,
  `commands/debug/linker.rs`, `commands/debug/overlap.rs`,
  `commands/clan/mod.rs`, `commands/validate_parallel/runtime.rs`,
  `commands/validate/audit_reporter.rs`, `src/main.rs`): each reads a value
  established by a prior check in the same scope. The `main.rs` site is the
  `from_arg_matches` expect, clap has already exited the process on any
  malformed argument, so the matches are well-formed by the time it runs.

Test code is exempt via `#![cfg_attr(test, allow(...))]` in `src/main.rs`.

## Verification

`just clippy`, which covers this crate along with every other. See
[README](README.md#verification) for what it checks and why the per-crate
command that used to sit here could not fail.
