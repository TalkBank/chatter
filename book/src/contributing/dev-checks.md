# Developer Verification Checks

**Status:** Current
**Last modified:** 2026-08-13 01:05 EDT

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
just gate-fast     # twelve checks, under a minute. Run this constantly.
just gate-slow     # compilation, tests, lints. 10-13 minutes.
just gate          # both
```

Or `just push`, which runs `gate` and then pushes.

The split exists because the whole gate exceeds this project's 900-second
command ceiling, so an agent cannot invoke it in one call and would otherwise
be pushed toward running some-but-not-all by hand, which is the failure the
gate exists to end. It also puts every cheap check ahead of every expensive
one: a workflow typo or a stale version pin now fails in seconds instead of
after twelve minutes of rustdoc.

**Do not assemble this by hand from the list below.** It used to be a list,
`just push` ran no tests at all under a comment claiming it was the full CI
gate, and the predictable thing happened: a green `just test` was read as a
green gate and CI went red on a doctest. `just test` is `--tests`, and doctests
are a separate compilation it cannot see.

What `gate` runs, and why each is not covered by the others:

| Step | Catches what nothing else does |
|---|---|
| `just fmt-check` | `cargo test` does not run rustfmt; CI does |
| `just grammar-generate-check` | a stale `parser.c`. The traversal staleness guard hashes `grammar.json` and `node-types.json`, so a regeneration touching only `parser.c` passes it correctly; a tree-sitter version bump does exactly that |
| `just test` | the compiled test suite |
| `just check-feature-off` | the crate still builds with default features off |
| `cargo test --doc --workspace` | doctests, invisible to `--tests` |
| `just test-spec` | the `spec/` workspace, which `--workspace` does not reach |
| `just book` | the book builds and its links resolve |
| `just doc-dates` | a `Last modified` header older than the file |
| `just actionlint`, the two sync checks | workflow syntax and version pins |

Clippy is deliberately absent: CI owns it as a single pass. That is an accepted
way for CI to go red on something local did not run.

`just test-all` is the TEST half of the gate (both workspaces, doctests, the
proc-macro UI suite) and is what `gate` delegates to. Useful on its own when you
want the tests without the lints, the grammar checks and the book.

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
