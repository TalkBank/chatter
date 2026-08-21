# Setup

**Status:** Current
**Last modified:** 2026-08-21 13:42 EDT

Getting a working checkout, and what you need installed for each surface you
might touch. What to RUN once you are set up is in
[Developer Verification Checks](dev-checks.md), which owns that list.

Development is supported on **Windows, macOS, and Linux**. The commands below
use Unix shell syntax; on Windows use PowerShell or Git Bash.

## Prerequisites

**Always:**

- **Rust** via [rustup](https://rustup.rs/). Do NOT install a version by hand:
  `rust-toolchain.toml` pins the exact stable release and
  rustup honours it automatically. The pin exists so a new stable's clippy
  lints cannot turn every open PR red overnight.
- **[just](https://github.com/casey/just)** for the repo's recipes. Not
  strictly required, but every command in the contributing docs is a `just`
  recipe, and the recipes are the single owner of how each check is invoked.

**Per surface, only if you touch it:**

| You are changing | You also need |
|---|---|
| the grammar (`grammar/grammar.js`) | Node.js, and the tree-sitter CLI (`cargo install tree-sitter-cli`) |
| the grammar, so the typed traversal must be regenerated | a local checkout of `tree-sitter-grammar-utils`, which is not yet published (see [Grammar Workflow](grammar-workflow.md)) |
| the re2c lexer (`crates/talkbank-parser-re2c/src/lexer.re`) | `re2c`, which provides the `re2rust` binary |
| the book | `just book-install-tools` (installs mdBook and lychee into `.tooling/`) |

Nothing here needs a TalkBank corpus or any network service. The CHAT core
builds and its tests pass on a fresh machine with only the "always" row.

## Clone and build

```bash
git clone https://github.com/TalkBank/chatter.git
cd chatter
cargo build --workspace --locked
```

Then run the tests to confirm the checkout is sound:

```bash
just test          # cargo test --workspace --tests, about a minute
```

## Two Cargo workspaces

The repository has two INDEPENDENT Cargo workspaces. This trips people up
because `--workspace` from the root does not reach the second one, so a spec
change can be broken while every root gate is green.

### 1. The root workspace (`Cargo.toml`)

Every crate for parsing, model, validation, transform, CLI, LSP and desktop.
Plain `cargo` commands from the repo root operate here.

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
deliberately does not reproduce it. It used to list eight recipes, and by the
time anyone noticed there were thirty-one, so the copy was quietly telling
contributors that `just test-spec`, `just spec-status`, `just form-markers-gen`,
`just symbols-gen`, `just verify-vendored-lexer` and `just doc-dates` did not
exist.

Which recipes to run, when, and what each costs: [Developer Verification
Checks](dev-checks.md).

## Pushing

```bash
just push          # runs `just gate`, then pushes
```

`just gate` is the pre-push gate: everything CI runs that can run on one
machine, in one command. It takes 12-15 minutes. CI is a confirmation, never
the thing that finds your bug for you.

It used to be a list of commands on another page, and `just push` ran four fast
checks under a comment claiming to be the full CI gate. A green `just test` was
read as a green gate and CI went red. If you find yourself assembling the gate
by hand from a list, that list is the bug.

There is no `make verify` and no Makefile. This page used to describe one as
"not yet ported"; it was never coming, because the recipes replaced it.

## Editor setup

rust-analyzer works out of the box on the root workspace. If you are editing
under `spec/`, point your editor at `spec/Cargo.toml` as a second linked
project, or it will report the spec crates as not belonging to any workspace.
