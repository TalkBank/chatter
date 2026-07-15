# Summary

**Status:** Current
**Last modified:** 2026-06-15 15:00 EDT

[Introduction](introduction.md)
[Install](install/index.md)
[Quickstart](quickstart/index.md)
[Release Notes](release-notes.md)

---

# chatter, User Guide

- [Installation](chatter/user-guide/installation.md)
  - [Quick Start](chatter/user-guide/quick-start.md)
- [CLI Reference](chatter/user-guide/cli-reference.md)
- [Validation Errors](chatter/user-guide/validation-errors.md)
- [Chatter Desktop](chatter/user-guide/desktop-app.md)
- [CLAN Line Numbering](chatter/user-guide/clan-line-numbering.md)
- [Batch Workflows](chatter/user-guide/batch-workflows.md)
- [CI Integration](chatter/user-guide/ci-integration.md)
- [CHAT Processing Playbook (Editors & Analysts)](chatter/user-guide/chat-processing-playbook.md)
- [Sanitize (Protected Corpora)](chatter/user-guide/sanitize.md)
- [Speaker-ID (Label ASR Speakers)](chatter/user-guide/speaker-id.md)
- [Rediarize (Repair Speaker Attribution)](chatter/user-guide/rediarize.md)
- [Merge (Transcript Combination)](chatter/user-guide/merge.md)
- [Merge Workflow (pipeline, batch, adjudicate, sanity-scan)](chatter/user-guide/merge-workflow.md)

# CHAT Format

- [Overview](chat-format/overview.md)
- [Headers](chat-format/headers.md)
- [Utterances](chat-format/utterances.md)
- [Retraces and Repetitions](chat-format/retraces.md)
- [Replacements](chat-format/replacements.md)
- [Untranscribed Markers (xxx, yyy, www)](chat-format/untranscribed-markers.md)
- [Postcodes](chat-format/postcodes.md)
- [Dependent Tiers](chat-format/dependent-tiers.md)
  - [The %mor Tier](chat-format/mor-tier.md)
  - [Phon Tiers](chat-format/phon-tiers.md)
- [Word Syntax](chat-format/word-syntax.md)
- [Word Internals](chat-format/word-internals.md)
- [Symbols](chat-format/symbols.md)

# chatter, Architecture

- [Overview](architecture/overview.md)
- [Spec System](architecture/spec-system.md)
- [Grammar](architecture/grammar.md)
- [Overlap Marker Binding](architecture/overlap-binding.md)
- [Parsing](architecture/parsing.md)
- [CHAT Data Model](architecture/chat-model/chat-model.md)
- [Transform Pipeline](architecture/transform-pipeline.md)
- [Merge Pipeline, Domain Types](architecture/merge-domain-types.md)
- [Merge Pipeline, Test Plan](architecture/merge-test-plan.md)
- [Merge Pipeline, Crate Architecture](architecture/merge-architecture.md)
- [Merge Pipeline, Adjudication Workflow](architecture/adjudication-workflow.md)
- [XML Emitter](architecture/xml-emitter.md)
- [Errors, CHAT core](architecture/errors-and-validation/chat-core-errors.md)
- [Validation](architecture/errors-and-validation/validation.md)
- [CHECK Parity Audit](architecture/errors-and-validation/check-parity-audit.md)
- [Crate Reference](architecture/crate-reference.md)
- [CLI Startup and the Program Stack](architecture/cli-startup-and-stack.md)
- [Repo Architecture](architecture/repo-architecture.md)
- [Grammar Governance](architecture/grammar-governance.md)
- [Parser-Model Contracts](architecture/parser-model-contracts.md)
- [Parser Backends](architecture/parser-backends.md)
- [Leniency Policy](architecture/leniency-policy.md)
- [Error Diagnostics UX](architecture/errors-and-validation/error-diagnostics-ux.md)
- [Wide Struct Audit](architecture/chat-model/wide-structs.md)
- [Spec Tooling](architecture/spec-tooling.md)
- [Symbol Registry](architecture/symbol-registry.md)
- [Bullet Validation](architecture/bullet-validation.md)
- [CA Terminator Resolution](architecture/parser-and-grammar/ca-terminator-resolution.md)
- [Validation Cache](architecture/parser-and-grammar/validation-cache.md)
- [Alignment](architecture/alignment.md)
- [Memory and Ownership](architecture/memory-and-ownership.md)
- [Algorithms and Data Structures](architecture/algorithms.md)

# Contributing

- [Setup](contributing/setup.md)
- [Grammar Workflow](contributing/grammar-workflow.md)
- [Spec Workflow](contributing/spec-workflow.md)
- [Testing](contributing/testing.md)
- [Coding Standards](contributing/coding-standards.md)
- [Coding Standards (Extended)](contributing/coding-standards-extended.md)
- [CI and Release](contributing/ci-and-release.md)
- [Crates.io Publication](contributing/crates-io-publication.md)
- [Quality Gates](contributing/quality-gates.md)
- [Documentation Architecture](contributing/documentation-architecture.md)
- [CHAT Processing Playbook (Developers)](contributing/chat-processing-playbook.md)
- [Open-Source Governance](contributing/open-source-governance.md)
- [Compile Times](contributing/compile-times.md)
- [Dev Checks](contributing/dev-checks.md)
- [Branch Protection](contributing/branch-protection.md)
- [Reference Corpus](contributing/reference-corpus.md)
- [Desktop App Testing](contributing/desktop-testing.md)

# chatter, Integrating

- [Library Usage](chatter/integrating/library-usage.md)
- [JSON Output Reference](chatter/integrating/json-output.md)
- [JSON Schema](chatter/integrating/json-schema.md)
- [Diagnostic Contract](chatter/integrating/diagnostic-contract.md)
- [Merge Override File Format](chatter/integrating/merge-overrides.md)
