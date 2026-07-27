---
name: regen-spec-artifacts
description: Regenerate generated tests and docs after touching spec/constructs/ or spec/errors/ (adding a construct, an error-code spec, or editing spec examples). Use after ANY spec change.
allowed-tools: Bash, Read, Edit, Write, Grep, Glob
---

# Spec Artifact Regeneration

Specs are the source of truth; tests and error docs are GENERATED
from them (never hand-written). Error-code tests flow ONLY through
`spec/errors/E###_*.md`.

After any spec change, from the repo root (spec tooling is a
SEPARATE workspace; use its manifest path):

1. Run the four generators per `spec/CLAUDE.md` (gen_tree_sitter_tests,
   gen_rust_tests, gen_validation_corpus, gen_error_docs), with
   `--template-dir spec/tools/templates`.
2. Beware: gen_rust_tests with a defaulted output path can create a
   stray root-level generated tree; check `git status` for
   unexpected new directories before committing.
3. `cargo test -p talkbank-parser -p talkbank-parser-tests`
   (equivalence + roundtrip reference corpus are the mandatory
   gates).
4. If the grammar also changed, the grammar-change skill's sequence
   runs FIRST (generate before testing anything).

Spec-file formats, templates, and the layer-migration checklist
(parser-layer vs validation-layer specs; auditing E316 catch-alls):
`spec/CLAUDE.md` and `book/src/contributing/spec-workflow.md`.
Reference corpus files must be valid CHAT (roundtrip gate); invalid
examples belong in `spec/errors/`.
