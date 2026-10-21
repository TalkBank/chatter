# Rust Compilation Times

**Status:** Reference
**Last updated:** {{git-dates:page}}

Use the workspace's existing profiles and focused development commands before
changing compiler settings. This chapter explains the checked-in configuration,
not a benchmark or a promise about a contributor's effective configuration.

## Checked-in profiles

The root `Cargo.toml` owns these settings:

| Profile | Explicit setting | Purpose |
| --- | --- | --- |
| `dev`, `test` | `debug = "line-tables-only"` | Retain source locations for backtraces without local-variable debug information |
| `dev`, `test` | `split-debuginfo = "off"` | Keep debug information in the linked artifact |
| `dev.build-override` | `opt-level = 3` | Optimize build scripts and proc macros |
| `release` | No explicit overrides | Use Cargo's release defaults |
| `dist` | Inherits `release`, `lto = "thin"` | Distribution-specific linking policy |

There is no workspace-wide `profile.dev.package."*"` or
`profile.test.package."*"` optimization override. The manifest records the
historical reason for the debug-information choice; it does not establish a
current directory-scan cost on every platform. Distribution and ordinary release
builds are not the same profile.

Cargo configuration and environment variables can override manifest profiles.
Use the [Cargo profile reference](https://doc.rust-lang.org/cargo/reference/profiles.html)
for inheritance and debug-information semantics. A debugger session requiring
local variables needs a deliberately selected full-debug configuration, not an
assumption that the default development artifact contains them.

## Distinguish three kinds of reuse

- Cargo can reuse an unchanged crate's existing artifacts.
- Rust incremental compilation reuses compiler work within a rebuilt crate.
- A compiler cache such as sccache can reuse eligible compiler invocations.

These mechanisms answer different questions. A wrapper setting is not itself
an incremental-compilation setting: Cargo exposes them separately. Inspect
profile/configuration overrides and `CARGO_INCREMENTAL`, as well as
`RUSTC_WRAPPER` and `RUSTC_WORKSPACE_WRAPPER`. Both wrappers can be active and
nested. See [Cargo's wrapper configuration](https://doc.rust-lang.org/cargo/reference/config.html#buildrustc-wrapper).

[sccache's Rust documentation](https://github.com/mozilla/sccache/blob/main/docs/Rust.md)
requires incremental compilation to be disabled for caching and lists other
eligibility constraints. Its linker restriction covers `bin`, `dylib`,
`cdylib` and `proc-macro` crates; it is not a blanket exclusion of `rlib`
compilation. A configured wrapper therefore does not prove a cache hit, and
an uncached invocation does not mean Cargo rebuilt every dependency.
Use `sccache --show-stats` when investigating an existing sccache setup.

Do not impose a global wrapper, linker or incremental policy from this guide.
The repository ignores a contributor's `.cargo/config.toml`; preserve any
existing target, linker and environment settings when making a local change.
Check the installed coverage tool's compatibility before applying a wrapper
override to an instrumented run. A historical wrapper problem is not evidence
that every current version has the same defect.

## Investigate one slow boundary

1. Name the affected package, command, profile, features and target. Separate
   compilation from linking, code generation, test execution and coverage export.
2. Reuse retained timings when their inputs match. Otherwise add `--timings` to
   the necessary Cargo build for that scope, for example
   `cargo build -p talkbank-model --timings`. Inspect the generated report in
   `target/cargo-timings/` (or the selected target directory).
3. Record whether artifacts were warm, what changed, and the compiler/tool
   versions and relevant overrides. Compare equivalent commands; a clean build
   and a warm no-op are different workloads.
4. Change one justified setting and retain both build and runtime consequences.
   Do not delete the target directory merely to investigate a warm-build delay,
   or run concurrent Cargo commands in the same workspace.
5. Restore experimental local overrides before obtaining release evidence.
   A faster build under different features, instrumentation, backend or profile
   does not qualify the intended release artifact.

This guide does not prescribe an alternate codegen backend or linker, or claim
a fixed speedup for one. Such experiments need their own supported toolchain,
target and correctness evidence. Use [Development Checks](dev-checks.md) for
the normal feedback commands and [CI and Release](ci-and-release.md) for the
publication checks; timing work does not replace either.
