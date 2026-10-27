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

The node-incidence owner, `spec/tools/src/node_coverage.rs`, distinguishes
required concrete nodes, invalid-by-construction nodes and nodes awaiting
policy-aware review. Supertypes are accounted for separately. Unsupported and
recovery syntax must not be added to the valid corpus merely to fill a node
count; `unsupported_*` and `thumbnail_header` currently have explicit exclusions.
An excluded node appearing in the corpus requires adjudication, not automatic
promotion. In particular, separator padding reports E758 in ordinary CHAT but
is permitted by CA policy; its grammar node alone cannot establish invalidity.
The authored `ca/separator-padding.cha` reference promotes the legal CA example
from E758, with header order normalized so the complete document is valid. It
includes padded headers, main and dependent tiers. Removing its CA option must
restore E758; serialization removes padding without changing content.
A passing incidence check does not prove all grammar interactions,
semantic rules or executable branches are covered.

Preserve the distinction between authored controls and derived language
examples. Inspect each example's provenance and redistribution permission;
neither a directory name nor its `%mor`/`%gra` tiers establishes a common
production history or permission to redistribute it.

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

The authored `tiers/mor-ending-combinations.cha` and
`tiers/wor-ending-combinations.cha` promote the corresponding utterance specs
into complete documents with neighboring turns. They preserve eight MOR ending
forms and seven WOR endings plus its tag marker. Both parsers validate and
roundtrip these references. The shared independent spec expectations also check
every full-document utterance and tier across LF, CRLF and CR, preserving the
intended terminator and separator shapes. These are authored controls,
not externally attested transcripts. The inventory therefore distinguishes
nineteen named authored witnesses, while retaining all nineteen historical
attested-reference obligations. Each authored pair must
remain in its named fixture; another reference cannot silently replace it.
The authored `content/colon-separator.cha` reference follows the standalone-colon
construct spec. Both parsers retain its separator and distinguish it from
word-internal lengthening when the colon is attached to the preceding word.
Neither structural incidence nor these controls proves attestation, grammar
completeness or implementation coverage.

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
| `perturb_corpus` | `spec/tools/src/bin/` | Produces unreviewed text-mutation candidates, or reports syntax-recovery observations in mining mode |

### Selecting language files

`extract_corpus_candidates` ranks files by node/tier variety, length and
speaker count, with a morphology bonus. It rejects Tree-sitter ERROR and
MISSING nodes; by default it also requires zero Rust parse diagnostics and
zero validation diagnostics, including alignment. These checks use anonymous
transcript context, not filename-aware admission. The corresponding CLI
options can relax the Rust filters, so retain the actual options with results.

The current scanner selects the first three-character language entry rather
than establishing every word's language. Files shorter than ten lines or above
`--max-lines` (default 200) are skipped. `--max-files` bounds inspection and
`--top` truncates ranked results. The required `--node-types` path is checked
for existence; the ranking counts observed CST kinds rather than loading that
file as a completeness denominator.

**The extractor does not currently exclude `Password` directories.** Point it
only at an explicitly authorized, already curated input tree. Directory names
are not a privacy or licensing boundary. Repository enumeration, traversal and
selected-file read failures stop the command before it publishes a report;
an existing output report is left unchanged on these failures. Deliberate caps,
filters and top-N ranking still mean success is not an exhaustive inventory.
Reports contain source
paths; `--output` writes its report path without a no-overwrite guarantee.

Selection does not authorize changing original transcripts, regenerating tiers
in place, or publishing derived material. Preserve originals and retain the
source relationship, permissions and any separately approved transformation
before proposing a reference fixture.

### Producing error files

`perturb_corpus --output-dir PATH --list` lists the legacy text strategies
without writing candidates. The output-directory argument is required even
for listing. Their names describe intent, not verified seed validity, a
single-fault guarantee or an expected diagnostic:

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

For a source-bound main-tier terminator deletion from an admitted seed, prefer
the separate command documented in
[Spec Workflow](spec-workflow.md#mutation-candidates-are-not-golden-tests).
That section also owns candidate promotion and output-collision behavior.
Legacy text mutation does not validate its seed and can introduce multiple
defects. Candidate output planning refuses duplicate/existing destinations and
uses exclusive creation, but a later I/O failure can leave earlier new files.

`--mine DIR` looks for Tree-sitter ERROR or MISSING nodes and writes
`mining_summary.json`, not extracted CHAT fixtures. It filters paths with
case-insensitive `Password` components or `-xml` suffixes, but does not prune
those directory walks. Traversal and read failures may be skipped. Use only an
authorized input scope; filtering is not permission or anonymization. The
summary includes source paths and overwrites an existing summary; it is not
covered by candidate-output no-overwrite admission.

## Design notes

- Prefer rule-derived, reviewed controls over unbounded mining. An observed
  recovery result is not the expectation for a new diagnostic test.
- Diagnostic implementation status belongs to `spec/codes/error-codes.toml`;
  claims and dispositions belong to the owning specs. Do not infer that an
  internal diagnostic has no emission path because ordinary CHAT cannot
  intentionally produce a tool fault.
- Adding a reference changes every consumer that discovers the population.
  Check the affected parsing, validation, roundtrip and transform contracts;
  retain legal controls and review new exclusions rather than assuming that
  an additive file is behaviorally isolated.
