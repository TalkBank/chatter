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

Node incidence is necessary, not sufficient: a word with a shortening and a
word with a suffix do not witness their combination. Review ordered typed
content, category, suffix and annotation context together. The authored
`word-features/shortening-combinations.cha` reference covers repeated omissions,
suffixes, replacement wrappers and neighboring lengthening; its E209 specs pair
legal spoken segments with suffix-bearing words that contain only omitted
sounds. Existing source/wire controls consume the whole reference, while
targeted assertions pin the combinations rather than just counting words.

The authored `ca/word-boundary-combinations.cha` reference similarly preserves
leading inhalation and pitch, attached overlap points, partial-word underline
boundaries and adjacent lengthening in their exact typed order. Its E356/E357
controls remove only one underline marker pair beside pitch: a pitch shift
cannot replace an underline opener or closer. Existing overlap rules remain
distinct from utterance-wide underline balancing.

The authored `ca/delimiter-stretches.cha` adds selected paired prosodic
stretches across word boundaries and their overlap/lengthening combinations.
E230 controls pair each selected type and delete just one source-bound marker.
Different delimiter types pair independently across the utterance; an overlap
point cannot supply a prosodic boundary. These examples complement, rather than
repeat, the registry's symbol-incidence controls.

The authored `word-features/phonological-fragments.cha` reference combines the
`&+` category with lengthening, segment repetition and phonetic `@u` form.
These remain fragments: `%mor` and `%wor` exclude them, while `%pho` and `%sin`
count them. E387 feature controls add only a replacement annotation to an
otherwise valid fragment; word-internal features do not relax that rule.

The authored sound-category, CA-omission and segment-metadata combination
references extend this approach to fillers/nonwords, attached omission markers,
repeated clitic/compound/syllable boundaries, stress, longer lengthening and
form/language/POS context. CA parentheses surround the omitted lexical text,
not adjacent overlap or prosodic markers. Their policy controls retain the
single-shortening normalization boundary and keep parser-stage suffix/stress
rejection separate from validation-stage material and replacement rules.

The authored main-tier recovery control adds Unicode context, neighboring turns
and source-owned comment tiers. Its prefix/word mutations distinguish whole-file
recovery from strict fragment rejection; retained native carriers and their
models are checked independently. The grouped bare-ampersand case pins native
absorption of the next turn separately from retention of later healthy carriers.
Stacked marker controls travel repeated leading
overlap/underline and continuation CA, clitic, pause and stress choices. E252
edge-pause cases preserve their structured words for validation, while E245
marker-only words are rejected without inventing spoken material.

Real-data comparisons need explicit selection limits, source identities and
separate parse/validation exclusions. A recovered model is not a valid witness,
and physical filename checks and anonymous-input checks have different context.
Observed frequency in a bounded sample is not population prevalence. Review a
missing combination against its rule before adding an independently authored
control; retain derived material's source relationship and permissions instead
of calling renamed text synthetic. Language-marker shape alone does not prove
resolved word language. Such comparisons guide curation, not validity rulings
or claims of complete execution coverage.

## Tools

The authored `tiers/mor-gra-combinations.cha` control combines repeated
post-clitics, POS subcategories, ordered flat/keyed features, a multivalue
feature, Unicode and a compound lemma. Its source-owned boundary variants keep
MOR item counts separate from expanded word chunks and GRA relations. Feature
emptiness remains an independent validation policy; internal morphology
recovery retains distinct document and strict-fragment behavior. The neighboring
turns and comments make attachment and source preservation observable.
This is a selected interaction matrix, not exhaustive morphology or coverage.

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
