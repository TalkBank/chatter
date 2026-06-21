# Testing and Quality Gates

**Status:** Current
**Last modified:** 2026-06-15 15:00 EDT

This page summarizes the **current** relationship between local verification and
the repository CI workflows.

## Local pre-merge contract

There is no repo-local `make verify` wrapper in this checkout today. The local
contract is the command set documented in [Setup](setup.md) and
[Developer Verification Checks](dev-checks.md):

```bash
cargo fmt --all -- --check
cargo build --workspace --all-targets --locked
cargo nextest run --workspace
cargo test --doc
```

plus grammar/spec/parser-specific checks when you touch those surfaces.

## Never-regress gates

Beyond the formatting/build/test sweep above, the CHAT core has four
**never-regress gates** that must stay green for any change touching the
grammar, parser, model, validation, serialization, or alignment: parser
equivalence, roundtrip idempotency, reference-corpus 100%, and the
error-code spec tests. Each has a fast, targeted command. They are defined,
with the exact command and what each protects, under
[Testing, Never-Regress Gates](testing.md#never-regress-gates).
A red gate is a bug until proven otherwise, never a test expectation to
quietly update.

## Root CI contract

The main CI workflow (`.github/workflows/ci.yml`) is the authoritative shared
signal for this staging repo. Today it covers:

- Rust build, test, and clippy
- mdBook build

Additional workflows cover cross-platform build coverage and rolling-clippy
drift checks.

Because the old local wrapper pipeline has not been ported into this repo,
historical references to numbered gates such as `G0-G14` should be treated as
legacy labels from the predecessor workspace, not as the current command
surface here.

## Additional CI-only checks

These are required CI signals or workflow checks that are not identical to the
local command set:

- cross-platform release/build coverage
- weekly rolling-clippy drift checks
- workflow-specific smoke tests attached to release automation
