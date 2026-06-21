# Rust Compilation Times: Findings and Optimizations

**Status:** Reference (historical analysis; current Cargo.toml profile knobs
are the source of truth)
**Last updated:** 2026-05-20 20:32 EDT

This document captures the compilation performance analysis that drove the
current dev/test profile knobs in the workspace root `Cargo.toml`. The
absolute measurements below were taken before the 2026-04-28 batchalign3
fold roughly tripled the third-party dependency surface; subsequent updates
are reflected in `Cargo.toml` comments, which are the source of truth.

## Background: How Rust Compilation Works

Rust compilation has two key mechanisms for speed:

1. **Incremental compilation**: When you change one file and rebuild, the compiler
   remembers which "codegen units" within each crate were affected and only
   recompiles those. This is the primary speedup mechanism for local iterative
   development (edit-compile-test cycles).

2. **Crate-level caching**: Cargo tracks which crates have changed inputs
   (source files, dependencies, feature flags). Unchanged crates are skipped
   entirely. This helps when you edit a leaf crate and don't need to rebuild
   unrelated crates.

Additionally, there are external tools:

3. **sccache**: A shared compilation cache that stores compiled artifacts by
   content hash. Designed for CI environments where builds start from a clean
   state. It works by wrapping `rustc` and checking a cache before invoking the
   real compiler.

4. **Linker choice**: The linker runs after all crates are compiled to produce
   the final binary. Faster linkers (like `lld`) can shave seconds off link time
   for large binaries.

## What We Found

### Problem 1: sccache Was Disabling Incremental Compilation (Critical)

The global `~/.cargo/config.toml` had:

```toml
[build]
rustc-wrapper = "/opt/homebrew/bin/sccache"
```

This caused two compounding problems:

- **sccache disables Rust incremental compilation entirely.** When a
  `rustc-wrapper` is set, Cargo cannot use incremental mode because the wrapper
  interposes between Cargo and rustc, breaking the incremental artifact protocol.

- **sccache had near-zero cache benefit for this workspace.** The sccache stats
  showed a 2.7% Rust cache hit rate. Out of 37 compilations, 36 were marked
  "non-cacheable" because rlib crates (library crates, which is what most
  workspace crates produce) cannot be cached by sccache.

The result: every `cargo build` after a one-line change was effectively a clean
rebuild of the entire dependency chain. A change to `talkbank-model` (near the
root of the crate graph) triggered a full recompile of 11+ downstream crates,
taking 60-90 seconds even for a trivial edit.

### Problem 2: Full Debug Info Was Inflating Link Times

The dev profile was generating full DWARF debug info (level 2), which includes:
- Type definitions for every struct/enum
- Variable location info for debugger inspection
- Full scope and lifetime metadata

This produces large `.dSYM` bundles and `.o` files, increasing linker input size
and slowing down the link phase.

### Problem 3: Third-Party Dependencies at -O0

All third-party crates (serde, regex, tree-sitter, etc.) were compiled at
`opt-level = 0` in dev builds. Since these crates rarely change, this was a
pure penalty: slow runtime (tests using serde deserialization, tree-sitter
parsing, or regex matching ran ~10x slower than necessary) with no compile-time
benefit after the first build.

### Non-Problem: lld Linker

The `linker = "lld"` setting in the global cargo config was fine. On macOS this
uses `ld64.lld` from Homebrew's LLVM toolchain (LLD 21.1.8), which is slightly
faster than Apple's default linker for workspaces of this size. No change needed.

## Changes Made

### Change 1: Project-Local sccache Override

Created `.cargo/config.toml` in the project root:

```toml
[build]
rustc-wrapper = ""
```

This overrides the global sccache setting for this project only, re-enabling
incremental compilation. Other Rust projects on the system are unaffected.

**Why not modify the global config?** Keeping the project-local override is
safer, sccache may still be useful for other projects or CI workflows.

**Note:** `.cargo/config.toml` is gitignored (not committed) because the
empty-string `rustc-wrapper = ""` value trips a `cargo-llvm-cov` bug that
treats `""` as a real wrapper path instead of "no wrapper." Each
contributor opts in locally; CI does not carry the override.

### Change 2: Reduced Debug Info

In the workspace `Cargo.toml`:

```toml
[profile.dev]
debug = "line-tables-only"

[profile.test]
debug = "line-tables-only"
```

This generates only file/line number information for backtraces, skipping the
bulky type and variable metadata. You still get useful panic/backtrace output
with source locations; you just can't inspect local variables in a debugger
(lldb/gdb). For most development workflows this is the right tradeoff.

### Change 3: Optimized Third-Party Dependencies, RETIRED post-fold

The original change set `[profile.dev.package."*"] opt-level = 1` to
optimize every third-party crate. After the 2026-04-28 batchalign3 fold
roughly tripled the third-party dependency surface (axum, async-trait,
tokio's full feature set, etc.), the build-time cost of this setting
became prohibitive, and the workspace `Cargo.toml` comment block now
explains why it was removed.

`[profile.test.package."*"] opt-level = 1` was also removed for the
same reason; for specific tests where runtime is the bottleneck, opt
in locally rather than reintroducing the workspace-wide setting.

## Results (pre-fold, 2026-03 measurement)

The numbers below were captured pre-fold against the original ten-crate
workspace. The fold roughly tripled the third-party dep set and forced
retiring `[profile.dev.package."*"] opt-level = 1`; today's wall-clock
will be slower and depends on which crate you touched. Re-run
`cargo build --timings` on the current workspace if you need fresh
numbers.

| Scenario | Before | After (pre-fold) |
|----------|--------|------------------|
| Clean build | ~3-5 min (est.) | ~39s |
| Incremental rebuild (touch `talkbank-model`) | ~60-90s | ~4s |
| Test runtime (serde/regex/tree-sitter hot paths) | Slow (-O0) | Faster (-O1, when opt-in) |

## Optional: Cranelift Backend for Maximum Iteration Speed

For the fastest possible "does it compile?" checks during rapid iteration,
Rust nightly supports the Cranelift codegen backend:

```bash
cargo +nightly -Z codegen-backend=cranelift build
```

Cranelift generates code ~2x faster than LLVM but produces unoptimized output
and is nightly-only. It is useful for compile-check cycles but not for
correctness testing or benchmarking.

## General Principles for Rust Compile Time

1. **Incremental compilation is king for local dev.** Anything that disables it
   (sccache, certain rustc-wrapper tools) is a net negative for iterative
   development.

2. **sccache is for CI, not local dev.** It shines when doing clean builds from
   scratch (CI runners, cross-compilation). For edit-rebuild cycles, incremental
   compilation is far more valuable.

3. **Optimize dependencies, not your own crates.** `[profile.dev.package."*"]`
   with `opt-level = 1` gives you faster test execution with minimal compile
   cost (dependencies rarely change).

4. **Debug info has a real cost.** Full DWARF debug info inflates binary sizes
   and link times. Use `line-tables-only` unless you actively need a debugger.

5. **Measure before optimizing.** Use `cargo build --timings` to generate an
   HTML report showing per-crate compile times and parallelism. Use
   `sccache --show-stats` to verify cache effectiveness.

6. **Watch for crate graph bottlenecks.** Crates that sit at the root of the
   dependency graph (like `talkbank-model`) are the critical path, changes to
   them trigger the longest rebuild chains. Keep these crates lean and consider
   splitting them if they grow too large.
