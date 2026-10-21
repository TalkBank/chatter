# Developer Verification Checks

**Status:** Current
**Last modified:** {{git-dates:page}}

What to run locally, and what each thing costs. The commands are `just`
recipes; `just --list` shows them all.

## The inner loop

```bash
just test          # workspace test binaries, complete failure inventory
```

Narrower is better while iterating. Prefer the smallest thing that can fail:

```bash
cargo test -p <crate> --tests <name filter>
cd grammar && tree-sitter test        # grammar-only edits
```

If the needed result is a test run, do not reflexively precede it with
`cargo check`: check-only metadata and test code generation are different build
units, so that can duplicate work. Select the required package, features and
target once. A check-only build is useful when compilation itself is the missing
fact; it does not establish runtime behavior. A zero-test result likewise proves
no behavioral contract. Confirm the expected test names actually ran.

## Deterministic tests and native-platform feedback

Express admission and lifecycle invariants through typestate, ownership and
validated constructors first. Use pure tests for structural and policy logic,
and deterministic doubles for clocks, services and scheduling. Keep spec and
reference-corpus contracts at their real public boundaries. A double cannot
establish a database driver's worker shutdown or an operating system's file
locking behavior; retain thin, isolated native-platform tests for those facts.

Use explicit barriers and acknowledgments rather than sleeps or scheduler luck.
A deadline bounds execution; it is not the synchronization condition. Consume
resource capabilities at lifecycle transitions, and await actual cleanup where
the external API requires it. A passing retry does not resolve an observed
failure.

The shared workspace recipe and native CI use `--no-fail-fast`: all test
binaries run, failures still fail the command, and the result is one repair
inventory rather than successive first-failure discoveries. This flag does not
retry tests or weaken assertions. CI and the gate share `just test-workspace`,
which requires prerequisites rather than permitting inner-loop skips.
A local gate verifies its own platform, not
every supported operating system; use authorized incremental push/CI
checkpoints before freezing a release candidate instead of treating release
publication as the first cross-platform test.

## The git hooks

Run `just install-hooks` once per clone. It points `core.hooksPath` at the
tracked `.githooks/` directory, because `.git/hooks` does not survive a clone
and an untracked hook is a gate that exists on exactly one machine.

| Hook | Behavior |
|---|---|
| `pre-commit` | only chains to an optional contributor-owned local hook |
| `commit-msg` | only chains to an optional contributor-owned local hook, forwarding the message path |
| `pre-push` | a push with no `just gate` stamp, or a stamp taken on different bytes |

There are no built-in date, evidence or changelog commit gates. Privacy,
publication metadata, production-change evidence and breaking notes receive
explicit review. Contributor-owned local hooks remain opt-in. The publication
hook is unchanged: `pre-push` runs no checks of its own and reads
the stamp `just gate` writes, because git has already opened its connection to
the remote by the time a pre-push hook runs, so a multi-minute hook is closed
by the SSH idle timeout and fails a push that had passed.

`just doc-dates` admits each document's publication metadata. Git-derived
headers follow the document's own history automatically; see
[documentation architecture](documentation-architecture.md). Remaining
handwritten dates are checked against actual history and, for pending changes
since the configured upstream, today's date. An explicit staged review can
select `--prospective --snapshot index`, so an unstaged correction cannot
conceal a stale staged header. This is not run automatically on commit.
Detached CI checks actual committed history. Neither Git publication metadata
nor a handwritten date certifies a content review.

**Evidence belongs in the review and history.** If a change
genuinely admits neither a test nor a type, say so in a `Red:` trailer on its
own line in the message body, naming what was red:

```
Red: the compiler, at 14 call sites of Word::new
Red: nothing. A pure deletion; it removes the only caller of X.
```

That trailer is recorded in the history and names a claim a reader can check,
without a bypass variable or automatic commit gate. In this repo a **spec file
counts as the failing test**: a construct or parser bug is fixed by writing the spec first,
and `just regen` turns it into fixtures.

`just evidence-gate-test` and `just breaking-changelog-test` exercise the
retained standalone review-check scripts in both directions; both remain in
`just gate`. Their presence does not reinstall automatic commit hooks.

## Before pushing

```bash
just gate          # static checks plus every test CI runs; the pre-push gate
```

After maintainer authorization for the reviewed candidate, `just push` runs
`gate` and then pushes. Do not use it as a local-only verification command.

`just release-lint` is separate and is NOT part of this: clippy over both
workspaces plus the feature-off build, run once before a release. Each is its
own cargo unit that recompiles the workspace, and none of them is a thing a
per-push gate needs to know.

`gate` starts with locked metadata and several static checks before its later
build/test stages. It also includes checks that compile or invoke generators;
do not assume every prerequisite failure is cheap. The recipe owns ordering.

**Do not assemble this by hand from the list below.** A green `just test` is
not a green gate: `just test` is `--tests`, and doctests are a separate
compilation it cannot see.

Examples of distinct checks in `gate` (not an exhaustive or executable checklist;
the current `justfile` owns membership):

| Step | Catches what nothing else does |
|---|---|
| `just fmt-check` | `cargo test` does not run rustfmt; CI does |
| `just grammar-generate-check` | a stale `parser.c`. The traversal staleness guard hashes `grammar.json` and `node-types.json`, so a regeneration touching only `parser.c` passes it correctly; a tree-sitter version bump does exactly that |
| `just test-workspace` | the compiled test suite with strict prerequisite admission |
| `cargo test --doc --workspace` | doctests, invisible to `--tests` |
| `just test-spec` | the `spec/` workspace, which `--workspace` does not reach |
| `just book` | the book renders, publication metadata is present, and offline/internal links resolve; external links are excluded |
| `just doc-dates` | publication metadata follows the repository's Git-derived/header policy |
| `just actionlint`, the two sync checks | workflow syntax and version pins |

Clippy is deliberately absent, and so is the feature-off build: both are
`just release-lint`, which per-push CI does not run either.
`scripts/check_ci_gate_sync.py` checks declared recipe coverage and selected
workflow provisioning conventions. It cannot prove a different operating
system's behavior, service availability or arbitrary CI configuration correct.

`just test-all` is the TEST half of the gate (both workspaces, doctests, the
proc-macro UI suite) and is what `gate` delegates to. Useful on its own when you
want the tests without the lints, the grammar checks and the book.

**`just fmt-check` is not optional.** `cargo test` does not run rustfmt, CI
does, and formatting drift accumulated across 19 files once while every test run
stayed green.

## By surface

**Parser, model, alignment, serialization, roundtrip** (select the affected
contracts; retain the final gate separately):

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
set it includes, install the exact `re2rust` version named in
`re2c-version.toml`, then run:

```bash
just verify-vendored-lexer
```

The recipe fails before regeneration if the installed generator version differs
from that source of truth, and then compares the generated bytes. Nothing else
checks it: no CI workflow installs re2c, so this is the only check that exists,
and it takes under a second.

**Docs:**

```bash
just book          # render, metadata checks and offline/internal links
```

Use `just doc-dates` for the broader publication-metadata policy when needed.
Review examples against their source owners; a passing link check does not
establish that a command works or a behavioral claim is true.

## Dependency updates

Review Rust and JavaScript desktop dependency changes together with both
lockfiles. For schema-validation or compiler-helper updates, exercise the
reference corpus and JSON contracts:

```bash
cargo test --locked -p talkbank-parser-tests --test integration -- transform_corpus::json_contracts reference_corpus_parses
```

For desktop dependency updates, run `npm ci`, `npm run test:unit`, and
`npm run build` from `apps/chatter-desktop`, plus the native bridge tests
from the workspace root:

```bash
cargo test --locked -p chatter-desktop --test validation_bridge
```

These are focused compatibility checks, not release acceptance: they do not
certify signed installers, updater delivery, or native behavior on every target
platform. A manifest-only update does not require grammar regeneration.

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

For CLI subprocess failures, retain the full exit status, stdout and stderr.
Check successful completion before interpreting cache counts or other output:
an empty stream alone cannot distinguish a product failure from a terminated
process. Reproduce the exact failing test before broadening the run.

On Unix, tests that vary the program name should use `CommandExt::arg0` on
the original executable. This avoids giving the shared test executable a
second filesystem name during concurrent launches. Windows uses a temporary
same-filesystem hard link because its command API has no `arg0` override.


A relevant failing check blocks acceptance of the claim it tests. Distinguish
implementation defects, missing prerequisites and unrelated existing failures
using retained receipts and scoped source/history inspection. Preserve unrelated
work; do not reset, stash or create another checkout merely to obtain a green
run. Fix defects within the authorized scope, or state the unresolved cause and
required decision. Never turn a failed or skipped check into a passing receipt.
