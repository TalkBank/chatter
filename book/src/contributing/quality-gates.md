# Testing and Quality Gates

**Status:** Current
**Last modified:** 2026-08-27 14:04 EDT

How local verification relates to CI. The local commands themselves live in
[Developer Verification Checks](dev-checks.md), which is their single owner;
this page says which of them CI repeats and which it does not.

## Local pre-merge contract

`just gate` runs everything CI runs; `just push` runs it and then pushes. See
[dev-checks](dev-checks.md#before-pushing) for what it contains and why each
step catches something the others cannot.

The list is deliberately no longer reproduced here. When it was, it was a set
of commands a human assembled from memory, and the one that was easiest to
forget was `cargo test --doc --workspace`, which is exactly the one a green
`just test` gives no signal about.

## Never-regress gates

The CHAT core has five gates that must stay green for any change touching the
grammar, parser, model, validation, serialization or alignment: parser
equivalence, roundtrip idempotency (which carries reference-corpus coverage in
the same test), the generated spec tests, the validation error corpus, and the
gate registry. Each has a fast targeted command, listed with what it protects
under [Testing, Never-Regress Gates](testing.md#never-regress-gates).

Those commands take `--tests <filter>`, not `--test <name>`: each crate has one
integration binary, and the per-file target names the book used to give have
not existed for some time, so every one of them errored out.

**A red gate is a bug until proven otherwise, never a test expectation to
quietly update.** That rule has teeth in both directions: a diagnostic that
LOOKS better after a change earns the same scrutiny as one that looks worse.
A specific, plausible-looking error message was once defended as a loss when
the corruption producing it was fixed.

## What CI actually runs

`.github/workflows/ci.yml` is the authoritative shared signal, and it has more
jobs than this page used to admit:

| Job | Checks |
|---|---|
| `rust` | build, test, and the `spec/` workspace. NOT clippy: that is release-time |
| `wasm` | the re2c parser still compiles for `wasm32` |
| `book` | mdBook build plus a lychee link check |
| `rust-version-sync` | version pins in workflows, and doc date headers |
| `app-version-sync` | the desktop app version tracks the workspace version |
| `shellcheck` | every tracked shell script, default severity |
| `grammar` | the grammar's own checks |
| `dependency-audit` | dependency advisories |

Separate workflows cover release-time lint (`release-lint.yml`: clippy over
both workspaces plus the feature-off build, on a tag or on demand),
cross-platform builds (`cross-platform.yml`), rolling clippy drift
(`clippy-rolling.yml`), crates.io readiness, and the release and desktop
pipelines.

## What CI does NOT cover

Worth knowing, because these are the gaps where a local run is the only signal:

- **The vendored re2c lexer.** No workflow installs re2c, so nothing verifies
  that the committed lexer matches `lexer.re`. `just verify-vendored-lexer` is
  the only check, and it must be run by hand. `build.rs` used to claim a CI job
  did this; there has never been one.
- **The observation snapshot** (`spec/observations/example-diagnostics.json`)
  records, for every spec example, the codes each stage emitted and whether
  the parsed model serializes back byte-exact. It IS in CI, through its
  currency test, but the gap is human: a regenerated snapshot with a changed
  entry passes the test, so every diff in it must be adjudicated in the
  commit as intended or unintended rather than committed because `just regen`
  produced it.
- **A consumer's behaviour after regenerating a generated module.** A
  differential over generated TEXT is blind to a change in behaviour precisely
  when the text is expected to change; only running the consumer's own suite
  sees it.

## Legacy labels

References to numbered gates such as `G0-G14` come from the predecessor
workspace and name nothing here. There is no Makefile in this repository.
