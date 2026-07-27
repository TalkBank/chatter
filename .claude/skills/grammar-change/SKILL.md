---
name: grammar-change
description: Change the CHAT tree-sitter grammar (grammar/grammar.js or any grammar source), including reverts. Use BEFORE editing grammar sources and to run the mandatory regeneration + verification sequence afterward.
allowed-tools: Bash, Read, Edit, Write, Grep, Glob
---

# Grammar Change (mandatory sequence)

`grammar/src/parser.c` is GENERATED; `tree-sitter test` regenerates
before testing and therefore does NOT detect a stale parser.c; only
cargo builds exhibit stale-parser bugs. So, after ANY grammar-source
edit (including reverts), run in order:

1. `cd grammar && tree-sitter generate`
2. `cd grammar && tree-sitter test`
3. Regenerate the typed CST traversal module
   (`crates/talkbank-parser/src/generated_traversal.rs`): the exact
   command shape is in `crates/talkbank-parser/src/lib.rs`'s doc
   comment (generate_typed_traversal from grammar.json +
   node-types.json, no --skip, pinned edition + toolchain). NEVER
   hand-edit the generated file; staleness guard:
   `generated_traversal_is_current`.
4. Regenerate corpus/error tests from specs (see `spec/CLAUDE.md`).
   Do not regenerate corpus expectations blindly; review failures
   first. Note: the spec generator can overwrite hand-derived corpus
   files (e.g. `grammar/test/corpus/word_markers/marker_density.txt`);
   restore such files via `git checkout` if clobbered.
5. `cargo test -p talkbank-parser -p talkbank-parser-tests`
   (equivalence + roundtrip gates are mandatory before commit).
6. One real-file CLI validation over the changed syntax path.

Design rules: strict + catch-all pattern for closed header-value
sets (`grammar/CLAUDE.md`); every grammar/parser bug fix ships with a
spec + reference-corpus entry (permanent regression gates); test
failures mean STOP AND ASK, never silently update expectations.
Emergency revert: revert grammar.js, then STILL run steps 1-5.

Full workflow with rationale:
`book/src/contributing/grammar-workflow.md`.
