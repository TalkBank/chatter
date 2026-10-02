# Reference Corpus

**Status:** Current
**Last modified:** {{git-dates:page}}

The reference corpus (`corpus/reference/`) is the finite, reusable input suite
that the parser, roundtrip and transformation gates run against. The layout,
the provenance and licensing rules and the validation commands are in
[`corpus/README.md`](https://github.com/TalkBank/chatter/blob/main/corpus/README.md);
the test harness discovers the current population, so no count is kept here.
This page describes the tools that select, extend and measure the corpus.

## What the corpus must cover

Every concrete grammar node type must be exercised by at least one reference
file, so a grammar regression in any construct fails a gate. Constructs that do
not occur in real-world data (rare terminators, `uptake_symbol`,
`scoped_best_guess`, the `unsupported_*` nodes and `thumbnail_header`) are
covered by small handcrafted files in the corpus rather than left uncovered.
The language files are real conversations morphotagged with `%mor`/`%gra`, one
per language, drawn from the corpus data.

## Tools

| Tool | Path | Purpose |
|------|------|---------|
| `corpus_node_coverage` | `spec/tools/src/bin/` | Reports which concrete grammar node types the corpus exercises |
| `extract_corpus_candidates` | `spec/runtime-tools/src/bin/` | Scores and ranks candidate files in a corpus data directory for target languages |
| `perturb_corpus` | `spec/tools/src/bin/` | Produces error files by mutating a valid `.cha` file, and mines real data for tree-sitter ERROR nodes |

### Selecting language files

`extract_corpus_candidates` ranks files per target language. Its criteria:

- the file parses cleanly with tree-sitter (no ERROR nodes), which is mandatory;
- short files (a `--max-lines` bound, default 200, preferring 15-100 lines);
- varied tiers (`%mor`, `%gra`, `%pho`, `%com`);
- multiple speakers preferred;
- `Password` directories are skipped, for privacy.

Fresh `%mor`/`%gra` tiers for a selected file come from batchalign3
`morphotag` run in place over its language directory.

### Producing error files

`perturb_corpus --list` prints the mutation strategies; each takes a valid
`.cha` file and applies one controlled mutation:

| Perturbation | Mutation |
|-------------|----------|
| `delete-participants` | Delete the `@Participants` header |
| `delete-languages` | Delete the `@Languages` header |
| `delete-id` | Delete all `@ID` headers |
| `undeclared-speaker` | Change a speaker code to an undeclared `XXX` |
| `delete-terminator` | Remove the terminator from the first utterance |
| `extra-mor-word` | Add an extra word to the first `%mor` tier |
| `fewer-mor-words` | Remove a word from the first `%mor` tier |
| `delete-begin` | Delete the `@Begin` header |
| `delete-end` | Delete the `@End` header |
| `duplicate-participants` | Duplicate the `@Participants` header |
| `mor-terminator-mismatch` | Change the `%mor` terminator to differ from the main tier |

`--mine DIR` scans a data directory for tree-sitter ERROR nodes, again
excluding `Password` directories.

## Design notes

- **Perturbation beats mining for systematic coverage.** Well-curated corpora
  contain almost no tree-sitter parse errors, and mining is slow on large
  directories, so controlled mutation is the way to reach a specific error code.
- **Parser recovery codes are hard to trigger.** Tree-sitter's error recovery
  routes most malformed input through the generic path (E316) rather than the
  specific recovery codes (E319-E322, E376), so examples for those codes
  usually cannot reach them; their specs are `not_implemented` and say why.
- **Some codes have no emission path** (internal or reserved codes such as
  E001 and E002), so their specs document why no example is possible.
- **Adding files is purely additive.** New reference files extend the gate
  without disturbing existing ones.
