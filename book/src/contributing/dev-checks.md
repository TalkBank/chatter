# Developer Verification Checks

**Status:** Current
**Last modified:** 2026-08-27 14:10 EDT

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

## The git hooks, and what each refuses

Run `just install-hooks` once per clone. It points `core.hooksPath` at the
tracked `.githooks/` directory, because `.git/hooks` does not survive a clone
and an untracked hook is a gate that exists on exactly one machine.

| Hook | Refuses |
|---|---|
| `commit-msg` | a `type(scope)!:` subject that does not touch `CHANGELOG.md`; and production Rust staged with no test, spec, corpus or fixture beside it |
| `pre-push` | a push with no `just gate` stamp, or a stamp taken on different bytes |

Neither has a bypass flag, and `pre-push` runs no checks of its own: it reads
the stamp `just gate` writes, because git has already opened its connection to
the remote by the time a pre-push hook runs, so a multi-minute hook is closed
by the SSH idle timeout and fails a push that had passed.

**The red-evidence gate has one way past it, and it is not a flag.** If a change
genuinely admits neither a test nor a type, say so in a `Red:` trailer on its
own line in the message body, naming what was red:

```
Red: the compiler, at 14 call sites of Word::new
Red: nothing. A pure deletion; it removes the only caller of X.
```

That trailer is recorded in the history and names a claim a reader can check,
which a bypass variable is not. In this repo a **spec file counts as the
failing test**: a construct or parser bug is fixed by writing the spec first,
and `just regen` turns it into fixtures.

`just evidence-gate-test` and `just breaking-changelog-test` prove both gates
fire, in both directions; both run in `just gate`.

## Before pushing

```bash
just gate          # static checks plus every test CI runs; the pre-push gate
```

Or `just push`, which runs `gate` and then pushes.

`just release-lint` is separate and is NOT part of this: clippy over both
workspaces plus the feature-off build, run once before a release. Each is its
own cargo unit that recompiles the workspace, and none of them is a thing a
per-push gate needs to know.

`gate` puts every cheap check ahead of every expensive one, so a workflow typo
or a stale version pin fails in seconds rather than after the test suite.

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
| `cargo test --doc --workspace` | doctests, invisible to `--tests` |
| `just test-spec` | the `spec/` workspace, which `--workspace` does not reach |
| `just book` | the book builds and its links resolve |
| `just doc-dates` | a `Last modified` header older than the file |
| `just actionlint`, the two sync checks | workflow syntax and version pins |

Clippy is deliberately absent, and so is the feature-off build: both are
`just release-lint`, which per-push CI no longer runs either. Nothing in CI
goes red on something the local gate did not run; that equivalence is what
`scripts/check_ci_gate_sync.py` enforces.

`just test-all` is the TEST half of the gate (both workspaces, doctests, the
proc-macro UI suite) and is what `gate` delegates to. Useful on its own when you
want the tests without the lints, the grammar checks and the book.

**`just fmt-check` is not optional.** `cargo test` does not run rustfmt, CI
does, and formatting drift accumulated across 19 files once while every test run
stayed green.

## By surface

**Parser, model, alignment, serialization, roundtrip** (mandatory):

```bash
cargo test -p talkbank-parser-tests --tests reference_corpus_parses
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
