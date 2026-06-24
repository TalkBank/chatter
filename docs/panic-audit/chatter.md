# Panic audit: chatter

**Status:** Reference
**Last updated:** 2026-06-13 21:07 EDT

See [README](README.md) for the shared policy. This page records the
crate-specific panic surface.

## Surface

Inline `#[allow(clippy::...)]` sites, dominated by command-routing
catch-alls plus a few guarded unwraps.

- **Command-routing `unreachable!`** (`commands/dispatch.rs`, five sites).
  `CommandRoutingService::dispatch` partitions the `Commands` enum by family
  and forwards each variant to exactly one `CommandFamilyService`. Each
  service's `match` then handles only its family's variants, and the
  `_ => unreachable!(...)` arm is reached only if the partitioning and a
  service disagree, an internal bug. The inline comment names the routing
  invariant. See [talkbank-lsp](talkbank-lsp.md) for the typed-sub-enum
  follow-up sketch that would remove the catch-all entirely (the same shape
  applies here).
- **Guarded unwraps / formatting** (`commands/json.rs`, `commands/clean.rs`,
  `commands/debug/linker.rs`, `commands/debug/overlap.rs`,
  `commands/clan/mod.rs`, `commands/validate_parallel/runtime.rs`,
  `commands/validate/audit_reporter.rs`, `src/main.rs`): each reads a value
  established by a prior check in the same scope. The `main.rs` site is the
  `from_arg_matches` expect, clap has already exited the process on any
  malformed argument, so the matches are well-formed by the time it runs.

Test code is exempt via `#![cfg_attr(test, allow(...))]` in `src/main.rs`.

## Verification

```bash
cargo clippy -p chatter --lib --bins --locked -- -D warnings
```
