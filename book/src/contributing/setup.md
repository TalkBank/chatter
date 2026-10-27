# Setup

**Status:** Current
**Last modified:** {{git-dates:page}}

Getting a working checkout, and what you need installed for each surface you
might touch. What to RUN once you are set up is in
[Developer Verification Checks](dev-checks.md), which owns that list.

The project targets Windows, macOS and Linux. Commands below use a Unix-style
shell. The `justfile` explicitly uses Bash, including scripts and environment
assignments that cannot be pasted unchanged into PowerShell; Windows
contributors need a Bash environment for those recipes and the native build
tools for their selected target.

## Prerequisites

**Always:**

- **Rust** via [rustup](https://rustup.rs/). Do NOT install a version by hand:
  `rust-toolchain.toml` selects the current stable release and its
  components, and rustup honours that channel selection. Update an existing
  installation with `rustup update stable` before release-candidate verification;
  record `rustc --version --verbose` and `cargo --version` in its evidence.
  Chatter 1.0 supports current stable, with no older-compiler or fixed-MSRV
  promise. A new stable's clippy
  lints are caught by the weekly `clippy-rolling.yml` run and fixed in a
  focused commit.
- **[just](https://github.com/casey/just)** for shared verification and
  generation recipes. Focused Cargo commands are also useful; the `justfile`
  remains the owner of shared checks and their publication sequence.
- **Git and a native compiler/linker.** Rust compilation needs the platform
  toolchain; crates compiling the Tree-sitter C parser also need a C compiler.
  A Rust installation alone does not install every operating-system build tool.

**Per surface, only if you touch it:**

| You are changing | You also need |
|---|---|
| the grammar (`grammar/grammar.js`) | Node.js at `grammar/.nvmrc`, then `cd grammar && npm ci` for the locked Tree-sitter CLI; make that `grammar/node_modules/.bin` directory available to recipes that invoke `tree-sitter` |
| the grammar, so the typed traversal must be regenerated | Python 3.11 or later and a clean maintainer-provided `tree-sitter-grammar-utils` checkout at `tsgu-revision`; its source is not yet publicly available (see [Grammar Workflow](grammar-workflow.md)) |
| the re2c lexer (`crates/talkbank-parser-re2c/src/lexer.re`) | `re2c` at the exact version in `re2c-version.toml`, which provides the `re2rust` binary; `just verify-vendored-lexer` rejects drift |
| the book | Python 3.11+ and `just book-install-tools` (installs pinned mdBook, mdbook-mermaid and lychee into `.tooling/`) |
| the desktop app, or the whole root workspace that includes it | The desktop's native system dependencies and frontend tools; see [Desktop App](../chatter/user-guide/desktop-app.md#build-from-source). The [Linux CI action's package list](../../../.github/actions/tauri-linux-deps/packages.txt) owns the Linux dependency list. |
| final publication checks | Python 3.11+, Node.js/npm with the grammar CLI available, the pinned re2c tool, book tools, actionlint, shellcheck, cargo-insta and cargo-deny, plus the native dependencies above; inspect `just gate` and [Developer Verification Checks](dev-checks.md) before running it |

Ordinary core parsing and validation do not require a restricted TalkBank corpus
or an inference service. Fetching toolchains, Cargo/npm dependencies and book
tools does require network access unless those inputs are already cached.
Some checks need their own additional tools; a skipped prerequisite is not a
passing check. Grammar regeneration additionally has the generator-access
limitation described above.

## Clone and build

```bash
git clone https://github.com/TalkBank/chatter.git
cd chatter
cargo build -p talkbank-model --locked
```

This starts with the model package, not desktop or all other packages. Select
the package and focused behavior you intend to change. For example, to build
and run the library recipes shown in the book:

```bash
cargo test -p talkbank-transform --test integration book_library_usage_examples
```

Expect the six named examples to run, not a zero-test success. Their source is
included in [Library Usage](../chatter/integrating/library-usage.md). A successful
selection proves that selection only. After installing the whole workspace's
prerequisites, `just test` runs its compiled test targets; the final publication
gate has additional requirements. Durations depend on the machine, selected
features and available build cache.

## Two Cargo workspaces

The repository has two INDEPENDENT Cargo workspaces. This trips people up
because `--workspace` from the root does not reach the second one, so a spec
change can be broken while every root gate is green.

### 1. The root workspace (`Cargo.toml`)

Crates for parsing, model, validation, transform, CLI, LSP and desktop.
Plain `cargo` commands from the repo root operate here. An unqualified
`--workspace` includes the desktop member and its native dependencies; select
`-p <package>` when you mean a smaller surface.

### 2. The spec workspace (`spec/Cargo.toml`)

Two member crates, `spec/tools` and `spec/runtime-tools`. Reach it with the
WORKSPACE manifest, not an individual crate's:

```bash
cargo test --manifest-path spec/Cargo.toml --workspace   # or: just test-spec
```

`just test-spec` is the same thing, and `just gate` runs it. What
the two crates are for, and why the split exists, is in
[Spec Tooling](../architecture/spec-tooling.md).

## The recipes

```bash
just --list
```

That is the authoritative catalog and it is worth reading once end to end: it
covers testing, both generators, the spec gates, formatting, the book, doc
dates, the vendored lexer, coverage, and the release commands. This page
deliberately does not reproduce it: a copy would drift and tell contributors
that recipes such as `just test-spec`, `just spec-status`,
`just form-markers-gen`, `just symbols-gen`, `just verify-vendored-lexer` and
`just doc-dates` do not exist.

Which recipes to run, when, and what each costs: [Developer Verification
Checks](dev-checks.md).

## Pushing

Prepare and review the candidate, obtain the required maintainer authorization,
and run `just gate` on its final content. The pre-push hook checks the
content-bound receipt. A local gate verifies the local platform and its declared
checks; CI still supplies other platform and runner evidence.

`just push` is a convenience that runs the gate and then pushes. It is an
external action, not a command to run merely to check a checkout. See
[CI and Release](ci-and-release.md) for the authorized publication sequence.

A green `just test` is not a green gate: the gate also runs doctests, the
`spec/` workspace and static checks. Clippy and feature-off verification are
separate release checks, not part of the per-push gate. Use the owning recipe
rather than substituting a manually selected subset and calling it a gate.

There is no `make verify`; `just` owns shared verification recipes. Focused
Cargo commands remain appropriate for selected package/test feedback.

## Editor setup

rust-analyzer works out of the box on the root workspace. If you are editing
under `spec/`, point your editor at `spec/Cargo.toml` as a second linked
project, or it will report the spec crates as not belonging to any workspace.
