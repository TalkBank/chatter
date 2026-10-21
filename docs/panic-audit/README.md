# Panic audit

**Status:** Reference
**Last updated:** [Git history](https://github.com/TalkBank/chatter/commits/main/docs/panic-audit/README.md)

The CHAT-core crates hold a **no-panics-in-production** discipline. This
directory records, per crate, how that discipline is enforced and which
deliberate exceptions exist. The per-crate pages are referenced from each
crate's `src/lib.rs` (or `src/main.rs`) header and its `[lints.clippy]`
table.

## The policy

The root Cargo manifest denies the panic-shape Clippy lints. Each of the six
crates below opts in with `[lints] workspace = true`:

```toml
[workspace.lints.clippy]
unwrap_used   = "deny"
expect_used   = "deny"
panic         = "deny"
unreachable   = "deny"
todo          = "deny"
unimplemented = "deny"
```

Consequences:

- Clippy rejects a new `unwrap()`, `expect()`,
  `panic!()`, `unreachable!()`, `todo!()`, or `unimplemented!()` without an
  applicable allowance. Ordinary Cargo compilation does not run Clippy.
  Existing module-wide allowances cover more than their motivating call;
  they must be reviewed as permissions, not per-site proofs.
- **Test code is exempt.** Each crate relaxes these lints under
  `#![cfg_attr(test, allow(...))]` in its `lib.rs`/`main.rs`: assertion
  macros panic by design and fixture `unwrap()` is the standard Rust testing
  idiom, so denying them in tests would be noise with no production analogue.
- Each production allowance needs a reviewed justification stating the
  invariant and its evidence. Existing comments are rationale, not proof that
  every allowed call is infallible. These pages are an index, not a completed
  audit of all compiled configurations.

## Why a comment and not a `Result`

A panic is only acceptable when the failure is impossible by construction
(a checked invariant a few lines up, a compile-time-embedded constant, an
infallible `std::fmt::Write` target) or is a genuine internal-invariant
violation that should crash loudly rather than corrupt data. Anything that
can fail on real input is converted to a typed `Result`, not annotated.

## Regenerating the site list

Locate lint declarations and allowances for a crate:

```bash
rg -n 'clippy::(unwrap_used|expect_used|panic|unreachable|todo|unimplemented)' \
  crates/<crate>/src
```

Classify each match in its enclosing configuration; this includes multiline
attributes and test modules inside production files. Search results alone do
not identify runtime sites. Inspect build scripts, generated includes and
dependencies separately. No unshipped audit script is required by this guide.

## Per-crate pages

| Crate | Selected review boundary | Notes |
|-------|------------------------|-------|
| [talkbank-parser](talkbank-parser.md) | CST/source admission | distinguish test attributes from production |
| [talkbank-parser-re2c](talkbank-parser-re2c.md) | lexer and conversion allowances | generated and handwritten boundaries |
| [talkbank-model](talkbank-model.md) | overlap index admission | removed unwrap pattern |
| [talkbank-transform](talkbank-transform.md) | embedded JSON initialization | runtime parsing of embedded bytes |
| [chatter](chatter.md) | decoding and serialization | source-bound invariants |
| [talkbank-lsp](talkbank-lsp.md) | initialization and command routing | typed failures and request families |

These are selected boundaries, not exhaustive counts or panic-freedom proofs.

## Verification

```bash
just clippy
```

This selects the root workspace's all-target lint pass. The separate spec
workspace uses `just clippy-spec`; `just release-lint` owns the consolidated
release boundary. Follow [Developer Verification Checks](../../book/src/contributing/dev-checks.md),
not a per-edit lint ritual. To select a crate when scoped linting is needed:

```bash
cargo clippy -p <crate> --all-targets --locked
```

Record source, features, target and results. Restriction lints cover particular
code shapes, not all indexing, arithmetic, allocation, dependency or generated
failures. A passing lint command does not establish panic freedom.

**The command this section used to give could not fail, and that is worth
stating rather than quietly replacing.** It was:

```bash
cargo clippy -p <crate> --lib --bins --locked -- -D warnings
```

`unwrap_used`, `expect_used` and `panic` are clippy RESTRICTION lints, which
are allow-by-default. `-D warnings` promotes warnings to errors and a lint
that never warns produces none, so the command was green whether or not the
crate had the lints enabled. Six of the twelve crates, including the entire
CHAT core, never opted into the workspace table until 2026-08-14, and this
page reported them clean throughout. A verification that cannot distinguish
"no violations" from "not checked" is not a verification.
