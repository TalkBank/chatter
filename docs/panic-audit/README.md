# Panic audit

**Status:** Reference
**Last updated:** 2026-06-13 21:07 EDT

The CHAT-core crates hold a **no-panics-in-production** discipline. This
directory records, per crate, how that discipline is enforced and which
deliberate exceptions exist. The per-crate pages are referenced from each
crate's `src/lib.rs` (or `src/main.rs`) header and its `[lints.clippy]`
table.

## The policy

Each audited crate sets the panic-shape clippy lints to `deny` in its own
`[lints.clippy]` table, rather than inheriting the workspace `warn` floor:

```toml
[lints.clippy]
unwrap_used   = "deny"
expect_used   = "deny"
panic         = "deny"
unreachable   = "deny"
todo          = "deny"
unimplemented = "deny"
```

Consequences:

- Production code **cannot** introduce a new `unwrap()`, `expect()`,
  `panic!()`, `unreachable!()`, `todo!()`, or `unimplemented!()` without an
  explicit, reviewed `#[allow(clippy::...)]` at the site.
- **Test code is exempt.** Each crate relaxes these lints under
  `#![cfg_attr(test, allow(...))]` in its `lib.rs`/`main.rs`: assertion
  macros panic by design and fixture `unwrap()` is the standard Rust testing
  idiom, so denying them in tests would be noise with no production analogue.
- Every remaining production `#[allow(clippy::...)]` carries an **inline
  justification comment** immediately above it, stating the invariant that
  makes the call infallible (or why the panic is the correct behavior). The
  inline comments are the authoritative per-site record; these pages are the
  policy and the index.

## Why a comment and not a `Result`

A panic is only acceptable when the failure is impossible by construction
(a checked invariant a few lines up, a compile-time-embedded constant, an
infallible `std::fmt::Write` target) or is a genuine internal-invariant
violation that should crash loudly rather than corrupt data. Anything that
can fail on real input is converted to a typed `Result`, not annotated.

## Regenerating the site list

The current production panic-allow sites for a crate:

```bash
rg -n --glob '!**/tests/**' --glob '!**/*_tests.rs' \
  'allow\(clippy::(unwrap_used|expect_used|panic|unreachable)\)' \
  crates/<crate>/src
```

To check that every site is justified (a comment sits immediately above each
allow), the audit script lives in the private workspace, not this public
repo.

## Per-crate pages

| Crate | Production panic sites | Notes |
|-------|------------------------|-------|
| [talkbank-parser](talkbank-parser.md) | none | `deny` with zero exceptions |
| [talkbank-parser-re2c](talkbank-parser-re2c.md) | 1 module-level + 4 inline | generated-lexer panic + retrace/internal-invariant |
| [talkbank-model](talkbank-model.md) | 2 inline | index unwrap after `is_none` short-circuit |
| [talkbank-transform](talkbank-transform.md) | inline (largest surface) | guarded slices, embedded-JSON statics, infallible writes |
| [chatter](chatter.md) | inline | command-routing `unreachable!` catch-alls |
| [talkbank-lsp](talkbank-lsp.md) | 6 inline | request-routing `unreachable!` catch-alls |

Counts drift as code moves; treat the source (the inline annotations) as
authoritative and regenerate with the command above.

## Verification

For any audited crate, this is green (every panic site is absent or carries
a reviewed allow; a new unguarded panic fails the build):

```bash
cargo clippy -p <crate> --lib --bins --locked -- -D warnings
```
