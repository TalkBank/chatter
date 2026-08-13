# Developer Verification Checks

**Status:** Current
**Last modified:** 2026-08-12 21:00 EDT

What to run locally, and what each thing costs. The commands are `just`
recipes; `just --list` shows them all.

## The inner loop

```bash
just test          # cargo test --workspace --tests, about a minute
```

Narrower is better while iterating. Prefer the smallest thing that can fail:

```bash
cargo test -p <crate> --tests <name filter>
cd grammar && tree-sitter test        # grammar-only edits
```

**Do not run `cargo check` before `cargo test`.** `cargo test` type-checks
everything `check` would, and the two are DIFFERENT cargo units: `check` emits
only `.rmeta` while `test` emits full `.rlib` with codegen, so nothing is
reused and alternating them recompiles the whole dependency graph twice. This
page used to prescribe exactly that sequence. If a crate has no tests, run
`cargo test -p <crate>` anyway; it compiles and reports zero tests.

## Before pushing

```bash
just fmt-check     # BOTH workspaces; cargo test does not run rustfmt
just test          # compiled tests
just check-feature-off
cargo test --doc --workspace
just test-spec     # the spec/ workspace, which --workspace does not reach
just book          # builds the book and link-checks it
```

`just test-all` bundles the first four, but takes 10-15 minutes, almost all of
it rustdoc compiling one merged doctest binary per crate. It also exceeds this
project's 900-second command ceiling, so run the pieces separately.

**`just fmt-check` is not optional.** `cargo test` does not run rustfmt, CI
does, and formatting drift accumulated across 19 files once while every test run
stayed green.

## By surface

**Parser, model, alignment, serialization, roundtrip** (mandatory):

```bash
cargo test -p talkbank-parser-tests --tests parser_equivalence
cargo test -p talkbank-parser-tests --tests roundtrip_reference_corpus
cargo test -p talkbank-parser-tests --tests gates
```

**Grammar.** Follow the full [Grammar Workflow](grammar-workflow.md);
`tree-sitter test` does NOT detect a stale `parser.c`, so regeneration is
mandatory before any parser behaviour can be trusted.

**Specs, or either registry:**

```bash
just spec-status   # derived state: statuses, verified/deferred, parity counts
just test-spec     # the gates: example codes, manifest, registry drift
```

**The re2c lexer.** After changing `lexer.re` or the generated form-marker code
set it includes:

```bash
just verify-vendored-lexer
```

Nothing else checks it: no CI workflow installs re2c, so this is the only
check that exists, and it takes under a second.

**Docs:**

```bash
just doc-dates     # a `Last modified` header older than the file fails
```

## Regeneration

Run a generator only when its inputs changed, and never edit its output:

```bash
just symbols-gen         # spec/symbols/symbol_registry.json
just form-markers-gen    # spec/form_markers/form_marker_registry.json
```

The spec-driven generators (tree-sitter corpus, Rust tests, validation corpus)
are in [Spec Workflow](spec-workflow.md), with every command written out.

Regeneration is not a substitute for choosing the right regression test.

## Failure policy

A failing check blocks the change. If a failure is unrelated and pre-existing,
verify that by running against a clean checkout, say so, and fix it anyway
rather than routing around it: pre-existing defects linger precisely because
each person who meets them decides they belong to somebody else.
