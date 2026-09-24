# Reference Corpus

## Overview

The reference corpus at `corpus/reference/` is the finite, reusable input
suite for parsing, validation, roundtrip and transformation contracts.
The canonical backend is tree-sitter (`talkbank-parser`). The alternate
re2c backend is experimental and incomplete; it is not a specification
oracle, and passing the canonical suite does not establish backend equivalence.

Reference files supply representative valid constructs and combinations.
Canonical specifications in `spec/constructs/` and `spec/errors/` supply
independently justified expectations, legal boundary controls and deliberate
invalid variants. Fixture counts and passing tests alone do not prove complete
CHAT representativeness or 100% execution coverage.

## Provenance and licensing

Distinguish authored controls from examples derived from corpus material.
Retain the source relationship, transformations and applicable redistribution
permission for each derived example. A neutral `@ID` corpus field, renamed
file or explanatory comment does not establish authorship, anonymization or
licensing. Do not infer that every existing file was synthesized or that all
dependent tiers were produced by one pipeline.

New fixtures should explain their purpose and constructs in `@Comment:`
headers. Keep public examples minimal and sanitized; do not embed private
source paths or identifying provenance in this public tree. Resolve source
and licensing evidence before adding derived material. The repository's
MIT OR Apache-2.0 license is not evidence of permission for external input.

## Structure

`corpus/reference/` is organized into subdirectories by what each
group of files demonstrates:

- `core/`: document structure, headers, metadata
- `content/`: the main tier (words, terminators, linkers, pauses)
- `annotation/`: brackets, retraces, groups, scoping
- `tiers/`: dependent tiers (`%mor`, `%gra`, `%pho`, `%wor`, etc.)
- `ca/`: conversation analysis (overlaps, intonation)
- `audio/`: audio-linked files with `%wor` word-level timing
- `languages/`: one conversation per language, morphotagged with
  `%mor`/`%gra`
- `edge-cases/`: boundary and corner-case constructs
- `word-features/`: feature-focused word-level fixtures

Do not maintain a second static fixture count here. The test harness discovers
the current population. `just spec-status` reports specification claims and
CHECK adjudication counts, not execution or corpus-representativeness coverage.

## Validation

```bash
cargo test -p talkbank-parser-tests --test integration reference_corpus_parses::
cargo test -p talkbank-parser-tests --test integration roundtrip_reference_corpus::
just spec-status
```

## Key Policies

- Investigate failures against the specification and retained evidence;
  neither current parser output nor a reference file is automatically right.
- Preserve recovery diagnostics. Parsing a recovered AST is not proof of
  validity, and normalizing away a fault is not a successful roundtrip.
- Keep expectations separate from observations. A generated mutation is a
  candidate until its intended rule and expected behavior are reviewed.
- Prefer bounded examples that exercise related real-use combinations over
  repeated whole-production-corpus differential runs.
- Change generated specification fixtures through their owning specs and
  `just spec-gen`; never hand-edit the generated artifacts.

## See Also

- The repo-root AGENTS.md
- `crates/talkbank-parser-tests/`: the equivalence-test harness
- [Spec workflow](../book/src/contributing/spec-workflow.md)
- [Testing](../book/src/contributing/testing.md)

---
**Last modified:** 2026-09-24 00:21 EDT
