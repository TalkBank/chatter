# Merge Pipeline, Test Plan

**Status:** Current
**Last modified:** {{git-dates:page}}

This is a map of current test ownership and obligations, not a completion
ledger. Named tests, old implementation-cycle tables and proposed fixtures are
not execution receipts. The removed `merge`, `pipeline` and `batch` CLI
commands are not targets to restore.

See [Merge Architecture](merge-architecture.md) and
[Domain Types](merge-domain-types.md) for the contracts being checked.

## Current owners

Paths below are relative to the repository root.

| Boundary | Test owner |
| --- | --- |
| Structural assembly, ordering, headers, origins and fates | `crates/talkbank-transform/tests/integration/transcript_merge_tests.rs` |
| Speaker mapping and lexical identification | `crates/talkbank-transform/tests/integration/speaker_id_tests.rs` |
| Scripted adjudication | `crates/talkbank-transform/tests/integration/adjudication_tests.rs` |
| Confidence and override representation | Unit modules beside `speaker_id/types.rs` and `speaker_id/override_file.rs` |
| Canonical CHAT transform contracts | `crates/talkbank-parser-tests/tests/integration/transform_corpus.rs` and its declared modules |
| CLI speaker/adjudication/sanity behavior | `crates/chatter/tests/integration/` |
| Removed-command help surface | `crates/chatter/tests/integration/merge_experimental_help_tests.rs` |

The canonical transform harness includes `merge_corpus.rs`,
`gem_merge_corpus.rs`, `speaker_identification_corpus.rs` and
`adjudication_corpus.rs`. Follow the module declarations for the executable
selection; the existence of a source file alone does not prove it is registered.

## Structural merge obligations

Review the assembled model and provenance together:

- Preserve source-relative order, including header and section structure.
- Refuse indeterminate cross-source placement unless the explicit flagged
  draft policy was requested; retain its review records.
- Preserve untimed speech without fabricating timestamps. Equal recorded
  starts and absent starts have different meanings.
- Check output origins and every source fate, not just output length.
- Exercise tier stripping as caller policy, including custom and empty lists.
- Exercise language subset admission and conflicting participant metadata.
- Reject invalid assembled output before exposing a reportable result.
- Keep terminal-bullet edit provenance through validation and reporting.

The integration suite contains both success and refusal cases for source-order
placement, source anchor reversal, metadata ordering, headers and source fates.
A byte-stability assertion for a selected fixture is not a universal promise
that model serialization reproduces all original source bytes.

## Speaker and wire boundaries

Tests should distinguish lexical no-information, finite and unbounded margins;
keep absolute support with the decision. Input admission failure is not a
zero-score match. Mapping parsing, mapping application and output validity
are different contracts.

The current implementation passes unmapped speakers through. A test for
mandatory complete mapping would be a proposed policy change, not a missing
assertion to add without adjudication.

Override tests must cover per-speaker rename roles and the current schema
rather than the old single-role design. Deterministic local tests of sampling,
context preparation and report serialization are not provider executions.
Authored context records must be labeled as such; they are not corpus facts.

## Choosing evidence

Use spec/reference CHAT for canonical syntax and semantic claims.
Boundary and wire tests remain necessary where malformed inputs, external
records or deliberately corrupted state cannot be represented by a valid
reference transcript. Keep those populations distinguishable.

For a change, select the affected contract deck, inspect the actual assertions,
and retain the invocation, candidate identity and complete outcome. Batch
related repairs before verification. Do not infer acceptance from a proposed
test name, an ignored test, an unregistered module or a historical cycle count.

Documentation-only changes need proportional documentation checks; they do not
earn new functional coverage. Follow the current repository workflow rather
than the retired one-test-at-a-time implementation schedule.

## What these tests cannot establish

Structural correctness and a finite reference population do not establish
recording identity, source authority, acoustic correctness, human adjudication,
provider quality, corpus readiness, installed-runtime behavior or publication.
Each requires its own evidence and authority. No broad corpus run or external
provider call is implied by this test map.
