# Rust Compilation Times

**Status:** Reference
**Last updated:** {{git-dates:page}}

How the workspace's dev and test profile settings keep compilation fast, and
what to avoid. The workspace root `Cargo.toml` comments are the source of truth
for the exact settings; re-run `cargo build --timings` for current numbers.

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

## Settings this workspace uses

- **`debug = "line-tables-only"`** in `[profile.dev]` and `[profile.test]`.
  Backtraces keep file and line information, while the bulky type and variable
  metadata (full DWARF, large `.dSYM` bundles and `.o` files that inflate
  linker input) is skipped. You cannot inspect local variables in a debugger
  (lldb/gdb); for most development workflows this is the right tradeoff.
- **`split-debuginfo = "off"`** in the same profiles. macOS defaults to
  `unpacked`, which leaves one `.rcgu.o` per codegen unit in
  `target/debug/deps` and makes warm test runs pay for scanning tens of
  thousands of directory entries.
- **`opt-level = 3` for build scripts and proc macros**
  (`[profile.dev.build-override]`).
- **No workspace-wide third-party optimization.** `[profile.dev.package."*"]`
  and `[profile.test.package."*"]` with `opt-level = 1` are not set: with the
  workspace's third-party dependency surface (axum, async-trait, tokio's full
  feature set, and so on) their build-time cost is prohibitive. Where runtime
  is the bottleneck for a specific test, opt in locally rather than setting it
  workspace-wide.

## Do not let a compiler wrapper disable incremental compilation

A global `~/.cargo/config.toml` that sets `rustc-wrapper` (for example to
sccache) disables Rust incremental compilation entirely, because the wrapper
interposes between Cargo and rustc and breaks the incremental artifact
protocol. sccache also gives near-zero benefit for this workspace: rlib
crates, which most workspace crates produce, cannot be cached by sccache. The
result is that every build after a one-line change is a full rebuild of the
dependency chain; a change to `talkbank-model`, near the root of the crate
graph, recompiles 11+ downstream crates.

If your global config sets a wrapper, override it for this project only with a
local `.cargo/config.toml`:

```toml
[build]
rustc-wrapper = ""
```

Other Rust projects on the system are unaffected, and sccache stays available
for CI and other projects. The file is gitignored (the repository's
`.gitignore` names `/.cargo/config.toml`) rather than committed because
the empty-string value trips a `cargo-llvm-cov` bug that treats `""` as a real
wrapper path instead of "no wrapper"; each contributor opts in locally, and CI
does not carry the override.

The `lld` linker (`linker = "lld"` in the global config, `ld64.lld` from
Homebrew's LLVM on macOS) is fine and slightly faster than Apple's default
linker for a workspace of this size.

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

3. **Optimize dependencies, not your own crates, where the dependency surface
   is small.** `[profile.dev.package."*"]` with `opt-level = 1` speeds test
   execution at little compile cost when dependencies rarely change, but its
   build-time cost grows with the dependency set. This workspace does not set
   it (or the `profile.test` equivalent); where runtime is the bottleneck for a
   specific test, opt in locally.

4. **Debug info has a real cost.** Full DWARF debug info inflates binary sizes
   and link times. Use `line-tables-only` unless you actively need a debugger.

5. **Measure before optimizing.** Use `cargo build --timings` to generate an
   HTML report showing per-crate compile times and parallelism. Use
   `sccache --show-stats` to verify cache effectiveness.

6. **Watch for crate graph bottlenecks.** Crates that sit at the root of the
   dependency graph (like `talkbank-model`) are the critical path, changes to
   them trigger the longest rebuild chains. Keep these crates lean and consider
   splitting them if they grow too large.
