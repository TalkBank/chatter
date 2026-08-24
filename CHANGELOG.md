# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Before 1.0, breaking changes to the CLI or library APIs bump the minor
version and are listed under "Changed" / "Removed".

## [Unreleased]

### Changed

- **Tree-sitter 0.26.13** across the workspace, the grammar crate, the spec
  workspace and the `tree-sitter-cli` devDependency, plus desktop npm
  devDependency bumps and jsonschema 0.51.

  **Validation verdicts: UNCHANGED over the sampled corpus, and that needed
  measuring rather than assuming.** 0.26.13 avoids wide error nodes on
  unparseable input, which is a change to RECOVERY, so it can move what
  `validate` reports on malformed CHAT without changing one byte of the
  regenerated `parser.c` (which is in fact byte-identical here) and without any
  fixture in the suites noticing, because they all parse. The corpus
  differential is what can see it: over 2,147 files at stride 50, stratified per
  repo, against the v0.13.0 fleet build, there were no new error codes, no
  per-code count increases, no newly failing roundtrips and no new cross-backend
  disagreements. That is a statement about the sample, not about the whole
  corpus; at this stride a defect in a few dozen of ~106,000 files could still
  hide.

  The generated typed CST traversal is byte-identical apart from its generator
  provenance stamp, and `just spec-gen` moved no artifact.

## [0.13.0] - 2026-08-21

**Validation verdicts: UNCHANGED.** Nothing here moves what `validate` reports
on a CHAT file. Every prior entry states this either way, and the published
promise is that an entry without the note did not move its verdicts, so an
entry that omits it cannot be told from one nobody filled in.

### Removed

- **`talkbank_transform::capitalize` is GONE.** The English capitalization
  transform announced in 0.7.0 (`capitalize_english`, `capitalized_pronoun_i`,
  `is_capitalizable_initial`, `capitalize_first`) is deleted. It is the only
  change here that affects a library consumer.

  Why: chatter is the CHAT-format authority, and English orthography is a
  convention of one language rather than a fact about CHAT. Nothing inside
  chatter ever called it; its two users were downstream generators, which
  wanted different policies. One of them had already written its own version
  of `is_capitalizable_initial` and documented that chatter's answered a
  different question. The module also had no stopping rule: pronoun "I" and
  sentence capitals today, then contractions and proper nouns on request.

  If you used it: copy it into your own generator, where the policy belongs.
  It is built entirely on public API (`walk_words_mut`, `Word::category`,
  `Word::untranscribed`), so nothing about the move needs chatter internals.
  Note that the version shipped here had three defects in
  `is_capitalizable_initial`, all from deciding a structural question from
  `cleaned_text()`, which strips the very prefixes the question needs: a
  non-letter-initial word did not consume the utterance-initial slot, so the
  capital landed on the following word; an apostrophe-initial word received no
  capital at all; and the `&`-fragment guard could never fire, so a filler took
  the sentence capital. Ask the typed model instead.

  `num_words` is unaffected and stays: it serves E220, a rule chatter enforces.

### Changed

- **`chatter new-file` builds its template through the typed model.** The
  emitted skeleton is produced by parsing and serializing a typed `ChatFile`
  rather than formatting text, so it is roundtrip-proven by construction; the
  default output is unchanged.

- **`docs/errors/index.md` is one table, sorted by code.** It emitted one `##`
  section per spec, 236 of them with 31 exact duplicates, each over a
  single-row table, and reprinted every description. It is now a flat table
  with Code, Name, Category, Kind, Level and Status columns: 234 lines where it
  was 2,247. Anything that scraped the old section structure will need
  updating; anything that followed the `E###.md` links is unaffected.

- **The error-spec format is TOML frontmatter with a required CLAIM per
  example.** Landed in stages within this unreleased window, superseding
  earlier entries' details: metadata moved from `## Metadata` bullets to `+++`
  frontmatter (an unknown or missing field is a load error); the authored
  `Layer` field was then DELETED (which pipeline stage catches a rule is
  recorded per example in the generated `spec/observations/` snapshot, and
  every example is a fixture in the validation corpus); and
  `Expected Error Codes` was replaced by `claim = 'violates' | 'legal' |
  { subsumed_by = ... }`, whose negative halves (a code that must NOT fire)
  are enforced. `level` moved from the spec file to the example, where it is
  required: a code can be violated at one level in one example and another in
  the next (E519 has header-level and utterance-level violations), so the fault site
  is a fact about the example; a code's page renders the distinct set. A
  non-empty `Description` remains required. This matters only if you author
  specs against `spec/errors/`.

- **Corpus tests require `TALKBANK_DATA`.** The re2c integration tests defaulted
  to a hard-coded directory under `$HOME`, which could only ever be right on one
  machine and silently sent everyone else to a path that does not exist. The
  variable is now required and its absence fails loudly. `just corpus-tests`
  needs it set; the default test suite is unaffected, since those tests are
  `#[ignore]`d.

### Internal

Not part of any published API, listed because the commits are marked breaking:
`spec/errors/*.md` now has ONE parser in the spec workspace rather than two (`ErrorCorpusSpec` and
its types are deleted), and the spec format's vocabulary moved to a new
dependency-light `talkbank-spec-vocabulary` crate that both cargo workspaces
share. The `generators` and `talkbank-parser-tests` crates are `publish =
false`.

## [0.12.0] - 2026-08-16

**Validation verdicts: CHANGED.** Four rules report where they were silent:
E241 on illegal untranscribed spellings, E756 on any empty dependent tier,
the participants check on files declaring an empty set, and the re2c backend
on empty tiers it used to paper over. If you gate a pipeline on `validate`,
diff your own corpus before upgrading; see
[What a Version Bump Promises](https://talkbank.github.io/chatter/chatter/integrating/versioning.html).

Adjudicated against real corpus data before shipping, per the standing
grammar-change gate. The full-stride differential against the shipped 0.11.0
build covers all 106,507 corpus files and reports EVERY error code unchanged
except E241, whose 661 new instances are every one an illegal short or miscased
spelling of an untranscribed marker: 624 `ww`, 18 `Www`, 10 `XX`, 6 `Ww`, 2
`Xxx`, 1 `Xx`. All adjudicated INTENDED, the rule correctly flagging invalid
data, and the 194 affected files join the cleanup queue. No new cross-backend
disagreements and no newly-failing roundtrips.

### Added

- **E241 rejects the illegal untranscribed spellings.** The corpus authority
  ruled that `ww` is not legal CHAT and `www` is canonical, adding `yy` against
  `yyy` unprompted. Which spellings are wrong is now DERIVED from the canonical
  set rather than listed, so `ww` cannot be missed while `xx` and `yy` are
  caught, which is what happened before. Eight instances in the differential
  sample, every one adjudicated as the rule correctly flagging invalid data.
- **E756 covers every dependent tier, not only `%x*`.** A tier line whose
  payload is absent or whitespace-only declares nothing. The rule always said
  that; only its name was `%x`-specific, and it could not be applied to a
  standard tier until the model could represent an empty one. Before this, an
  empty `%eng:` was read as VALID by the re2c backend and rejected by
  tree-sitter through an undescribed code, so the two backends disagreed about
  a file neither could explain. Zero instances in the differential sample: the
  construct is invalid CHAT and correspondingly rare.

  The rule now reaches EVERY tier whose grammar body is free text, which is
  every dependent tier except the structured ones (`%mor`, `%gra`, `%pho`,
  `%mod`, `%sin`, `%wor`), whose bodies are not free text and whose empty case
  fails earlier and more specifically. That boundary is a grammar fact, not a
  list: a tier qualifies exactly when its rule marks its body `optional(...)`.
  `%tim` gained an `Empty` state to make this expressible, since both of its
  content variants hold a non-empty string; the Phon tiers (`%xmodsyl`,
  `%xphosyl`, `%xphoaln`, `%xphoint`) answer from the word or group count they
  already reported.

### Fixed

- **An empty dependent tier is no longer papered over.** The re2c backend met
  `%eng:` with no content and substituted a single space, which made the tier
  look well formed and the whole FILE read as valid where tree-sitter reported
  errors. The model can now say that a tier declares nothing, so the parser
  reports what the file contains and E756 judges it.
- **An empty `%x` tier survives a roundtrip, and `normalize` no longer swallows
  the file.** `%xtst:` with no content reported E756 from the PARSE path and
  returned without adding the tier to the model, so the line vanished on
  roundtrip while an empty `%eng:` was preserved. Worse, because the report came
  from parsing rather than validation, `chatter normalize` treated the whole
  file as unparseable and wrote NOTHING. The parser now says what the file
  contains and the validator judges it, as it does for every other tier kind.
- **An empty `%tim:` is a `%tim` tier.** The re2c backend lowered it to an
  unsupported DEPENDENT TIER and reported E605, "unsupported dependent tier
  '%tim'", about a tier name that is perfectly supported; a whitespace-only body
  additionally drew E603 ("Invalid %tim tier format: ''") alongside E756, two
  codes for one fact and the more specific of them false. Same for an empty
  `%xphoaln:` and `%xphoint:`, which conflated an absent body with a malformed
  one. All four now report E756 on both backends.
- **The participants check reads the declaration.** An empty participant set
  used to disable the check rather than fail it, so the files least likely to
  be well formed were the ones exempted from the rule.
- **An annotation's separator is not part of its text.** `[=!  contacts]`,
  written with two spaces, parsed as `" contacts"` in one backend and
  `"contacts"` in the other. That was the last content-level disagreement
  between the two parser backends across all 107,403 corpus files.
- `chatter validate --format json` no longer writes cache housekeeping to
  stderr. Two facts leaked there: `Cleared N cache entries` on every `--force`
  run, and `note: pruned N unreachable cache row(s)...` whenever a prune fired.
  Both broke the documented promise that JSON mode's stderr is empty, and the
  test suite contained two tests with contradictory expectations about it, one
  requiring stderr empty and one asserting it contained the cleared count. The
  first only failed when a prune happened to fire, which is why both shipped
  green through four releases.

### Changed

- **Breaking (library): `TimTier` gained a third variant.** `TimTier::Empty
  { span }` represents a `%tim:` line that declares nothing, which neither
  `Parsed` nor `Unsupported` could hold: both carry a `NonEmptyString`. Code
  matching on `TimTier` exhaustively must add an arm. `TimTier::empty()`
  constructs one, `declared_content()` returns `None` for it (`as_str()` still
  flattens to `""` for `Display` and serialization), and the serde form is
  unchanged apart from `""` now deserializing to `Empty` instead of erroring.
- **Breaking (library): the `test-utils` feature is REMOVED**, and with it
  `ChatCleanedText::test_unchecked` and `ChatRawText::test_unchecked`. Not
  renamed: gone. This is the breaking change that bites FIRST, because cargo
  refuses to resolve a graph that asks for a feature which no longer exists, so
  it fails before anything compiles and is invisible to a "what will fail to
  build" scan. A consumer sees:

  ```
  package `X` depends on `talkbank-model` with feature `test-utils`
  but `talkbank-model` does not have that feature
  ```

  Build fixtures through the parser instead: `TreeSitterParser::parse_word`
  followed by `ChatCleanedText::from_word`. The hatch was removed because a type
  whose existence proves "this text came from a parsed AST" is only as strong as
  its weakest constructor, and one any dev-dependency could switch on was that
  constructor. Downstream adoption on the day of release found three fixtures
  that had been asserting on a shape production cannot emit (a terminator in a
  `words` list), passing only because the hatch let them fabricate it.
- **Breaking (library): `BulletContent::empty()`.** A named constructor for a
  payload that carries nothing, distinct from `from_text("")`, which fabricates
  an empty text segment that is not in the file. Additive; no existing call site
  changes.
- **NDJSON surface: a new record type.** Those facts now arrive on stdout as
  `{"type":"cache","action":"clear"|"prune"|"warning",...}`, emitted only when
  cache maintenance did something. Silencing them under `--format json` was
  considered and rejected: they are results a caller can act on. A consumer
  that ignores unknown `type` values needs no change; one that errors on an
  unrecognised `type` will see these. The `type` field's documented value set
  is now `"file"`, `"summary"`, `"cache"`, and the contract page says to treat
  unknown values as ignorable.
  See [Diagnostic contract](https://talkbank.github.io/chatter/chatter/integrating/diagnostic-contract.html).

## [0.11.0] - 2026-08-13

**Validation verdicts: CHANGED, in BOTH directions.** Six error codes that had
silently degraded to E316 "unparsable content" now report themselves again
(E202, E307, E311, E314, E370, E375), and two false positives are gone. If you
gate a pipeline on `validate`, diff your own corpus before upgrading; see
[What a Version Bump Promises](https://talkbank.github.io/chatter/chatter/integrating/versioning.html).

Adjudicated against real corpus data before shipping: the operator's corpus
differential over a 2158-file stratified sample reports byte-identical per-code
counts and no newly-failing roundtrips against v0.10.0. The changes below are
all on malformed input, which a curated corpus contains almost none of.

**Library APIs: BREAKING.** This release changes the public API in several
ways. The list below is from a mechanical diff of the public surface between
the two tags, made after the notes first shipped saying "additive" and then
being corrected twice as a downstream consumer hit one break after another. A
release note written from memory of a 415-file change is a guess; this one is a
measurement.

Removed items (6):

- `FormType::A`. The `@a` marker was retired by the corpus authority in 2024
  and is absent from the form-marker registry that now generates every site of
  that closed set; the variant survived only because sixteen hand-written
  copies of the list disagreed. No replacement: the construct is not CHAT.
- `ALL_MARKERS`, `all_markers_string`. Superseded by the same registry.
- `collect_bracketed_content`, `collect_bracketed_item`. Superseded by the
  typed traversal.
- `counts_for_tier_in_context`. Use `counts_for_tier`, now re-exported at
  `talkbank_model::alignment`.
- `iso`.

Added, and breaking for an exhaustive match:

- `FormType::Undeclared(String)` carries the raw text of a marker naming no
  declared form, so `word@zz` roundtrips instead of being silently rewritten to
  `word@z:zz`.

Changed signatures:

- `ChatFile::validate` and `validate_into` take `TranscriptName<'_>` rather
  than `Option<&str>`. `None` becomes `TranscriptName::Anonymous`; a real name
  becomes `TranscriptName::Named`. The `Option` could not say which of "no
  name" and "a name we failed to read" it meant, and both reached the same
  branch.

**Library APIs: additive.** `talkbank_model::alignment` re-exports
`walk_words`, `walk_words_mut` and `counts_for_tier`, which previously required
naming the `helpers` module.

### Fixed

- **A recovery node could displace an entire `tier_body`.** An utterance ending
  in " ." was told its terminator was missing (E305), and a retrace or bracket
  at utterance start took the rest of the line with it. The parser was reading
  its own recovery artefact as evidence about the user's file. The generated
  typed CST traversal is regenerated from a generator that no longer absorbs an
  ERROR child at whatever position its cursor had reached.

- **Six codes degraded to the E316 catch-all.** Which classifier a recovery node
  reached was decided by WHERE tree-sitter had put it, so the same construct was
  named precisely at utterance start and generically after spoken material.
  `MainTierRegion` is now stated by the caller that knows it, and every
  main-tier Unexpected sink routes through one owner.

- **E246 blamed a lengthening marker for a stray tab.** The classifier saw a `:`
  before the recovery node, and that `:` was the SPEAKER's. A tab inside the
  main tier now reports the tab.

- **E758 pointed at whitespace nowhere near a tab.** It claims "extra whitespace
  between the tab and the tier content"; filling that slot never established the
  adjacency the sentence asserts, so ordinary space between two words was
  reported as a leading-space violation. The span is now built only when it
  starts at the tab's end byte.

- **E754 retired.** `@l` and `@ls` no longer require a single character.

- **Windows: the content-catch-all gate reported every exempted file as new**,
  because repo-relative paths were compared with the host separator against a
  forward-slashed list.

### Changed

- Two wall-clock test assertions became hang detectors with order-of-magnitude
  ceilings; both were tuned to one machine and one of them turned the Windows
  matrix red doing correct work.

- `chatter-desktop` and `talkbank-llm` set `doctest = false`. Neither has doc
  examples, and each was paying a full rustdoc compile to run zero doctests.


## [0.10.0] - 2026-08-07

**Validation verdicts: CHANGED, in the stricter direction.** Two new error
codes reject retrace constructions that previously passed silently, and two
existing rules stopped being suppressed by the shape of the content they were
asked about. Adjudicated over all 107,376 corpus files: E377 fires 53 times in
42 files and E378 15 times in 12 files, all real transcription defects and
queued for data cleanup; restoring E372 and E704 costs **zero** new instances,
so those two are pure correctness.

**Library APIs: BREAKING.** `Retrace` loses its `annotations` field, both
content enums gain an `AnnotatedRetrace` variant, and per-word language records
lose `word_index`. Pre-1.0, so this is a minor bump.

Every fix below except the `merge` one is the same defect: a traversal carrying
its own private list of which content variants contain other content, plus a
catch-all arm for everything the list forgot. Five such traversals existed.

### Added

- **E377 `RetraceWithNoMaterial`.** A retracing marker whose material is
  another marker, so it retraces nothing of its own. One rule covers the
  unbracketed `на [//] [/] на` and the bracketed `<<a> [/]> [//]`, because the
  lowering folds both into the same tree; naming it for the shape rather than
  for a spelling is what makes that possible. Deliberately narrow: 11,163
  retraces in the corpora sit inside another retrace and only **4** wrap a lone
  marker, so a "no retrace inside a retrace" rule would have rejected ordinary
  stutter chains (`<the [/] the piece> [//] the people`) in exactly the aphasia
  and fluency corpora that study them.
- **E378 `RetraceWithoutWords`.** The retraced material must contain a word at
  some depth. Phrased against absent WORDS rather than a present event, so
  `<the floor on the &=laughs water> [//]` stays legal while `<&=sigh> [/]`
  does not. The boundary falls out of what the model already calls a word:
  `0det [/] 0det dog` is legal (an omitted determiner is lexical content),
  `<xxx> [/] xxx` is legal (untranscribed speech is speech), and
  `0 [=! snuffles] [/] ok` is not.

### Fixed

- **A retracing marker's position among its annotations was discarded.**
  `dog [* p:w] [/] dog` and `dog [/] [* p:w] dog` are different claims: the
  first codes the error on the abandoned attempt, the second on the retrace.
  chatter built the identical model for both and wrote the first back as the
  second. **12,226** places in the corpora put an annotation immediately before
  a retrace marker. A second adjacent marker had nowhere to go at all, so
  `на [//] [/] на` round-tripped as `на [/] на`, losing a marker outright in
  **105** places across 46 files, 31 of them bilingual or language-impairment
  corpora where disfluency is the research variable.
- **E704 (overlapping bullets) was silently disabled** on any utterance whose
  content held a retrace or a group. The predicate for "does this utterance say
  anything timeable" recursed into neither, so two speakers' bullets could
  overlap by a full second and report nothing whenever either line contained a
  retrace.
- **E372 (nested quotation) was invisible below every container except an
  annotated group.** `“a <“b”> [/] c”` is a quotation inside a retrace inside a
  quotation, and reported nothing.
- **Per-word language metadata skipped every word inside a quotation,
  phonological group, sign group or retrace**, in a tool whose per-word
  language resolution is the point. `hao3 “ni3” <ma> [/] ma` produced records
  for two words out of four.
- **The re2c backend diverged from tree-sitter on marker runs.** It split a run
  where tree-sitter folds it, dropped retrace markers on events, and never
  raised E377 at all, despite three doc comments saying it did. The
  parser-equivalence gate now covers all three.
- **`chatter merge` dropped donor `@Comment` rows** when the reference file had
  none of its own.
- **Five missing CHANGELOG link references.** Every release from v0.6.0 to
  v0.9.1 shipped a `## [X.Y.Z]` heading with no matching `[X.Y.Z]:` definition.
  It renders as literal bracketed text rather than as a broken link, so the
  book's link check reports zero errors and cannot see it. The version gate now
  requires both halves of the entry.

### Changed

- **`Retrace::annotations` is gone.** Annotated retraces are
  `AnnotatedRetrace(Box<Annotated<Retrace>>)` in both content enums, parallel
  to the existing `Group`/`AnnotatedGroup` pair. Parser lowering is now a left
  fold over the marker run, one wrapper per marker, which absorbed three
  hand-rolled copies of the same tail.
- **Adjacency is validated, not refused at parse time.** Folding the offending
  input faithfully means it still round-trips, so a file that trips E377 stays
  recoverable rather than being partly discarded during recovery.
- **Per-word language records no longer carry `word_index`**, and
  `get_word_language` is removed. The records are a `#[serde(transparent)]`
  list, so a consumer reads position with `enumerate()`. The stored index was a
  second representation of that position whose documentation claimed it matched
  the tier-alignment domains; it cannot, because `%mor` excludes retraces and
  `%pho` counts them, so no single integer indexes both.
- **`--parser re2c` is documented as reporting unreliable diagnostic
  positions.** The lexer emits spans and the parser discards them; until that is
  plumbed through, the flag is for cross-checking verdicts, not for locating
  them.

### Internal

- **Design rule 3 (no `_ =>` catch-all over the content enums) is now enforced
  by the compiler**, through `#![deny(clippy::wildcard_enum_match_arm)]` added
  per file as each is cleaned, seven so far. A reintroduced catch-all is a
  compile error at the exact line, which no scalar count could be.
  `cargo run -p talkbank-parser-tests --bin audit_content_catch_alls`
  inventories the 24 modules still to clean.
- **`ContentStructure` is the single owner of which content contains what**,
  carrying `WordRef` and `GroupRef` payloads so a caller can ask not merely
  whether something is a container but which one. That set had been encoded
  independently in 18 files, and two copies disagreeing about phonological and
  sign groups is what let E377 escape from inside `‹...›`.

## [0.9.1] - 2026-08-05

**Validation verdicts: UNCHANGED.** No rule was added, removed or altered, and
no file changes its valid/invalid verdict. This release completes the library
API that v0.9.0 closed the fields on, and every entry below was found by
compiling a real downstream consumer against v0.9.0, which is a gate this
project did not previously have.

### Added

- **`into_vec()`, `take()` and `retain()` on every collection newtype.** v0.9.0
  made these types' inner fields private but shipped only the READING half of
  the resulting API. With no consuming accessor there was no way to move the
  items out, so a consumer rebuilding a content list or resegmenting a file
  could only clone through `as_slice().to_vec()`, on paths that run per
  utterance; and with no `retain`, every caller wrote take-edit-rebuild by
  hand, which hands a closure a `&mut Vec<_>` and is `DerefMut` under another
  name. Downstream, these three methods delete three helper functions and
  sixteen copies of one incantation.
- **One owner for that API.** `collection_newtype_ops!` now emits the accessor
  set for all seventeen `Vec`-backed newtypes. They had drifted while
  hand-written: `into_vec` was on 6 of 17, `as_slice` on 11, `as_mut_slice` on
  6, so what a consumer could do depended on which type it happened to hold.
- **`TierContentItems` and `BracketedItems` are re-exported from `model`.**
  Both were `pub` but reachable only through a glob, so a consumer could not
  name the type to reconstruct one after its field closed.

### Fixed

- **A doc-comment claim that was not true.** Several comments said
  reconstruction "goes through `new`, where a future invariant would be
  enforced". Every one of these seventeen types also has `impl From<Vec<T>>`
  and `impl Deref<Target = Vec<T>>`, so `new` is not the only door and no
  invariant is enforceable on them today. Closing the fields prevents literal
  construction and destructuring, and nothing more. The docs now say that, and
  name the open question (whether `From` should become `TryFrom`) rather than
  implying it is already answered.
- **A test whose "unique" temporary directory repeated 98% of the time.** The
  name was the pid plus `SystemTime::now()`, but the pid is constant across a
  test binary and 19,584 of 20,000 consecutive `SystemTime` samples measured
  identical, so parallel tests shared a directory and one's cleanup deleted the
  other's file. Now a process-wide counter.

## [0.9.0] - 2026-08-05

**Validation verdicts: CHANGED, in the permissive direction.** Files that
earlier versions wrongly REJECTED now parse: an unquoted `@Media` filename may
contain dots, parentheses, interior spaces and non-ASCII characters. Nothing
that used to pass now fails. The corpus differential over a 2,136-file
stratified sample of the reference corpora reports no new error code and no
count increase on any code, and no newly-failing roundtrip file.

Three rules changed with no effect on any known transcript. E767 (new) reports
whitespace before the `@Media` comma; those files were already invalid, and what
changes is the diagnostic. E768 (new) cannot be reached from a `.cha` file at
all. E602 became E756 on empty user-defined tiers, a construct that occurs zero
times in the wild corpus.

This release closes the library's newtype surface ahead of 1.0, so it carries a
lot of breaking API change and very little behaviour change.

### Fixed

- **`@Media` rejected legal media filenames.** `media_filename` was an ASCII
  allowlist (`[a-zA-Z0-9_-]+`), so a dot, a space, a parenthesis or any non-ASCII
  character made the header fail to match. The failure surfaced as E330 "Missing
  media_type node" on a line that visibly ended in `, audio`, and as E525 about a
  header chatter had recognised perfectly well. A filename is now defined the way
  the format defines it, as everything up to the comma that introduces the media
  type, with the quoted form still available for URLs (which may contain commas).
  This was costing real transcription runs: a media file named in Chinese, or
  containing a space, could not be referenced at all.
- **A `%mor` tier could be silently dropped on an empty user-defined tier.**
  `UserDefinedDependentTier::content` was a `NonEmptyString`, so the model could
  not represent a `%x` tier with empty content and the two parsers disagreed about
  what to do with one. The state is now representable and rejected by a validation
  rule (E756) rather than being unrepresentable and handled twice.
- **The validation cache could panic on drop** inside an async runtime. This was
  the second half of the nesting bug fixed in 0.8.0: the first half covered the
  call, this one covers teardown.
- **A `%mor` clone was a no-op**, cloning a reference rather than the owning
  vector it was meant to copy.
- **A declared speaker with no `@ID` was reported as undeclared.** The
  "Speaker *X not declared in @Participants" check read the
  `@Participants`-to-`@ID` join rather than the `@Participants` header, so for
  a speaker declared without an `@ID` it asserted the opposite of the file. The
  missing `@ID` is a real fault and E522 already reported it correctly. The
  neighbouring "@Participants header missing or has no participants" check had
  the same confusion: an empty join and an absent header are different facts.
- **E767 never fired in the editor.** It was implemented as a file-level sweep,
  and the LSP calls `validate_headers_only`, which does not run those. Both
  `@Media` payload rules now live on the per-header dispatcher that every entry
  point calls, so the CLI and the editor report the same thing. The LSP's
  per-speaker code lens had a quieter version of the roster bug: a speaker
  without an `@ID` got no lens while speaking.
- **A spec file had been failing to load silently.**
  `E502_wor_cascade_regression.md` carried a malformed title, and the loader
  downgraded every load failure to a warning on stderr, so it simply left the
  corpus unnoticed. The loader now fails closed on a spec it cannot parse, and
  distinguishes a spec from the prose that shares its directory.

### Added

- **`ChatFile::declared_speakers()`** returns every speaker declared in
  `@Participants`, in declaration order, each enriched with its `@ID` metadata
  when present. `participants` is populated from the `@Participants`-to-`@ID`
  join, so a speaker declared without an `@ID` raised E522 and was then absent
  from the map: consumers saw fewer speakers than the file declares. Prefer this
  for "who is in this transcript"; `all_participants()` remains the `@ID` join.
- **`ChatFile::participant_entries()`**, the named `@Participants` extraction,
  alongside the existing `id_headers()`.
- **`MorWord::analysis()`** borrows the analysis half of a `%mor` item
  (`lemma[-Feature]*`) so a consumer whose token model keeps the tag and the
  analysis in separate fields need not serialize the whole item and strip the
  `POS|` prefix back off. `MorWord::write_chat` now delegates to it, so the two
  renderings cannot drift.
- **`DependentTierEntry::kind()`, `span()` and `content_span()`**, the last
  giving the byte range of a tier's content without its label or terminator.
- **`MediaFilename::parse()`, `unquoted()`, and `MediaFilenameProblem`.**
- **E767**: whitespace between the `@Media` filename and its comma. Reported
  from the validation layer so both parser front ends raise it from one
  implementation.
- **E768**: an `@Media` filename that cannot be written to a header and read
  back unchanged. Unreachable from CHAT by construction; it guards the JSON
  ingress, where a document can carry a value no transcript could express.
- **`string_newtype_read_impls!`**, the read and render surface shared by every
  string newtype, so a newtype WITH an invariant can share it instead of copying
  it.
- **`Status: unreachable_from_chat`** for error specs: a rule that IS
  implemented but that no CHAT input can trigger, so it carries no corpus
  fixture and owes a named out-of-corpus test instead. This closes a hole in
  the gate meant to stop an implemented rule shipping untested: a spec with no
  example used to fail to parse, and the loader turned that into a warning, so
  the gate never saw the one case it names. Both directions are now checked, a
  spec marked unreachable that carries an example is also an error.

### Changed

- **BREAKING: no model newtype exposes its inner field.** Every newtype in the
  model, including every one generated by `string_newtype!`, now has a private
  field. Code reading `.0` uses `as_str()` / `as_slice()` / `raw()`; code
  mutating through it uses the named accessors.
- **BREAKING: `DerefMut` is gone from the collection newtypes.** While it
  existed, a private field bought nothing: any caller could still push, clear or
  replace the contents. `as_mut_slice()` allows element mutation without allowing
  the collection to be resized.
- **BREAKING: `MediaHeader::new` takes a `MediaFilename`,** not
  `impl Into<MediaFilename>`, and `MediaFilename` has no `new`, no `From<&str>`
  and no `From<String>`. `parse` is the only way in. An `@Media` filename
  containing the delimiter was constructible, and `build_chat` built one.
- **BREAKING: `build_header_lines` and `build_media_header` are fallible,** and
  `BuildChatError` gains a `MediaFilename` variant, because a caller-supplied
  media name is external input that `@Media` cannot always represent.
- **BREAKING: `UserDefinedDependentTier::content` is no longer a
  `NonEmptyString`.**
- **BREAKING: the crates are edition 2024.**
- Deserialization of `MediaFilename` is lenient, like every other checked
  newtype in the model: the serde boundary reconstructs what the document held
  and validation reports the violation with a code and a span.


## [0.8.0] - 2026-08-03

**Validation verdicts: UNCHANGED.** No rule was added, removed or altered, and
no file changes its valid/invalid verdict in this release. What changes is that
the desktop app can run at all, that runs differing only in `--suppress` share a
cache again, and the library API named under "Changed" below.

### Fixed

- **Chatter Desktop can validate again.** Since v0.6.0 the desktop app could not
  start a run at all: it stopped on "Starting..." forever, on every machine and
  every folder. Tauri drives a command on its async runtime, and the validation
  cache bridges its synchronous API to an async database by owning a runtime and
  blocking on it; nesting runtimes panics, the panic unwound out of the command,
  and the IPC call then never resolved OR rejected, so the window had nothing to
  report and no error to show. The cache now runs such a call on a thread with no
  ambient runtime, so nesting cannot arise, and the desktop `validate` command
  always produces an outcome, reporting a panic as a failed run rather than as
  silence. The CLI was never affected. Introduced 2026-07-07; shipped in v0.6.0
  and v0.7.0.

### Changed

- **Desktop commands return typed errors instead of `String`.** Each command now
  names the failures it actually has (`TargetError`, `ValidationStartError`,
  `ClanError`, `InstallCliError`, `RevealError`, `ExportError`,
  `OpenExternalError`), so a failure can be matched on and carries its source
  error. Errors still cross the IPC boundary as the same display text, so
  nothing the user sees changes.

- **`--suppress` no longer throws the validation cache away.** Suppression is a
  presentation preference: it changes which diagnostics are printed, never which
  ones the validator computes. v0.6.0 folded the suppression set into the cache
  key, so every distinct `--suppress` list got its own private cache and
  `chatter validate ~/corpus` followed by `chatter validate --suppress xphon
  ~/corpus` re-validated all ~106,000 files from cold instead of hitting the
  cache. Runs that differ only in `--suppress` now share one cache;
  `--strict-linkers`, which genuinely turns extra checks on, still validates
  afresh. Suppression behaviour itself is unchanged: a suppressed code is not
  reported, and a file with other diagnostics still counts invalid.

- **The cache no longer grows without bound across releases.** Every read binds
  the current rules version, so rows written under a superseded one can never be
  matched again, yet nothing deleted them: only a 30-day age cutoff existed,
  which answers a different question. Each release therefore stranded a complete
  copy of the corpus in the database, which had reached 464,773 rows across 88
  versions (about 190 MB of a 243 MB file) for a corpus of ~106,000 files.
  Opening the cache now deletes rows outside a two-generation window (the
  current version plus the most recently written previous one, so a rollback or
  a bisect is not cold), rewrites the file so the space actually returns to the
  filesystem, and reports what it reclaimed.

- **Rule selection and presentation policy are now separate types.**
  `ValidationConfig` held both "which rules run" and "how diagnostics are shown",
  and the validation cache key was derived from the whole thing, which is what
  let a display preference partition the cache. It is replaced by
  `talkbank_model::RuleSelection` (what is computed; the only input to the cache
  key) and `talkbank_transform::PresentationPolicy` (what is shown). The cache
  crate cannot name the second, since the crate that owns it depends on the
  cache, so folding a display preference into the key is now a compile error
  rather than a judgement call.

  Library callers: `ChatFile::validate_with_config` and
  `validate_with_alignment_and_config` are now `validate_with_rules` and
  `validate_with_alignment_and_rules`, taking a `RuleSelection`, and they report
  the complete diagnostic set with nothing filtered. `ConfigurableErrorSink`
  moved to `talkbank_transform` and takes a `PresentationPolicy`. The validation
  runner's config field `model_config` is now the pair `rules` and
  `presentation`.

## [0.7.0] - 2026-08-03

**Validation verdicts: UNCHANGED.** No rule was added, removed or altered, and
no file changes its valid/invalid verdict in this release. What changes is what
a run REPORTS about itself when it does not complete normally.

### Changed

- **A validation run now always terminates its event stream, and says how.**
  Previously a run whose thread died emitted no terminal event at all, and the
  three surfaces each guessed differently: the CLI exited non-zero, the desktop
  app waited forever showing "Discovering files", and the TUI marked the run
  COMPLETE, so a dead run was presented as a finished one. Terminality now
  belongs to the runner, which guarantees a terminal event on every exit path,
  so all three surfaces inherit the same guarantee instead of reconstructing it.

- **A run that lost files can no longer report success.** A panicking worker was
  caught, logged where no graphical user could see it, and then ignored: the run
  reported `Finished` with partial statistics, so a 500 file corpus could
  validate 480 and be presented as "all valid". `Finished` now means every
  discovered file was accounted for and is the only basis for a claim about the
  whole input; a run that covered less reports `FinishedIncomplete` with the
  number of files lost. **`chatter validate` exits non-zero in that case**, where
  it previously exited 0.

- **Cancelling a run now cancels it.** The cancel request was a single token on a
  channel that the dispatch loop, every worker, and the end of run check each
  consumed destructively, so exactly one of them observed it: cancelling stopped
  one worker while the rest drained the queue, and the run's own statistics
  usually recorded that it had not been cancelled.

- **`ValidationEvent` gains `Aborted` and `FinishedIncomplete`** (BREAKING for
  library consumers). The enum is deliberately NOT `#[non_exhaustive]`: a
  consumer that upgrades gets a compile error and has to decide what a dead or
  partial run means for its own interface, rather than silently inheriting
  "pretend it finished", which is the defect these variants exist to fix.

### Fixed

- **Desktop: a run that never started looked identical to one in progress.** The
  app showed "Discovering files" from the moment it sent the request, and the
  backend's own discovery event set the same state, so a backend that never
  answered was indistinguishable from one still working. The two are now
  separate states: the app shows "Starting" until the validator actually
  responds, and says so if that takes more than a few seconds. A run stuck there
  is a start-up fault rather than anything about your files, which is worth
  quoting in a bug report.

- **Desktop: an aborted run is no longer a dead end.** It reports why it stopped
  and offers Re-validate, instead of leaving the window with no way forward.

## [0.6.0] - 2026-07-31

**Validation verdicts: CHANGED.** Files that earlier versions accepted may
now be rejected, and files they rejected may now be accepted. Both
directions occur in this release: the Phon `%x` fixes below remove false
rejects, while the removed error codes and the suppression fix change what
`validate` reports and what exit code it returns. Pin with `~` if you depend
on a fixed rule set.

### Added

- **`chatter fix`**, built on the span-splicing engine. It supersedes the
  deleted `chatter lint`: the old `lint --fix` is now `fix --apply`. `fix`
  covers the full fix catalog rather than three codes, applies fixes at
  exact byte spans validated against the source text, and repairs a clean
  utterance in a file whose other regions did not parse (the utterance
  containing an edit must have parsed clean, or the edit is refused and
  reported, never silently dropped). Every catalog entry carries a
  batch-safety tier and a bare `--apply` writes only the mechanical ones;
  a semantic fix is written only when its code is named with `--code`; an
  ambiguous fix is only ever reported, never written by this command.

### Removed

- **`chatter lint` (the `--fix` auto-fixer) is deleted.** It was a live
  span-driven byte writer built before the splice engine's safety
  guarantees existed: it read `error.location.span` with no dummy-span
  guard (`Span::DUMMY` is `{0,0}`, a real file offset), called
  `String::replace_range` with no `is_char_boundary` check (a panic on a
  non-character-boundary span), inserted its E301 terminator fix at zero
  width with no dummy-span guard either (corrupting the `@UTF8` header had
  one ever fired at offset 0), and detected no overlap between fixes. An
  audit found zero production callers (no `talkbank-tools` reference, no
  workspace script, no IISRP pipeline usage; only its own tests and the
  book mentioned it). `chatter fix` is its successor; see Added above.

- **Five `ErrorCode` variants**, each unreachable or redundant:
  `LongFeatureLabelMismatch`, `NonvocalLabelMismatch`, `UnexpectedTierNode`,
  `UnexpectedMorphologyNode`, and `LegacyWarning` (with its generated spec
  entry). Consumers matching on `ErrorCode` will see these gone.

- **Twelve of the language server's twenty-one quick fixes.** Each was
  attached to a code it did not repair: the action offered for a duplicate
  header inserted a missing one, and eleven others were similarly
  mismatched. The nine that remain repair the diagnostic they are attached
  to. Quick-fix matching now goes through parsed error codes rather than
  string literals, so a renamed code is a compile error instead of a
  silently dead action.

### Fixed

- **Phon `%x` dependent tiers, reconciled against the upstream spec.** Three
  fixes, two of which were false rejects on valid Phon output:
  - `%xphoaln` no longer requires its word count to equal `%mod`/`%pho`
    exactly. The spec allows a pause present on only one of the two tiers to
    consume a word slot only on the tier that contains it, so the counts
    legitimately differ by one.
  - Numeric inter-word pauses (`(1.5)`, `(1:05.2)`) are accepted on the
    syllabification tiers, alongside the three untimed forms. They were
    rejected on the grounds of being unattested in available corpora, which
    is not a basis for refusing a construct the spec declares legal.
  - Intra-word pauses (`^`, U+005E) are tokenized rather than absorbed into
    the neighbouring phone. A word-final `^` previously produced a spurious
    error, and a mid-word `^` silently became part of the following phone.
    Reconstruction preserves the pause in place, per the spec's rule that
    stripping each unit's `:CODE` and concatenating must reproduce the
    source word exactly.

- **`validate --suppress` no longer zeroes the invalid count and the exit
  code.** Suppressing a code removed it from the report AND from the
  tallies, so a file with genuine OTHER errors could be counted valid and the
  command could exit 0. A file that still has unsuppressed diagnostics now
  counts invalid and the command exits non-zero, as it should. A file whose
  every diagnostic was suppressed does count valid: that is what asking for
  those codes to be suppressed means.

- **The validation cache key covers every dimension of the verdict**,
  including parse behaviour and the active rule set, as a required
  parameter rather than a hand-picked subset. A cached verdict from one
  configuration could previously be served for another.

- **Strict parsing no longer discards the model it built** on failure, so a
  caller can inspect what parsed alongside the diagnostics.

### Changed

- **Diagnostic classification happens once, from the active rule set**,
  rather than being recomputed at three call sites that could disagree.

- **The diagnostic kind is generated from the spec** instead of mirrored in
  a hand-maintained match, and a divergence between the spec and the
  `ErrorCode` enum now fails the build in both directions rather than
  falling through to a default.

## [0.5.1] - 2026-07-30

### Fixed

- **`validate --force` was unusable at corpus scale** (v0.5.0 DOA): the
  cache refresh called `clear_prefix` once per resolved FILE, and each call
  scanned every `file_path` in the cache, so a corpus-sized invocation did
  quadratic work (on a 136k-file cache, effectively forever) at 100% CPU
  behind a blank screen before the progress display started. The refresh is
  now one batched `DELETE ... IN (...)` pass over the resolved file list,
  and `clear_prefix` itself became a single range-predicate statement
  instead of a scan-and-loop. Pinned by a real-CLI regression test that
  warms a 6,000-file cache and bounds the forced pass (old: 34s at that
  size; new: seconds, dominated by validation itself).

## [0.5.0] - 2026-07-30

### Removed

- **Two ungrounded CA-mode validation exemptions.** `@Options: CA` no longer
  disables the E241 illegal-untranscribed checks, nor the E701/E704 temporal
  checks (which it had skipped wholesale via an early return, while E362
  bullet monotonicity kept running on the same files). Neither skip had a
  CLAN CHECK counterpart: CHECK's whole CA behavior is three suppressed
  errors (21 terminator, 155 parenthesized word, 123 leading space), and
  chatter keeps exactly those three. Measured before removal on ALL 994 kept
  CA-declared files: both gates protected zero occurrences. The temporal
  skip's recorded rationale (leniency-policy Decision 6, false positives on
  CA reference files) no longer reproduces, since the temporal rules gained
  the 500 ms tolerance and per-speaker semantics; its Revisit line
  anticipated this removal. CA files with genuine timing defects or illegal
  untranscribed markers are now diagnosed like any other file.

### Fixed

- **E326 now says when the skipped line looks like a CHAT line pushed off
  column 1.** An indented dependent tier (` %mor:	...`) was reported as
  "Unsupported line skipped", accurate but useless: the reader hunts for junk
  when the fix is deleting one space. The message now names the shape ("looks
  like a dependent tier line pushed off column 1; it must begin at column 1")
  for tier-, main-tier-, and header-shaped lines, with a suggestion to remove
  the leading whitespace.

- **An annotated word's wrapper span was never set**, left `Span::DUMMY` at
  construction while the annotated event, action, and group paths all set a
  real one. Two consequences: any diagnostic located on an annotated word
  pointed at byte zero, and E757 could not see a bracketed code glued to the
  following word (`hello [!]there`) at all, because its detection is span
  adjacency. The wrapper now spans the word through the enclosing node's end,
  covering its trailing `[...]` codes, exactly as the retrace paths do.

### Changed

- **E757 now covers every bracketed code, not only retraces.** `hello [/]x`
  was rejected; `hello [!]x` and `bobo [= toy]x` were silently accepted,
  though they are the same defect and the code's own description already said
  "bracketed code". Juxtaposition-matrix cell 8, ruled REJECT 2026-07-18.
  Mirrored in the re2c front end, where a bare closing bracket joins the
  retrace tokens. The 2026-07-18 matrix scan found `][letter` unattested
  corpus-wide and the differential confirms no new instances, so no kept file
  is affected.

### Added

- **`talkbank-transform` gained a default-on `validation-runner` feature.**
  The corpus-scale validation runner is the crate's only SQL consumer (sqlx,
  via `talkbank-cache`), so it, that dependency, and the runner-only
  `crossbeam-channel`/`num_cpus` now sit behind the feature. Default builds
  are unchanged; a consumer that wants the transform surface without a SQL
  stack opts out with `default-features = false`. The path predicate
  `is_chat_transcript_path` moved to the feature-independent
  `talkbank_transform::paths` (still re-exported from `validation_runner`),
  since the corpus walk and CLI walks need it on every build.

- **E766, a linker placed after utterance content** (`yeah that go +" okay .`).
  Linkers connect an utterance to the previous one, so they are
  utterance-initial by definition; a misplaced one used to surface as generic
  unparsable content (E316), which gave the transcriber nothing to act on.
  The grammar now parses the misplaced linker into the CST (the same
  strict+catch-all pattern as the curly-quote rule) so the diagnostic names
  the construct at the exact token, in both parser front ends. One deliberate
  carve-out: a `++` glued to words on both sides (`un++do`) is a word run
  with an empty compound part and keeps its E233 diagnosis. A side effect of
  the grammar change is finer error recovery on several unparsable-content
  shapes: diagnostics that used to blame a whole line now land on the exact
  offending region (e.g. an unmatched `<` now yields E316 on `<word ` with
  the rest of the utterance parsed normally).

- **E765, a free-standing `:` or `;` separator, or a pause, glued to the item
  after it** (`:and`, `;;`, `(.)dog`). Same family and same span-adjacency
  mechanism as E764; the preceding side stays valid, since `word↘` and `dog,`
  are documented convention and `dog:` fuses into the word.

  Juxtaposition-matrix cell 7 was ruled REJECT for the whole separator class,
  against an estimate of roughly six affected files. The corpus differential
  measured that reading at 270 new instances on a 2%, 2,134-file sample (about
  13,500 corpus-wide), every inspected one legitimate CA notation rather than a
  missing space: `≡` is latching and is written glued on both sides, and the
  intonation arrows attach to the material they mark, including directly before
  an overlap close. Adjudicated UNINTENDED, so the rule ships narrowed to the
  plain punctuation separators and pauses, where the differential is clean.
  Whether any CA mark should forbid trailing glue is left open, with receipts
  in the spec.

- **E764, a `&`-prefixed form glued to the preceding word** (`dog&-um`,
  `dog&~gaga`, `dog&+fr`). The shape parses as two words, because `&` cannot
  continue a word, so a missing space silently manufactures a word boundary
  that the transcriber did not write and nothing reported. Style rule in the
  E749/E751/E757 family, detected by span adjacency, mirrored in the re2c
  front end as a token scan. Glued omission (`dog0is`) is not this code: it
  yields one malformed word and E220 already rejects it.

  Juxtaposition-matrix cell 6, ruled REJECT 2026-07-18; zero main-tier
  attestations in the kept corpus at adoption, so no existing file is
  affected. Validator-only: grammar, model shape, and roundtrip behavior
  are untouched.

## [0.4.1] - 2026-07-27

### Fixed

- **`talkbank_transform::dependent_tiers::replace_or_add_tier` could not be
  called on an utterance.** It still took `SmallVec<[DependentTier; 3]>` after
  `DependentTierEntry` was introduced and `Utterance::dependent_tiers` became
  `SmallVec<[DependentTierEntry; 3]>`, so the one thing the helper exists to do
  no longer type-checked. It shipped in this state in 0.3.6 and 0.4.0.

  It compiled because it was internally consistent, and no test caught it
  because this workspace has no callers of it: the helper is public API for
  downstream consumers, and the only one was pinned to an older release. The
  regression guard added with the fix is a compile-time function taking
  `&mut Utterance`, so any future drift between the signature and the field
  fails the build rather than passing silently.

  On replace, the existing entry's `TierSeparator` is preserved (only the
  payload is regenerated, and the separator is the provenance E758 is detected
  from); on append the new entry is `CLEAN`. Serialization canonicalizes to a
  single tab either way, so this affects diagnostics, not output.

## [0.4.0] - 2026-07-27

### Added

- Three validation rules that catch real, previously-invisible defects in
  transcript data. All three live entirely in the validator: the grammar,
  the model's serialized shape, and roundtrip behavior are untouched.

  - **E761, `%gra` relation head is not a Universal Dependencies
    relation.** A `%gra` label is `HEAD` or `HEAD-SUBTYPE`; UD fixes the
    head set at 37 universal relations and leaves subtypes open and
    language-specific, so the head is checked against that closed set and
    the subtype is never checked. Nothing validated relation labels before,
    in chatter or in CLAN CHECK, so a corrupted label rode silently into
    every downstream analysis that reads the dependency graph. Grounded in
    a survey of the entire corpus (138,565,864 relation instances across
    106,158 files): all 37 universal heads are attested, 150 distinct
    labels occur, and exactly three heads fall outside the set, all of them
    defects (`IOB` for `IOBJ`, `PAD`, `PUNCTT` for `PUNCT`).

  - **E762, the prefix marker `#` stands alone as a word or opens one.**
    The marker attaches to the END of the prefix it marks, and the prefix
    is a word of its own (Hebrew `ha# kelev`), so neither shape can be that
    construct in any language. Language-independent, and zero-attested
    corpus-wide.

  - **E763, prefix marker in a language that does not use it.** Gated on
    the WORD's resolved language rather than the file's `@Languages`
    header, exactly as the digits rule (E220) is, so a code-switched word
    brings its own rules with it. Languages that write the marker: `heb`,
    `ara`. Word-internal markers stay legal wherever the language allows
    the marker at all.

- `TreeSitterParser` now implements the shared `ChatParser` trait
  (`talkbank_model::ChatParser`), making the two parser backends
  interchangeable behind one generic bound at every granularity (file,
  header, utterance, tier, word, relation). Previously only
  `Re2cParser` implemented the trait, so consumers selecting a backend
  per target (tree-sitter natively, pure-Rust re2c on wasm) had to
  hand-roll a cfg-gated facade. Every trait method delegates to the
  matching inherent `parse_*_fragment` method, so trait-path and
  inherent-path behavior are identical; conformance is pinned by
  `talkbank-parser/tests/chat_parser_trait.rs`.

- Dedicated error codes for two malformations that previously fell
  through to the generic E316 unparsable-content catch-all, from the
  CHECK-parity adjudication of CLAN CHECK errors 52 and 11: E759 (an
  utterance beginning with a postfix annotation such as `[/]`, `[<]`,
  or `[: text]`, which has no preceding material to scope over) and
  E760 (a `%mor` item with an empty part-of-speech field, `|we`). Both
  are recognized by the tree-sitter front end's error analysis and
  mirrored in the re2c oracle's front end; both files were already
  rejected, so no validity verdict changes, only the diagnosis.

### Removed

- **TalkBank XML support, in full.** The `to-xml` command, the
  `talkbank_transform::xml` emitter, the `corpus/reference-xml/` golden
  corpus, the `xml_golden` and `xml_schema_validate` suites, the bundled
  `talkbank.xsd`/`xml.xsd` schemas, the XML Emitter book chapter, and the
  `quick-xml` dependency.

  TalkBank stopped generating TalkBank XML on 2025-10-29, when its last
  consumer said he no longer used it, and the published `data-xml/`
  distribution has been offline since. Phon moved off the format some time
  ago. Nothing produced by this emitter had a consumer.

  **Breaking:** `chatter to-xml` no longer exists and there is no
  replacement. Use `chatter to-json`, which is the format the toolchain
  actually maintains. `talkbank_transform::xml::XmlWriteError` is gone from
  the public API surface.

### Changed

- The `%gra` documentation, examples, reference corpus, and error-spec
  fixtures no longer use retired TalkBank relation labels (`SUBJ`, `JCT`,
  `POBJ`, `COM`, `VOC`, `MOD`, `NEG`, `PRED`, `COMP`, `ADV`, `INCROOT`,
  `QUANT`, `LINK`), which E761 now rejects. None of them occurs anywhere in
  the real corpora; they were fixture inventions that would have taught
  readers of the API docs a vocabulary the validator rejects. Replaced
  throughout by the UD relations the corpora actually use (`NSUBJ`, `OBL`,
  `CASE`, `DISCOURSE`, `VOCATIVE`, `AMOD`, `ADVMOD-NEG`, `CCOMP`, `EXPL`,
  `DET`, `DEP`). The `gra_incroot` grammar construct deliberately keeps
  `INCROOT`: it pins the property that relation labels are open text at the
  grammar layer, which is why the vocabulary is a validation policy and not
  a syntax.

### Fixed

- **Word validation now reaches words nested inside groups.** Main-tier
  validation iterated content items flatly and matched only `Word`,
  `AnnotatedWord` and `ReplacedWord`, with a catch-all that silently
  discarded every container, so a word inside a retrace, a reformulation,
  an angle group or a quotation was never word-validated at all.

  The symptom: the identical token was rejected outside a group and
  accepted inside one. In English `hello3 dog .` was invalid (E220) while
  `hello3 [/] hello dog .` was valid, on every release up to this one.
  Every word-level rule inherited the hole, so E220 has carried it for as
  long as the rule has existed; the newer prefix-marker rules inherited it
  on arrival.

  Corpus impact, measured over all 106,158 files: 341 to 348 invalid files,
  8 new error instances across 7 files (E241 x2, E252 x4, E248 x1,
  E763 x1), each a pre-existing data defect that had been hiding inside a
  group rather than any change in what counts as valid CHAT.


- `ErrorCollector::is_empty()` violated the standard Rust contract
  `len() == 0 <=> is_empty()`: it answered "is the internal buffer
  unallocated?", so a collector created with `with_capacity` (which
  pre-allocates) reported non-empty while holding zero errors. Found
  by the 1.0 contract-set API audit; now implemented as `len() == 0`
  with a regression test.

- `TreeSitterParser::parse_gra_relation_fragment` (and the trait's
  `parse_gra_relation`) rejected EVERY bare `%gra` relation and leaked
  a spurious E709 diagnostic into the caller's sink, because the
  wrapper appended a scaffold terminator with the never-valid index 0
  (`0|0|PUNCT`) and the tier wrapper rejects on any internal
  diagnostic. The scaffold is now valid CHAT (`2|1|PUNCT`), and a
  scaffold-region filter guarantees diagnostics against wrapper
  scaffolding can never reach the caller. The re2c backend was
  unaffected (it parses the relation directly); the fix restores
  backend agreement. Caught by the new `ChatParser` trait conformance
  test.

- Validation cache: initialization is now concurrency-safe across
  processes, not just threads. Every opener takes an exclusive advisory
  file lock (`talkbank-cache.init.lock`, beside the database) around
  first-time create + migrate, so parallel `chatter` runs (or parallel
  test processes) sharing one cache directory can no longer race sqlx's
  SQLite migration (`UNIQUE constraint failed: _sqlx_migrations.version`,
  the 2026-07-13 flake) or collide on first-connection WAL setup. Lock
  acquisition is bounded: on timeout, opening fails with the new typed
  `CacheError::InitLockTimeout` and the CLI degrades to running
  uncached instead of blocking. The 2026-07-13 bounded retry is
  retained as a backstop for older builds that share the cache
  directory without honoring the lock protocol. Regression coverage:
  a cross-process stress test races 8 processes over a fresh cache
  directory for 4 rounds under a hard deadline, so both failure modes
  (constraint error and hang) fail the suite instead of flaking or
  wedging it.

- Desktop release: the macOS updater bundle is now uploaded under a
  per-arch asset name (`Chatter-<target>.app.tar.gz`). Previously both
  the aarch64 and x86_64 macOS jobs uploaded the arch-independent
  `Chatter.app.tar.gz`, which raced on the shared release asset (the
  v0.3.6 `release-desktop` upload failure) and pointed both darwin
  entries in `latest.json` at a single URL holding one arch's binary.
  Fresh `.dmg` downloads were unaffected; the desktop auto-updater is
  the surface this corrects. (Ships with the next release.)
- Public API: a downstream crate that depends only on `talkbank-parser`
  can now name the error type of its parse methods. The six
  `TreeSitterParser::parse_*` methods return
  `ParseResult<T> = Result<T, ParseErrors>`, but `ParseErrors` /
  `ParseResult` were not reachable from the `talkbank-parser` crate root
  (only via a `pub(crate)` module), forcing consumers to add a separate
  `talkbank-model` dependency or stringify at the boundary; both are now
  re-exported. Also re-exported `talkbank_model::SylWordError` (the error
  of `classify_syl_word` / `tokenize_syl_word`), which was omitted from
  the model root while its sibling phon parse-error types were present.
  Completes the BUG-3 audit: a compile-test now names every public
  fallible constructor's error type so this class cannot regress.

## [0.3.6] - 2026-07-17

### Fixed

- The Phon `%x`-tier content checks (introduced with the %x fold-in)
  no longer mass-flag valid Phon exports. Two wild-corpus conventions
  the original specification never confronted are now accepted:
  (1) pause fillers (`(.)`, `(..)`, `(...)`) mirrored at the same word
  position on `%mod`/`%pho`/`%xmodsyl`/`%xphosyl` (and as pause pairs
  on `%xphoaln`) to keep word-aligned tiers in index lockstep, which
  E735 previously rejected as malformed `phone:CODE` units (roughly
  13,000 spurious errors across the PhonBank corpora); and
  (2) `^` and IPA `.` syllable-boundary notation in `%mod`/`%pho`
  words, which the segment-level `%xphoaln` reconstruction comparison
  now ignores exactly as it already ignored stress markers (roughly
  770 spurious E740/E741). Genuine misalignments (index-shift chains,
  pause fillers standing in for real words) are still reported. Users
  who adopted `--suppress xphon` to silence the storm can remove it
  and regain the genuine `%x`-tier checks.
- Generated error-documentation pages (`docs/errors/`) no longer fuse
  words across wrapped spec lines or drop backticked text: the spec
  text extractor now renders soft line breaks as spaces and includes
  inline code spans.

### Added

- New validation rule E752: timing bullets without an `@Media` header.
  A transcript carrying timing evidence (utterance bullets or `%wor`
  word timing) must declare the media those timestamps index; completes
  the media-consistency family (E544: declared linkage without timing;
  E552: declared `unlinked` contradicted by timing). Mirrors CLAN CHECK
  error 112.
- New validation rule E753: a word consisting only of a repetition
  segment (fully `↫...↫`-wrapped, no stem outside the delimiters) is
  rejected; word-category prefixes (`&-` filler, `&~` nonword, `0`
  omission) count as a stem. Adopted from GUI CLAN CHECK error 151 as
  a chatter-authority rule (the unix CHECK build never enforced it).
- New validation rule E754: the `@l` letter form must carry exactly one
  letter of stem (`b@l`); multi-letter content belongs under `@k` /
  `@ls`. Repeated-segment material (`↫b^↫b@l`) does not count toward
  the stem, matching real CLAN CHECK behavior. Mirrors CLAN CHECK
  error 76.
- New validation rule E755: a `[- CODE]` utterance-level language must
  be declared in `@Languages` (utterance-level presence is
  substantial). Mirrors CLAN CHECK error 152.

- Word-level explicit language codes (`word@s:CODE`) are now validated
  against the ISO 639-3 registry (E519), the same rule that guards
  `@Languages` and `@ID`; declaration in `@Languages` remains not
  required.

- `@L1 of` values are now typed ISO 639-3 language codes and validated
  against the registry (E519), completing registry validation at every
  position language codes appear. Wild usage was already uniformly
  codes; generation via `build_chat` now takes a `LanguageCode` for the
  participant first language.

- E756 (empty user-defined `%x` tier) replaces W601: the rejection is
  unchanged; the old code fired as a hard error despite its warning
  prefix, so the number was the bug. The diagnostic message also no
  longer double-prefixes the tier name (`%xfoo`, not `%xxfoo`).

### Removed

- The E254 warning (word-level `@s:CODE` not listed in `@Languages`)
  is retired: an explicit word-level language code is self-contained
  and deliberately carries no declaration requirement. `@Languages`
  declares the transcript's substantial languages; a one-word
  insertion is not substantial presence. (This matches CLAN CHECK,
  which dropped its own `@s` declaration requirement in 2019.)

<!--
Deferred to a later release:
- Word-content validity: reject junk inside words (`|`, ideographic comma,
  mojibake, ...) per the curated word-segment allowlist. Pending adjudication.
- CHECK-parity endgame closes (48 illegal `|`, 76 single-letter `@l`) and the
  remaining per-rule decisions.
-->


## [0.3.5] - 2026-07-15

Emergency release restoring corpus-correct word parsing. Versions
0.3.3 and 0.3.4 have been YANKED (releases and tags removed).

### Fixed

- Reverted the whitespace-boundary overlap-custody grammar introduced
  in 0.3.3. Its GLR-arbitrated word readings fragmented words carrying
  four or more glued markers (for example multi-syllable-pause chains
  like `or^ga^ni^zi^ra`), causing spurious E252/E331/E600/E705
  validation errors across real corpora and, worse, a serialization
  mutation (a space inserted into such words on rewrite). Word parsing
  is restored to the 0.3.2 grammar, verified by an error-code
  differential and a roundtrip comparison against the 0.3.2 binary
  over a corpus sample: identical profiles.
- A regression test pins that multi-marker words parse as one word and
  validate cleanly.

### Retained from the yanked releases

- Typed `@u` phonetic word forms (UNIBET).
- The `build_chat` header emitters and @ID demographics fix.
- The shared English capitalization transform.
- The long-tier stack-overflow fix and its regression test.
- The SQLite cache concurrency-safety fix; CI runs under nextest.

## [0.3.4] - 2026-07-15 [YANKED]

### Added

- **`@u` phonetic forms are now typed phonetic content.** A `@u` word
  (a UNIBET/IPA phonetic transcription standing in a word slot, e.g.
  the spoken side of an aphasia `[: target]` replacement) now models
  its content as a dedicated `WordContent::Phonetic(WordPhonetic)`
  node instead of orthographic text, in both parsers. Orthographic
  word-hygiene rules structurally cannot apply to phonetic content;
  the phonetic string itself stays deliberately lenient (IPA, ASCII
  UNIBET, X-SAMPA), matching the `%pho` tier's stance. `to-json`
  emits `{"type": "phonetic", ...}` for these nodes (schema updated);
  `cleaned_text` remains the phonetic string verbatim; the sanitizer
  redacts phonetic forms like spoken text. Scope is `@u` only;
  sibling special forms remain orthographic words.

- **`build_chat` now emits the full standard header set.** The general
  CHAT-generation schema (`TranscriptDescription` / `ParticipantDesc`)
  gained typed optional fields for `@Date`, `@Situation`, `@Options`,
  `@Transcriber`, `@Comment`, per-speaker `@L1 of`, and `@PID`
  (preserved from a source, never minted), each emitted in canonical
  header order. `@ID` demographics (age, sex, group, SES, education,
  custom) are now carried through `ParticipantDesc` instead of being
  silently dropped, fixing empty demographic slots in generated `@ID`
  headers.
- **Shared English capitalization transform**
  (`talkbank_transform::capitalize`): capitalizes the pronoun "I"
  family and the first real word of each utterance on the typed model,
  for generators whose sources are all-lowercase (improves downstream
  `%mor` accuracy). Token-level helpers are public for generators that
  capitalize their own word representation.

### Fixed

- **`chatter validate` no longer headlines a warnings-only file as an
  error.** A file whose findings are all warnings (which is valid CHAT,
  and was already counted valid in the summary) now prints
  `⚠ Warnings in <file>` instead of the contradictory
  `✗ Errors found in <file>`, and the "fix structural errors first"
  hint fires only on hard errors. Presentation only; validation logic
  unchanged.
- **The validation cache no longer fails to initialize when opened
  concurrently.** Two `chatter` runs sharing a cache directory (or a
  multi-threaded consumer) could race the one-time SQLite setup and hit
  `UNIQUE constraint failed: _sqlx_migrations.version` or a WAL init
  collision, silently disabling caching for that run. Concurrent opens on a
  fresh cache directory now retry the transient init race and all succeed.

## [0.3.3] - 2026-07-13 [YANKED]

### Added

- **Desktop app: a "Check for Updates..." menu item and a periodic background
  update check.** The app previously checked for a new release only at launch,
  so an app that was rarely relaunched could sit far behind. It now also checks
  every six hours in the background, and the app menu has a manual "Check for
  Updates..." item that reports when you are already up to date.
- **Desktop app: a real "About Chatter" panel** with the version, a short
  description, and clickable links to the TalkBank site and the source
  repository, replacing the bare version-only default.
- **`talkbank_transform::build_chat`: assemble a validated CHAT file from a
  typed transcript description.** Given participants, optional media, and
  utterances as pre-formatted CHAT main-tier text (`TranscriptDescription`),
  it synthesizes the header block, parses each utterance through the
  tree-sitter parser, and returns a `ChatFile`. The description carries a
  `media_status`, so a transcript that names its media but has no timing
  bullets yet (pre-forced-alignment) can emit `@Media: <id>, audio, unlinked`
  and stay valid instead of falsely claiming linkage (E544).
- **`talkbank_transform::num_words::expand_number`: spell digit tokens as
  language-appropriate number words** (13 lookup-table languages, CJK, and
  English ordinals/decades), so generated CHAT satisfies E220 (numeric digits
  are not allowed in words for languages that do not permit them).

### Changed

- **Overlap custody now follows whitespace boundaries, with canonical overlap
  serialization.** Overlap markers bind to the token on the correct side of a
  whitespace boundary, and serialization emits a single canonical form.
- **tree-sitter updated to 0.26.11** across the workspace (CLI, grammar
  bindings, and the generated parser).

### Fixed

- **Long dependent-tier reconstruction is now linear-time.** A quadratic blowup
  on very long utterance tiers is eliminated; pathological inputs that
  previously stalled the parser now reconstruct in linear time.
- **Desktop app: the validation settings popover no longer opens hidden behind
  the results panel.** It was rendered below the panels in the stacking order;
  it now sits above them.
- **Desktop app: the "up to date" dialog now dismisses on the first OK.** A
  listener leak (an async menu subscription whose cleanup could run before it
  resolved) let duplicate listeners accumulate, so one menu click stacked
  several identical dialogs.

## [0.3.2] - 2026-07-10

### Added

- **`chatter rediarize`: repair speaker attribution from external
  diarization turns.** Takes a transcript whose utterance timing is
  trusted but whose speaker labels are not, plus a speaker-turns JSON
  file (`{"source": ..., "turns": [{"track", "start_ms", "end_ms"}]}`)
  from an external diarizer, and re-attributes each timed utterance to
  the dominant overlapping turn. Utterances with no turn coverage are
  flagged, never guessed. Reconciled `@ID` rows are inserted in the
  header block. `--summary-json` emits a machine-readable outcome
  summary (per-utterance reattributions and flag reasons) for
  downstream tooling.
- **Four validation rules for constructs that do not make sense**,
  each adjudicated against real CLAN CHECK behavior and the wild
  corpus: E748 leading-zero media-bullet times; E749 comma glued to
  the following word; E750 whitespace inside angle-group delimiters;
  E751 pause marker glued to a word.

### Fixed

- The re2c oracle lexer now tokenizes short-form parenthesized
  material the same way the canonical parser does (its catch-all
  previously swallowed a trailing delimiter), keeping the two
  independent parsers in cross-check agreement on the new spacing
  rules.

### Changed

- Rust toolchain pin bumped to 1.97.0 (CI workflow pins synced);
  workspace and spec lockfiles refreshed; desktop dependency bumps
  (jsonschema 0.47, TypeScript 7).
- Documentation: an architecture page on overlap-marker binding (why
  edge-adjacent overlap markers bind into words, the ideal top-level
  model, and the conversion-layer path); the grammar's empty-`extras`
  (all-whitespace-explicit) design rationale is now recorded at the
  declaration site.

## [0.3.1] - 2026-07-08

### Fixed

- **Every public fallible constructor's error type is now publicly
  nameable.** `LanguageCodeError` (from `LanguageCode::new`),
  `XphointParseError`, and `PhoalnParseError` were not re-exported, so
  downstream crates could not store them in typed `#[source]` fields and
  had to stringify at the boundary; found by the first real downstream
  consumption of the 0.3.0 API. A new API-surface guard test pins the
  contract so a constructor error type can never silently become
  unnameable again.

## [0.3.0] - 2026-07-07

### Added

- **`--llm-cache <file>` (env `CHATTER_LLM_CACHE`) for holistic speaker-id
  judgment.** A persistent, write-through JSON response cache for
  `speaker-id` / `pipeline` / `batch --judgment holistic`: an identical
  request (same endpoint, model, and rendered prompt) is served from the
  cache instead of making another LLM call, so re-running a batch after a
  crash or an unrelated code change does not re-pay completed sessions.
  Absent flag and env variable means uncached, unchanged from before.

### Fixed

- **`chatter batch` no longer reports holistic suggestions as merges.** In
  holistic-judgment mode the per-session pipeline exits 0 after writing a
  suggestion to the pending file without merging (the operator adjudicates
  first); the batch summary counted those as "merged" and reported zero
  pending work. Outcomes are now classified by whether the merged output
  actually exists, and the summary separately counts merges, suggestions
  awaiting adjudication, and low-confidence refusals awaiting adjudication.
- **E552 (`@Media` says `unlinked` but timing exists) now says where the
  timing was found and how to fix it.** When the only timing evidence is
  word-level bullets inside a `%wor` tier (invisible in normal display), the
  message names the `%wor` tier and offers both remedies (the media is in
  fact aligned: remove `unlinked`; or the `%wor` tier is stale: remove it)
  instead of asserting the media is linked and pointing at bullets the user
  cannot see. The main-tier-bullet case keeps its direct advice.
- **Chatter Desktop's single-file validation now shares the CLI's validation
  engine.** Previously, validating a single `.cha` file in the desktop app
  (as opposed to its parent folder) bypassed the on-disk cache entirely,
  skipped the `@Media`-filename check (E531), and could not honor
  `--roundtrip` / `--parser` / `--strict-linkers`. All of these now work
  identically to `chatter validate` and to the desktop's own folder
  validation, and a new **Settings** panel exposes the equivalent options.
- **Chatter Desktop no longer shows "N files, all valid" before a run has
  actually finished.** The file tree previously derived this message from
  the partial, still-streaming result set, so it could flash "all valid"
  mid-run whenever no error had streamed in yet.

## [0.2.1] - 2026-06-24

### Added

- **The `talkbank-lsp` language server now ships as a standalone release
  artifact.** Prebuilt, code-signed `talkbank-lsp` binaries for macOS (Apple
  Silicon and Intel), Linux (x86_64 and aarch64, static musl), and Windows are
  attached to the GitHub Release, each with its own `talkbank-lsp-installer.sh`
  / `talkbank-lsp-installer.ps1`. Any LSP-aware editor can now install the server
  without building it from source; it is a first-class artifact in its own right,
  not only the binary the VS Code extension bundles per platform.

## [0.2.0] - 2026-06-23

### Added

- **More of CLAN CHECK's invalidity is now enforced.** A batch of CHECK-parity
  rules was implemented so `chatter validate` rejects more invalid CHAT:
  - `E514`: an `@ID` line's corpus field is required (CHECK 63).
  - `E547`: a constant participant header must follow the `@ID` block.
  - `E548`: closes the case CHECK 126 covers.
  - `E549`: a speaker may not be declared twice (CHECK 13).
  - Duplicate `@ID` lines and out-of-order `@Options` fields (CHECK 13, 125).
  - A dependent tier used without being declared (CHECK 17).
  - An out-of-range `@Time Duration` (CHECK 35).
  - An `@Media` header marked unlinked while the transcript still carries timing
    bullets (CHECK 124), and an `@Media` filename that does not match the data
    file (CHECK 157).
  - A replacement `[: ...]` now requires a preceding space (CHECK 161).
  - Tree-sitter recovery nodes are surfaced as invalidity rather than silently
    repaired: a surviving `ERROR` node maps to `E316` and a `MISSING` node to
    `E342` (with the re2c oracle mirroring it), covering a group with no
    annotation and swallowed recovery nodes inside comma-list headers
    (CHECK 5/6/106/108).
- **Phon:** `U` (unknown) is accepted as a legal syllable-constituent code on the
  `%xmodsyl` and `%xphosyl` tiers.
- A formal behavioral CHECK-validity parity test suite that runs real CLAN CHECK
  and chatter on the same fixtures and fails if either side drifts.

### Changed

- **`chatter update` now self-updates in process.** It embeds the axoupdater
  self-updater as a library, reads the cargo-dist install receipt (keyed by the
  package name), and replaces the running binary from GitHub Releases. This
  removes the package-name coupling that previously made `chatter update` report
  "not installed" on a correctly installed binary.
- **The CLI package is renamed `talkbank-cli` to `chatter`** (the crate now lives
  at `crates/chatter/`). The generated install scripts are therefore
  `chatter-installer.sh` and `chatter-installer.ps1` (previously
  `talkbank-cli-installer.*`); update any pinned install URL accordingly. The
  binary is still `chatter`, and the library/API crates keep their `talkbank-*`
  names.
- **Validation is stricter.** Because of the new CHECK-parity rules above, some
  files that passed `chatter validate` under 0.1.1 may now report errors. This is
  intended: chatter is the CHAT-validity authority and is at least as strict as
  CLAN CHECK.

- Word-level explicit language codes (`word@s:CODE`) are now validated
  against the ISO 639-3 registry (E519), the same rule that guards
  `@Languages` and `@ID`; declaration in `@Languages` remains not
  required.

### Removed

- The standalone self-updater binary (cargo-dist `install-updater = false`). The
  `chatter update` subcommand is unchanged for users; it now updates in process
  instead of shelling out to a separate program.

### Fixed

- The recovery-node invalidity backstop is scoped to localized errors so it does
  not over-flag, and several malformed `@ID` test fixtures were corrected.
- Hardened the CHECK-parity audit and corrected a CHECK 126 verdict it had
  falsely certified; the curated CHECK error-code map is restored in place of a
  brittle keyword heuristic.

## [0.1.1] - 2026-06-22

### Fixed

- **Validation cache could serve a stale verdict across rule-set changes.**
  `chatter validate` keyed its result cache on the cache crate's package
  version, which does not change when validation rules change, so a "Valid"
  result cached before a new rule (such as a retrace-marker check) existed kept
  being served, while a fresh conversion of the same bytes correctly rejected
  them. The cache key now folds in a fingerprint over every error-code rule, so
  adding, removing, or renaming any rule invalidates stale entries; the cache
  is kept and still functions, only keyed correctly.
- CLI usage lines pin the binary name to `chatter` regardless of the invoked
  path (clap `bin_name`).
- The book renders Mermaid diagrams again (restored mdbook-mermaid assets).
- **Desktop app version is now locked to the release version.** The desktop
  bundle (`.dmg` / `.exe` / `.deb`) and the Tauri auto-updater manifest now report
  the same version as the CLI. A version-sync gate (`scripts/sync-app-version.py`,
  enforced in CI and at release time) keeps `tauri.conf.json`, `package.json`, the
  workspace version, and this changelog from drifting, so the updater can never
  again advertise a version the installed bundle does not match.

### Changed

- CI book toolchain bumped to mdBook 0.5.3 and mdbook-mermaid 0.17.0.
- Build: force `serialize-javascript >= 7.0.5` to clear advisories, and bump
  `rand` in the spec crate.
- Docs: the book intro is de-staged for the public release (download-first).

## [0.1.0] - 2026-06-15

First public release.

### Added

- **CHAT-format core.** A strict, incremental tree-sitter parser
  (`talkbank-parser`) with an independent re2c oracle parser
  (`talkbank-parser-re2c`) that cross-checks it on every file; a typed
  CHAT data model with structured validation, error codes, and tier
  alignment (`talkbank-model`); and CHAT-to-JSON / JSON-to-CHAT / XML
  conversion, normalization, transcript-merge, and redaction pipelines
  (`talkbank-transform`).
- **Phon extension tiers.** The four Phon `%x` dependent tiers
  (`%xmodsyl`, `%xphosyl`, `%xphoaln`, `%xphoint`) are parsed and
  validated as first-class CHAT tiers, on by default (pass
  `--suppress xphon` to opt out): syllabification constituent codes and
  phone-vs-source reconstruction, model-to-actual phone alignment, and
  per-phone time intervals, with dedicated error codes.
- **`chatter` CLI.** `validate`, `normalize`, `to-json` / `from-json` /
  `to-xml`, `merge`, `speaker-id`, `batch`, `pipeline`, `adjudicate`,
  `sanity-scan`, `lint`, `clean`, `watch`, `new-file`, `show-alignment`,
  `validate-utseg`, `schema`, `update`, and a content cache.
- **Language server** (`talkbank-lsp`): real-time validation, hover,
  go-to-definition, and cross-tier alignment for any LSP-aware editor.
- **Desktop app** (`Chatter`): a Tauri-based CHAT validation app, shipping
  in the coordinated release alongside the CLI.
- **Auto-update.** The `chatter` CLI self-updates with `chatter update`
  (the bundled cargo-dist / axoupdater self-updater), and the desktop app
  checks for and installs new releases on launch (Tauri updater). Both pull
  from GitHub Releases. The CLI self-updater is experimental.
- **Prebuilt binaries** for macOS (Apple Silicon and Intel), Linux, and
  Windows, plus desktop installers, attached to the GitHub Release. The
  macOS desktop `.dmg` is signed and notarized.

### Known limitations

- **The merge and adjudication surface is experimental.** `merge`,
  `adjudicate`, `speaker-id`, and `sanity-scan` work, but their
  interfaces and heuristics may change before 1.0.
- **Windows binaries are not code-signed yet**, so Windows SmartScreen
  warns on first run (choose "More info" then "Run anyway"). macOS CLI
  binaries are codesigned but not notarized; install via the release
  installer script to avoid the Gatekeeper quarantine prompt.
- **Not on crates.io yet.** crates.io publication is deferred.

[Unreleased]: https://github.com/TalkBank/chatter/compare/v0.13.0...HEAD
[0.13.0]: https://github.com/TalkBank/chatter/compare/v0.12.0...v0.13.0
[0.12.0]: https://github.com/TalkBank/chatter/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/TalkBank/chatter/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/TalkBank/chatter/compare/v0.9.1...v0.10.0
[0.9.1]: https://github.com/TalkBank/chatter/compare/v0.9.0...v0.9.1
[0.9.0]: https://github.com/TalkBank/chatter/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/TalkBank/chatter/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/TalkBank/chatter/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/TalkBank/chatter/compare/v0.5.1...v0.6.0
[0.5.1]: https://github.com/TalkBank/chatter/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/TalkBank/chatter/compare/v0.4.1...v0.5.0
[0.4.1]: https://github.com/TalkBank/chatter/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/TalkBank/chatter/compare/v0.3.6...v0.4.0
[0.3.6]: https://github.com/TalkBank/chatter/compare/v0.3.5...v0.3.6
[0.3.5]: https://github.com/TalkBank/chatter/compare/v0.3.4...v0.3.5
[0.3.4]: https://github.com/TalkBank/chatter/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/TalkBank/chatter/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/TalkBank/chatter/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/TalkBank/chatter/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/TalkBank/chatter/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/TalkBank/chatter/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/TalkBank/chatter/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/TalkBank/chatter/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/TalkBank/chatter/releases/tag/v0.1.0
