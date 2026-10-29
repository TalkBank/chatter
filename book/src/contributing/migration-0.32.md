# Migrating to Chatter 0.32.0

**Last updated:** {{git-dates:page}}

This pre-1.0 release changes CLI names, Rust APIs and selected JSON/RPC
representations. Update dependent code and regenerate model JSON from its
source where practical. Recovery-bearing JSON remains subject to validation;
a successful deserialization does not establish valid CHAT.

This page describes the published 0.32 line. For changes after 0.32.1,
continue with the [0.33 migration guide](migration-0.33.md).

## CLI users and automation

Use the [CLI migration table](../chatter/user-guide/cli-reference.md#migration)
for command-by-command replacements. Old names have no aliases. Frequent tasks
use `validate`, `fix`, `format`, `convert`, `new`, `inspect` and `checks`;
specialist operations live under `experimental`. `commands list/show --format
json` describes the executable tree and its effects. `--log-format` controls
logging, not command-result encoding.

Repair, cache clearing and in-place transforms preview unless `--apply` is
specified. Semantic repair still needs explicit code selection. Validation
refresh uses `--refresh-cache`; directory export regeneration uses `--rebuild`.
Mapping, matching, replay and LLM suggestions have separate argument shapes.
Replay needs a recorded decision file and session identity; review needs an
operator identity. Scripted review refuses unused decisions. Speaker scans
require all selected transcripts and never treat missing input as an empty scan.

Output failures return nonzero through owned cleanup. Forced validation
interruption returns 130. JSON validation writes no stderr on an output failure.
Review produces files with no stdout result; scan summaries use stdout.

Source replacement (`format --output` over an existing regular destination,
`fix --apply`, and in-place transforms) uses explicit macOS/Linux/Windows
metadata policies. It refuses symlinks, read-only or multiply-linked files and
unsupported metadata, and checks the planned source before in-place publication.
Separate conversion/redaction/speaker artifacts and validation reports retain
source-alias checks and stage regular outputs. Their existing symlinks select
the target; hard-link siblings retain old bytes. Artifacts preserve ordinary
permissions, not the stricter source metadata contract. Streams cannot roll
back; a multi-file command is not a filesystem transaction. Audit publication
also requires complete run accounting. Directory conversion retains its
explicit stale-output and prune policies.

Windows replacement uses handle-based POSIX rename semantics so retained source
identity handles remain valid while the new directory entry becomes visible.
Filesystems or Windows versions that do not support that operation return an
error without a delete/copy fallback. The staged file retains its admitted
metadata; existing readers and other hard links retain the old bytes.

The following sections retain the detailed library/protocol migrations from
the development changelog, grouped by consumer. Intermediate implementation
states superseded before release are omitted.

## Shared model and JSON consumers

- **Breaking (participant API/JSON):** removed `ChatFile.participants` and
  `with_participants`; `with_line_map` now takes only lines and the index.
  Participant accessors return borrowed `ParticipantView` values derived from
  current headers, so header edits and legacy JSON cannot restore stale ages,
  names or birth dates. Use `code()`, `id()` and `birth_date()` for view fields,
  or `to_owned_participant()` for a detached snapshot. The shared join retains
  its diagnostics; JSON and schema no longer contain the root metadata cache.

- **Breaking (shared file API/JSON):** removed the duplicated `ChatFile.options`
  and `ChatFile.media` fields. `option_flags()` and `media_header()` borrow the
  current header lines, so edits and JSON import cannot leave pipeline policy
  or media readers using stale copies. JSON and schema contain only the header
  representations. Edit the typed header lines to change either value.

- **Breaking (participant model and wire format):** `ParticipantEntry` and
  `Participant` replace independent name/role fields with tagged
  `ParticipantDetails::Complete { name, role }` or `MissingRole {}`. Use
  `name()` / `role()` accessors; declared roles are optional. Both parsers
  retain role-less speaker declarations without borrowing ID roles, and E513
  survives JSON import without false undeclared/orphan-speaker cascades.
  Contradictory or absent details are rejected during deserialization. LSP
  speaker name/role fields use null for absence. Merge role comparisons refuse
  missing roles. Native participant parsing now returns shared entries;
  `ParticipantsHeaderParsed` and `ParticipantEntryParsed` are removed.
  Structurally damaged declarations retain `UnparsedParticipants` source and
  cannot contribute fabricated identities or ID-join cascades. The shared
  `participant_declaration()` API rejects absent, unparsed or repeated headers;
  accessors and metadata joins no longer select the first repeated declaration,
  and merge refuses unavailable declarations. `apply_mapping_chat` and
  `rediarize` now return Results and refuse absent, rejected or repeated
  declarations before rewriting speakers. Pipeline/speaker-ID error types and
  recorded failure kinds distinguish declaration refusal. Sanitization refuses
  raw participant headers whose name boundaries are unknown.

- **Breaking (validation context):** `SharedValidationData::participant_ids`
  is replaced by typed `participant_roster` evidence. File validation derives
  speaker membership from declarations, independently of successful ID joins;
  missing metadata no longer makes a declared speaker appear undeclared.
  Unscoped and repeated declarations cannot establish negative membership.
  The shared participant join is now a borrowed plan: validation reports its
  diagnostics without rebuilding metadata, retaining missing-ID errors after
  JSON import. Birth-header membership uses declarations rather than ID joins.

- **Breaking (model and language transforms):** malformed `@Languages` headers
  retain `UnparsedLanguages` source through CHAT and JSON. `ChatFile.languages`
  and its independently serialized cache are removed; use
  `language_declaration()` to distinguish an admitted list from absent, empty,
  rejected or repeated declarations. Validation retains that distinction and
  withholds dependent membership/default conclusions. `fix-s` returns a
  declaration error and retagging has a declaration-refusal variant; neither
  partially rewrites a file with an unavailable declaration. Validation context
  declaration data and its alternate/tertiary-language queries now return
  explicit declaration errors. Merge admission uses the same shared error type.
  Native file parsing carries admitted shared headers in `HeaderParsed::Shared`.

- **Breaking (language declaration evidence):** successful file/context
  declarations use `DeclaredLanguagesRef`/`DeclaredLanguages`, whose primary
  code is structurally required. Empty builder input records `Empty` rather
  than successful empty evidence. Declaration-aware resolution and extraction
  APIs consume the admitted view; raw-list convenience APIs admit at entry.
  Ordered code-array serialization and individual-code recovery are unchanged.

- **Breaking (language resolution API):** `LanguageResolution` and
  `WordLanguages` mixed/ambiguous variants carry checked `MultipleLanguageCodes`
  rather than a bare vector.
  Use `TryFrom<Vec<LanguageCode>>` and `as_slice()`. Resolved JSON rejects lists
  shorter than two codes; recovery markers still import them unchanged, report
  E212 and resolve to `Unresolved` instead of silently succeeding.

- **Breaking:** replace recovery-capable `NonEmptyString` model fields with explicit
  `RecoveryText::{NonEmpty, Empty}`. `NonEmptyString` now enforces non-emptiness
  through serde as well as constructors. JSON strings and contextual empty-text
  diagnostics are preserved; Rust consumers must distinguish recovery from
  admitted text.

- **Breaking:** remove `Word::simple`, `Word::new_unchecked`,
  `WordText::new_unchecked`, and `NonEmptyString::new_unchecked`. Construct
  plain words with `Word::new(location, WordText::new(text)?)`, or convert
  `non_empty_literal!` output to `WordText`. Parse authored CHAT to retain its
  structure. JSON recovery admission is unchanged and still needs validation.

- **Breaking:** `Word::replace_simple_text` now requires `WordText` instead of
  a raw string. Paired WOR redaction preserves admitted nonempty display text
  through `CountMatchedWorTimings::main_display_texts`, removing unchecked
  replacement construction. JSON recovery remains subject to validation.

- **Breaking:** remove `WordPhonetic::new_unchecked` and
  `WordShortening::new_unchecked`. Use their checked `new`/`TryFrom`
  constructors or conversion from `NonEmptyString`. Lenient JSON admission
  remains subject to model validation.

- **Breaking:** `NonEmptyString::from_checked_literal` requires validated
  `NonEmptyLiteral` evidence. Direct callers can no longer bypass the literal
  macro's non-empty check by supplying a raw string.

- Add `Word::from_content` for direct structured-word construction. Native lowering
  no longer computes a second cleaned-text projection or creates an unchecked
  temporary word before replacing its content.

- WOR display generation retains checked borrowed text from slot admission,
  removing unchecked word construction. Add `NonEmptyStr` for non-empty borrowed
  text and its allocation-only transition to owned text.

- **Breaking:** remove `MorWord::l2_placeholder()`,
  `MorWord::reset_to_l2_placeholder()` and `ChatCleanedText::stanza_placeholder()`.
  Unavailable analysis and provider transport substitutions must have explicit
  downstream states rather than fabricate morphology or cleaned source text.

- **Breaking:** remove `LanguageCode::empty()`: absent source data must not
  manufacture the legitimate code `und`. Explicitly supplied `und` is retained.
  Native words now retain shared `WordLanguageMarker` values, removing
  `ParsedLangSuffix`, conversion-time language panics and the native word
  carrier's empty `Default` construction.

- **Breaking (age admission):** `AgeValue` is opaque; construct it from text
  and read numeric values through `components()`. Callers can no longer pair
  unrelated numbers and serialized age text. Spelling validation consumes
  admitted shape evidence; string JSON/CHAT and recovery policy are unchanged.

- **Breaking (MOR feature shape):** `MorFeature::flat` and
  `MorFeature::with_key_value` return `Result`, refusing an equals sign in a
  flat value or key that would change the feature's structure on JSON readback.
  Whole-text parsing still splits at the first equals sign and preserves empty
  fields and additional equals signs in values for ordinary validation.
  The string wire format and existing Debug representation are unchanged.

- **Breaking (GRA label admission):** `GrammaticalRelationType::new` and
  string conversions are fallible; the interned payload is private. Labels
  require an initial ASCII letter followed by ASCII letters, digits or hyphens,
  preserving case, unknown heads and custom subtypes. Constructors, JSON and
  schema enforce this token boundary. `GrammaticalRelation::new` and
  `AttachmentRelation::new` take checked labels; `GrammaticalRelationType::root`
  constructs the canonical root label. Vocabulary and graph validation remain
  separate from lexical admission.

- **Breaking (Types field admission):** `DesignType`, `ActivityType`, and
  `GroupType` admit nonempty ASCII alphanumeric atoms through fallible
  `from_text`/`TryFrom` constructors and JSON. `TypesHeader::new` takes checked
  fields; `TypesHeader::parse` admits three strings. Schemas enforce the same
  alphabet. The native parser rejects additional continued triples instead of
  silently keeping only the first one.

- **Breaking (page-number admission):** `PageNumber::new` returns a result;
  string conversions use `TryFrom`. Construction, JSON and schema require
  nonempty ASCII digits while preserving leading zeros and long identifiers.
  Native header parsing admits this shared type before lowering and refuses
  malformed page bodies instead of storing arbitrary text as a page number.

- **Breaking (timed pause admission):** `PauseDuration::Timed` requires
  `TimedPause::new(text)?`, separating CHAT spelling from the optional numeric
  projection exposed by `projection()`. The existing JSON shape is unchanged,
  but malformed spellings are rejected. `PauseTimedDuration` retains its
  standalone projection behavior. Large valid pauses remain syllable fillers
  and one-sided phonological alignment slots even without a numeric projection.
  Phon pause recognition now requires CHAT spelling too: integer-only or signed
  projection inputs no longer qualify as pause markers.

- **Breaking (time-header API):** `TimeDurationValue` and `TimeStartValue`
  admit text through their constructors and expose no mutable classification or
  components. Use `segments()` and `time()` to read parsed values. Empty headers
  have an absent state rather than fabricated midnight components; their pattern
  predicates return false. JSON strings and optional-empty validation are unchanged.

- **Breaking (timing-tier API):** `TimTier` owns a private classification and
  derived time segments. Use `from_text`/`empty` and the existing read accessors
  instead of constructing or mutating enum variants. Callers can no longer
  label unsupported text as parsed timing or edit its spelling independently
  of the segments. The JSON string representation is unchanged.

- **Breaking (scoped marker labels):** `LongFeatureLabel` and `NonvocalLabel`
  now require checked lexical admission; `new` and `TryFrom` return
  `InvalidScopedLabel`. All five marker constructors require their admitted
  label type. JSON remains a string but rejects empty or malformed labels.
  The symbol registry owns the common alphabet used by both grammars, the model
  and generated schemas. Native structural marker carriers hold shared located
  models, removing raw-label reconstruction during conversion.

- **Breaking (event admission):** `EventType::new` and `TryFrom` admit only
  nonempty descriptions in the registry-owned event character class and return
  `InvalidEventType` on refusal. `Event::new` requires this admitted type;
  unchecked string conversions are removed. JSON keeps its string shape but
  serde and generated schemas reject empty or delimited descriptions. Native
  event and annotation-chain carriers now contain the source-located shared
  `Event`. Sanitization admits its event replacement once before editing.

- **Breaking (interposed speech):** `OtherSpokenEvent` now owns a structured
  `Word` instead of a public raw `text` string. Its constructor takes a word;
  `word()` and `word_mut()` expose that payload. JSON uses `word` and rejects
  the legacy raw-text shape. Both parsers use their ordinary word lowering,
  and model validation checks interposed words at every depth without adding
  them to the main speaker's alignment sequence. `LeafContent<'a>` adds
  `Interposed(&Word)` and no longer implements `Eq`. Native token inspection
  exposes `OtherSpokenPrefix` followed by ordinary word tokens, and native
  `ContentItem::OtherSpokenEvent` carries the shared model directly.

- **Breaking (freecode model and lowering):** `Freecode::text()` exposes only
  admitted code text; the public mutable `text` field is removed. Both parsers
  preserve recognized blank freecodes as invalid source recovery. E316 replaces
  the primary parser's unrelated E306 report, and model validation rejects the
  recovery after JSON import. Sanitization refuses it. Valid `text` JSON remains
  unchanged; raw recovery uses `unparsed`. `parse_freecode` now consumes a
  source-bound typed CST node.

- **Breaking (postcode model):** `Postcode::text()` returns admitted payload
  text or `None` for recovery; the public mutable `text` field is removed.
  Both parsers preserve recognized empty postcodes as explicit source recovery.
  Validation rejects recovery and blank structured payloads after JSON import;
  sanitization refuses them. Valid `{ "text": ... }` JSON is unchanged, while
  retained source uses `{ "unparsed": ... }`.

- **Breaking (structural view):** `AngleRef::annotations` now carries
  `AngleAnnotations::Missing` or `Present` with admitted nonempty annotations.
  Use `as_slice()` for traversal. Model validation rejects retained bare angle
  groups at every depth, including after JSON readback; their CHAT spelling
  and existing wire shape remain unchanged.

- **Breaking (content model):** `OrphanAnnotation` is a distinct variant of
  `UtteranceContent`, `BracketedItem` and the borrowed content walker views.
  Native recovery no longer renders an unattached `[/]`, `[<]` or replacement
  as an authored freecode. Original spelling and invalidity survive JSON;
  sanitization refuses this raw annotation source.

- **Breaking (group locations and structural pauses):** `PhoGroup::new` and
  `SinGroup::new` take a location first, and `GroupRef::location` returns
  `Location` directly. Native group carriers retain delimiters in
  `EnclosedGroup`; their serialized payload shape stays unchanged.
  `LeafContent::Pause` distinguishes pauses from spoken events. E371 checks
  every nested phonological group and reports once at each offending group's
  extent, including pauses nested inside other containers. JSON imports still
  report violations without inventing source positions.

- **Breaking (structural overlap view):** `LeafContent<'a>` now carries
  `OverlapPoint(&OverlapPoint)` separately from generic notation. Main-tier
  validation checks each retained point once, at its own location, including
  nested, word-internal, replacement and interposed points. The redundant
  alignment-region validation pass is removed; pairing no longer collapses
  diagnostics for standalone points or duplicates word-internal findings.

- **Breaking (custom-tier model consolidation):** Removed the unused
  `UserDefinedTier` and suffix-only `UserDefinedTierLabel` types. Construct
  `DependentTier::UserDefined(UserDefinedDependentTier { .. })` using a checked
  full `UserDefinedDependentTierLabel`, such as `"xgesture"` rather than
  `"gesture"`. Parser output and the CHAT/JSON document representation are unchanged.

- **Breaking (custom-tier label admission):** `UserDefinedDependentTier::label`
  now requires `UserDefinedDependentTierLabel::new(text)?`. Construction and
  JSON import reject malformed labels, built-in names and reserved structured
  extension names. Valid labels retain their string wire format.
  `DependentTier::Unsupported` now carries `UnsupportedDependentTier`, preserving
  unsupported input independently of custom-label admission.

- **Breaking (dependent-tier model):** `DependentTier::UnparsedText` preserves
  a rejected text tier's label and complete source. Native whole-file parsing
  no longer silently removes malformed bullet tokens from `%com`, `%x*` and
  other text tiers. Recovery remains invalid after JSON readback; redaction
  drops it, and tier replacement matches its original label.

- Add explicit `Header::UnparsedID` recovery and `RedactError::UnparsedID`.
  Rejected ID fields retain source and invalidity through JSON. Missing-ID
  diagnostics require readable ID evidence; sanitization refuses rejected IDs.

- Main-tier terminal, inline and nested bullet positions now carry the shared
  `BulletState` enum. An occupied position holds parsed `Bullet` timing or an
  explicit `UnparsedBullet`; missing terminal timing remains `None`. This
  changes the Rust payload types and requires explicit numeric projection.
  Parsed JSON keeps its existing shape; rejected JSON carries an `unparsed`
  payload. Both parsers preserve rejected numeric source, model validation
  retains E360 after JSON import, and rejected terminal timing cannot promote
  an earlier inline bullet. Timing-dependent split/join/merge operations refuse
  recovery; rediarization flags rejected terminal timing without reassignment.

- `BulletContentSegment` gains an `UnparsedBullet` variant. Both parsers retain
  rejected numeric bullets in comment headers and structured text tiers instead
  of silently omitting them. Recovery retains the original spelling through
  CHAT and JSON serialization and remains invalid during model validation.
  Exhaustive matches over the public segment enum must handle this new state.

- **Breaking:** `DependentTier::UnparsedWor` retains rejected word-timing
  source in both file parsers. It remains invalid and unusable for timing through
  JSON; replacement updates its slot, while redaction and splitting drop it.
  Native recovery no longer reconstructs and reparses failed WOR content.

- **Breaking:** `DependentTier` adds `UnparsedPho`, `UnparsedMod` and
  `UnparsedSin`. Both parsers retain rejected phonology/sign source in its
  original slot. Recovery remains invalid and alignment-tainted after JSON
  import; redaction/splitting drop it and tier replacement updates its own slot.

- **Breaking:** `DependentTier::UnparsedGra` preserves the original rejected
  GRA tier in file parsing under both backends, including after JSON import.
  Recovery no longer exposes an empty or shortened dependency graph. It remains
  invalid, withdraws alignment trust, is replaced in its original regeneration
  slot, and causes redaction to return `RedactError::UnparsedGrammar`.

- **Breaking:** `UnparsedMorTier` becomes the shared `UnparsedTier` payload;
  `DependentTier` adds `UnparsedPhoaln` and `UnparsedXphoint` variants. MOR's
  recovery variant and serialized fields retain their meanings.

- **Breaking:** `DependentTier::UnparsedMor` explicitly represents unparsed MOR
  source in the shared model and JSON schema. Exhaustive consumers must handle
  recovery. Sanitization refuses this state; regeneration replaces its slot.

- **Breaking:** `WriteChat` implementations now accept `W: ChatWriter` and
  return `Result<(), W::Error>`. Existing `std::fmt::Write` destinations remain
  supported. Owned-string helpers render through an `Infallible` destination
  instead of discarding formatting failures; arbitrary `Display` callbacks are
  excluded from the rendering interface. Phon tier content rendering is shared
  by its `Display` and CHAT serializers.

- **Breaking:** `GraLowering::finish(location)` returns `ParseOutcome<GraTier>`;
  a rejected relation cannot construct a serializable partial graph. Remove
  `GraCompleteness` and `GraLoweringReceipt`. Source-bound Tree-sitter GRA parsing
  returns `Result<ParseOutcome<GraTier>, CstFailure>`; re2c tier construction
  propagates rejection directly. File recovery continues to retain UnparsedGra.

- **Breaking (diagnostic starting lines):** `ErrorContext::line_offset` and
  `with_line_offset` now use `NonZeroUsize`. JSON retains positive integer
  offsets and omitted offsets, but rejects zero instead of silently rendering
  it as line one. Source-backed contexts preserve the same nonzero proof
  through rendering; line-extent overflow remains an error.

## Parser and transform consumers

- **Breaking (single-utterance admission):** `SingleUtterance::admit` consumes
  a `ChatFile` instead of an utterance iterator, preserving document options.
  Single-utterance APIs in both backends retain a complete document's
  `@Options: CA` semantics. Callers no longer need to repeat those options in
  fragment context; bare shorthand still requires explicit CA context.

- **Breaking:** single-utterance parser APIs now reject input containing multiple
  utterances instead of silently returning the first. Both backends share
  cardinality admission; use whole-file parsing for multi-utterance inputs.

- **Breaking (fragment rebasing):** `FragmentSource::rebase` and `rebase_from`
  now return `Result<T, FragmentRebaseError>`, rejecting spans outside the
  admitted caller input instead of shifting or saturating them into document
  coordinates. Both parser backends propagate rejection. `SpanShift`
  implementations must provide the consuming `try_map_spans` method; its derive
  generates it without requiring `Clone`. The existing in-place text-edit
  shifting policy is unchanged. Unrepresentable diagnostic translations report
  an internal failure with the original message and snippet, without clamped labels.

- **Breaking (parse-product fixture extraction):** Removed the panicking
  `ParseProduct::expect_built` method from the production model API. Production
  callers match `Built` and `Unbuildable`, retaining the model and diagnostics.
  Tests can explicitly import `talkbank_parser_tests::test_error::ParseProductTestExt`
  for the former fixture-only behavior. This does not change parsing or validity
  policy.

- **Breaking (Tree-sitter bullet-text lowering API):** `parse_bullet_content`
  and the nine low-level bullet-capable dependent-tier parsers return
  `Result<ParseOutcome<T>, CstFailure>`. Missing content now rejects the
  enclosing body instead of inventing a continuation or an empty payload.
  Source-written continuations and absent optional dependent-tier bodies
  retain their existing meaning. The input-based `ChatParser` API is unchanged.

- Low-level Tree-sitter `parse_wor_tier` returns `Result<ParseOutcome<WorTier>,
  CstFailure>`, distinguishing source rejection from producer failure. The
  `ChatParser` interface is unchanged.

- Optional-validation and streaming-recovery APIs accept either parser through
  `ChatParser`, including named transcript validation. Streaming products retain
  diagnostics independently of the caller's sink. Parser rejection remains a
  parse failure and never becomes a parser-construction error or empty model.

- Checked source/model admission accepts either parser through `ChatParser`.
  `ParseProduct` now belongs to `talkbank-model`; its old parser-crate export
  remains available. Both backends use one product-construction and admission
  path, preserving built models, diagnostics and explicit rejection.

- **Breaking:** `RoundtripFailure::Reparse` distinguishes rejected serialized
  output from serialization errors and text mismatches.

- **Breaking (speaker mapping):** mappings that give retained participants the
  same final code now refuse with `MappingAssignmentError::TargetCollision`
  (CLI exit two), before CHAT output. Unmapped participants occupy their existing
  codes; swaps and reuse of dropped identities remain supported.

- **Breaking (speaker mapping API):** `apply_mapping_chat` returns
  `SpeakerIdError`, preserving input-header refusals as `SpeakerRewrite` and
  reporting rejected replacement fields as `InvalidAssignment`. Speaker mapping
  now admits replacement speaker codes, canonical roles and nonblank names
  without comma/line delimiters before rendering. CLI refusal exits two.

- **Breaking (pre-1.0):** `DiarizationTurn` now requires its checked `new`
  constructor and exposes read-only `track()` / `span()` accessors. Track
  labels must satisfy the same speaker-token constraints as speaker renames.
  Invalid turns-JSON tracks return `TurnsJsonError::InvalidTrack`; the CLI
  exits with code 2 before opening CHAT or summary output destinations.

- Speaker mapping and rediarization now return `SpeakerRewriteError`, distinguishing
  unavailable declarations from unparsed IDs. Merge reports `SpeakerHeaders`
  with input identity; pipeline and speaker-ID errors wrap `SpeakerRewrite`.
  These transforms refuse unreadable ID metadata before constructing output,
  including after JSON import. Recorded input failures add `unparsed_id`.

- Speaker-ID mapping now renames or drops `@Birth of`, `@Birthplace of`, and
  `@L1 of` references with their speaker assignment, preserving retained
  payloads. Header handling is exhaustive so new header kinds require review.

- **Breaking (pre-1.0):** `experimental review scripted` refuses unused trailing
  decisions with exit 2 before writing either document. `run_adjudication` now
  consumes its prompter, and `Prompter` requires a consuming `finish` method.
  `AdjudicationOutcome` exposes resolved sessions through read-only accessors.

- **Breaking (pre-1.0):** Sanitizer word and MOR lemma placeholders now use
  alphabetic suffixes (`worda`, `wordb`, …, `wordaa`; `lemmaa`, …), removing
  numeric placeholders that introduced E220 errors in English output.
  `PlaceholderIndex::first()` and consuming `successor()` replace numeric
  `new(u32)` / `value()`; the index is no longer `Copy` or ordered by Rust
  comparison traits. Its nonempty alphabetic representation has no zero index
  or fixed-width counter overflow. Timing correspondence and redaction policy
  are unchanged; sanitization still does not certify full output validity.

- **Changed:** `experimental transform redact` refuses partial source models whose unedited
  serialization drops or reorders non-whitespace source-line content, before
  opening output or writing stdout. `ParsedSourceChat::sanitize` exposes this
  source-bound transition to library callers; model-only `sanitize` does not
  establish source preservation. The shared loss check handles LF, CRLF and CR.

- Sanitization emits one placeholder per contiguous text or phonetic run,
  preserving structural markers and shortening boundaries. Escaped lexical
  segments no longer concatenate duplicate placeholders that change on a second
  pass. `Word::replace_content` applies a completed rewrite and invalidates its
  derived text cache.

- Dependent-tier replacement uses one model-owned label identity for parsed and
  recovery payloads. Replacing text tiers such as `%com` or `%eng` updates the
  first matching entry instead of appending a duplicate, while preserving its
  source separator and keeping distinct custom labels separate.

## Experimental native parser consumers

- **Breaking (native grammar API):** `parser::main_tier`,
  `parser::dependent_tiers` and `parser::headers` are internal modules. Use
  `Re2cParser`'s `ChatParser` methods or the input-based `parser::parse_*`
  helpers. Raw-token grammar composition could pair a token's original spelling
  with foreign payloads before producing a source-bearing value. Public lexing
  and token inspection remain available; they do not grant parser admission.

- **Breaking (native parser internals):** `parser::classify` is crate-private.
  The obsolete subtoken word-start and unused pause classifiers are removed.
  Public source-input parsers and token inspection remain the supported routes.

- **Breaking (native conversion API):** raw `content_item_to_model` and
  `token_to_terminator` helpers are internal. Public lowering consumes
  parser-owned values through the existing word, tier, header and file
  converters. Use `Re2cParser`/`ChatParser` or input-based parser helpers to
  admit source text; a caller-built token or non-word item does not establish
  agreement between its payload and its source spelling.

- **Breaking (native diagnostic API):** removed the unused native `error`
  module (`ParseDiagnostic`, `ParseResult`, `Severity`, and its span alias).
  Parser diagnostics use `talkbank_model::ErrorSink`, `ParseError` and
  `ParseOutcome`; no native entry point produced the removed types. The empty
  standalone binary is removed: `talkbank-parser-re2c` is a library crate.

- **Breaking (pre-1.0):** Removed the native parser's `chat_lines` library
  module. The logical-line helper belongs to its fixture investigation tools
  and now lives in the integration-test target; production parsing is unchanged.

- **Breaking (pre-1.0):** Removed the documentation-hidden native parser
  `tests_support` module. Fixture loading now belongs solely to integration
  tests, whose nonempty fixture type removes empty-input skip paths.

- **Breaking (pre-1.0):** Native `SourceText::text_at` is replaced by fallible
  `slice(range)`, returning a `SourceSlice` that retains its source, text and
  span. Reversed, out-of-bounds and split-UTF-8 ranges are refused. Dependent-tier
  conversion now accepts that source-bound slice instead of separate extent
  and source arguments; file-region consumers reuse admitted bytes.

- **Breaking:** native lexer `LexResult` retains its original source and exposes
  tokens through `tokens()`. `error_report()` no longer accepts a source argument,
  preventing mismatched sources or caller-edited ranges from panicking during
  reporting.

- **Breaking (native lexer API):** `Lexer::new`, `lex_line` and the `COND_*`
  constants use `lexer::StartCondition` instead of numeric DFA states. Lexer
  source, cursor, marker and tag fields are private. Callers choose a supported
  starting context; only the lexer may advance its internal state.

- **Breaking (pre-1.0):** Native lexer error records expose `token()`, `span()`
  and `context()` accessors instead of mutable public fields. Error classification
  and report context now share one owner, removing an unknown-context fallback.

- **Breaking (native header/file API):** `Line::Header` holds a `LocatedHeader`
  with private syntax, source and separator fields. `header_to_model` takes
  that owner without a separate source; `header_parsed_to_model` and
  `HeaderProvenance` are removed. Native `ChatFile` exposes `lines()`,
  `into_lines()` and `source()` instead of mutable fields. Native AST JSON
  retains its existing shape.

- **Breaking (native utterance API):** native `ast::Utterance` fields are private.
  Read its main tier and dependent tiers through `main_tier()` and
  `dependent_tiers()`. The native parser owns their grouping through lowering;
  callers cannot assemble unrelated parsed tiers or erase observed tiers from
  a native parse result. Debug serialization and shared-model output are unchanged.

- **Breaking (native word/dependent-tier API):** `WordWithAnnotations` exposes
  borrowed accessors and retains its admitted source slice. `word_from_parsed`
  derives source and location; it no longer accepts a source argument. Rich
  word grammar entry points reject a foreign source buffer. Dependent-tier
  entries keep private payload/source fields, and `dependent_tier_to_model`
  accepts the entry without a separate source slice. Native JSON is unchanged.

- **Breaking (native conversion API):** `main_tier_to_model` and
  `utterance_to_model` no longer accept a separate source argument. Located
  main tiers retain their original source; `ParsedMainTierLine::source()`
  derives it instead of exposing a replaceable source field.

- **Breaking (native header tokens):** `Token::HeaderPrefix` carries
  `HeaderPrefixToken`, whose `kind()` retains the exact lexer-selected header
  identity. Unknown names such as `@Commentary` and `@OptionsExtra` remain
  unknown headers and cannot acquire comment or option semantics. The shared
  model and serialized token payloads are unchanged.

- **Breaking (native precode AST):** `TierBody::langcode` contains the shared
  `LanguagePrecode` rather than unchecked extracted text. Main and WOR grammars
  admit language values and source locations together; model lowering no longer
  silently discards a failed language conversion. The native AST serialization
  now emits the language string; shared CHAT/model serialization is unchanged.

- **Breaking (native annotation carriers):** word replacements occupy one
  optional field before scoped annotations; `annotations()` now returns scoped
  annotations and `replacement()` returns the optional replacement. Event
  carriers hold their scoped annotations directly, replacing `AnnotatedEvent`.
  Native parsing rejects misplaced tier codes, nonword replacements, repeated
  or reordered replacements, and annotations lacking required whitespace.
  The shared `ChatParser::parse_word` boundary admits lexical words only;
  the native rich-word entry point still retains its attached syntax.

- **Breaking (native tier carrier):** `TierBody::linkers` contains located
  shared-model `Linker` values instead of arbitrary lexer tokens. The grammar
  admits their kinds once; lowering cannot silently filter out an unmatched
  token. Native carrier JSON reflects the shared linker representation.

- **Breaking (native dependent-tier API):** text-backed parsed tiers carry an
  admitted `TextTierLabel` instead of a raw prefix token.
  `dependent_tier_to_model` now returns `DependentTier` directly; every admitted
  entry retains a model tier, including explicit rejected-content variants.

- **Breaking (native word-body API):** `WordBodyError` adds
  `MalformedUnderline`. Direct word-body scanning rejects an incomplete or
  malformed underline control sequence instead of returning it as lexical text.
  Exhaustive error matches must handle the new refusal.

- **Breaking (native lexical API):** embedded or trailing NUL bytes now produce
  lexical errors and scanning continues through the complete caller input.
  `Lexer`, `lex` and `lex_line` no longer accept explicit NUL padding as EOF.
  The generated DFA distinguishes source bytes from the bounded input's end;
  parser-level control-character validation remains unchanged.

- **Breaking (native lexer API):** `DependentBodyKind::Trn` is removed.
  The re2c parser preserves retired `%trn` tiers as unsupported source text
  instead of promoting morphology-shaped content to `%mor`. E605 now reports
  the original unsupported label; malformed payloads remain opaque.

- Native re2c single-item retraces retain the admitted `ast::Chain` instead of
  unrestricted `ContentItem`. `RetraceBody::Single` now stores `Box<Chain>`;
  `RetraceBody::items` and `ContentItem::children` yield borrowed `ContentRef`
  iterators. Native AST serialization follows that shape. Marker ordering,
  shared-model output and source placement are
  preserved; unsupported single-retrace content is no longer representable.

- Native replacement annotations retain their complete source lexeme, preserving
  whitespace in `ParsedAnnotation::chat_text` and orphan recovery. The serialized
  native carrier now exposes `lexeme` instead of separate `begin`/`end` fields
  (**breaking, pre-1.0**); shared CHAT model serialization is unchanged.

- **Breaking (pre-1.0):** Removed native `Token::Shortening`, `StressPrimary`
  and `StressSecondary` variants and their discriminants. The lexer already
  carries these constructs inside rich word bodies; their body/model support
  remains unchanged. Obsolete subtoken classification and admission paths
  have been removed.

- **Breaking (pre-1.0):** Removed `talkbank_parser_re2c::parser::classify::is_word_token`.
  Subtoken assembly and annotation spacing now share one classified token owner;
  assembly consumes a nonempty sequence with its original source extent.

- **Breaking:** the native word-body scanner returns `WordBodyError`, separating
  empty segments from unclosed shortenings. It no longer constructs a completed
  shortening when its closing parenthesis is absent.

- **Breaking:** native stress and overlap carriers use shared
  `WordStressMarkerType` and `OverlapPointKind`. Remove duplicate enums and
  conversion matches; native debug kind names follow shared serialization.

- **Breaking:** native CA marker carriers use shared `CAElementType` and
  `CADelimiterType`; remove duplicate native enums, character tables and lowering
  matches. Native debug AST serialization uses the shared snake-case kind names.

- **Breaking:** native word-body text and shortening variants carry shared
  `WordText` and `WordShortening` values. Word-body scanning returns `Result`
  and refuses empty lexical segments; lowering no longer constructs unchecked
  model text. Native AST string serialization is unchanged.

- **Breaking:** native word carriers use shared `WordCategory`; remove the native
  duplicate enum and its lowering table. Native debug AST category serialization
  now uses shared-model names. Unknown rich-token prefixes are rejected.

- **Breaking:** native `ContentItem::Pause` now carries the shared located `Pause`;
  remove `PauseKindParsed` and its lowering. Grammar admission retains the
  lexer-issued full span and original timed-duration text.

- **Breaking:** remove native `TerminatorKindParsed`; lexical classification and
  shared terminator construction now use one constructor table. Terminator
  conversion refuses tokens outside their supplied source instead of returning
  an unlocated value.

- **Breaking:** native `ContentItem::Separator` carries the shared located
  `Separator`; remove `SeparatorKindParsed` and its conversion/spelling tables.
  Main and WOR parsing share one separator grammar and reject missing source
  placement instead of substituting an unlocated value during conversion.

- Native media parsing constructs shared `MediaHeader` values directly, preserving
  comma-whitespace provenance and explicit malformed-header recovery. Remove
  unused public native `MediaHeaderParsed` and `TypesHeaderParsed` structs.

- **Breaking:** the native body-only `parse_id_header` and `id_header_parser`
  return shared `IDHeader` values; `IdHeaderParsed` and its conversion are
  removed. Native file and fragment parsing use the same construction path.
  Empty elements inside nonempty ID language lists reject instead of being
  silently removed; file recovery retains the original header as unknown.

- **Breaking (experimental native fragment API):** `parse_languages_header`
  returns `ParseOutcome<LanguageCodes>`; `parse_languages_header_with_errors`
  also reports input-relative diagnostics. Malformed lists reject instead of
  becoming empty successful lists. `LanguagesHeaderParsed` is removed.

- **Breaking (experimental native AST):** `ScopedAnnotationParsed` now holds
  the shared `ContentAnnotation` plus the original lexeme. Inspect `annotation()`
  instead of matching a duplicate native enum. Public CHAT model and wire
  formats are unchanged; source spans and structural annotation policies remain.

- Native `ast::TierBody::terminator` now contains the shared `Terminator`
  instead of a lexer token. Its experimental AST serialization follows the
  shared model representation. Main, MOR and WOR parsing admit located
  terminators through one grammar boundary; `ChatParser` is unchanged.

- **Breaking:** native WOR parsing constructs shared `WorItem`/`WorTier`
  directly. Removed `WorItemParsed`, `WorTierParsed` and their converter;
  `wor_tier_parser` now takes the tier location. Unsupported word shapes and
  failed language/terminator admission reject instead of becoming placeholders
  or disappearing from successful tiers.

- **Breaking:** native PHO/SIN parsers construct shared model tiers directly.
  Removed the duplicate phonology/gesture ASTs (including `PhoWordParsed`) and conversion helpers. The
  low-level PHO API returns `ParseOutcome<PhoTier>` instead of fabricating an
  empty tier on failure; the SIN body API now lives in `parser` alongside it.

- **Breaking:** native GRA APIs return shared model values in `ParseOutcome`,
  with diagnostic variants ending in `_with_errors`. Remove native `GraTier`,
  `GraRelationParsed`, `LoweredGra` and their conversion APIs. GRA numeric
  rejection uses the shared all-or-rejected lowering transition directly.

- **Breaking:** native MOR combinators construct shared `MorWord`, `Mor` and
  `MorTier` values directly. Removed the duplicate native morphology types and
  `mor_tier_to_model` conversion. Low-level `parse_mor_word` and `parse_mor_tier`
  return `ParseOutcome` of the shared types; diagnostic variants are available.

- Native text-tier helper APIs return `ParseOutcome<BulletContent>` using the
  shared model directly. Grammar or lexical admission failure propagates
  rejection instead of a fabricated empty or silently shortened successful body.
  The duplicate `TextTierParsed` and `TextTierSegment` types and their conversion
  functions are removed. Shared model and diagnostic wire formats are unchanged.

## LSP clients and embedders

- **Breaking (pre-1.0):** LSP execute-command failures now use JSON-RPC errors
  instead of successful string payloads. Malformed arguments and unknown command
  names return `-32602`; admitted commands that fail return `-32001`. Successful
  transcript, header, graph and other command payloads keep their existing shapes.

- **Breaking (pre-1.0):** `talkbank/scopedFind` results replace `line`,
  `character` and `length` with an LSP `range`, preserving both endpoints of
  multiline matches. Matches that cannot project exactly, such as endpoints
  inside a CRLF pair, return an error instead of a rounded selection.

- **Breaking (LSP speaker extraction):** `talkbank/getSpeakers` now returns a
  JSON-RPC error when the current document has absent, unparsed or repeated
  participant declarations. It uses shared-model admission instead of returning
  an empty list or selecting the first competing header. Command failures are
  errors rather than strings inside successful responses. Missing roles in an
  admitted declaration remain explicit nulls.

- Speaker rename emits revision-bound document edits and is advertised only to
  clients supporting `workspace.workspaceEdit.documentChanges`. Direct rename
  and prepare requests from unsupported clients return no result. Out-of-bounds
  cursor positions cannot be clamped onto a different rename target.

- LSP sessions that reject an edit now require newer full text or a reopen.
  Withhold source-dependent features, clear old diagnostics, and reject further
  incremental edits until synchronization is restored.

- LSP document changes reject reversed ranges before position clamping;
  a rejected batch never commits partial text as a new revision.

- Completion, on-type formatting and linked-editing responses now use the
  shared revision guard. Lost synchronization returns `ContentModified` instead
  of an apparently successful empty result.

- LSP formatting retains its producing open-document revision and refuses a
  response after that revision changes, closes, reopens or loses synchronization.
  Formatting, quick fixes and rename share document-owned revision admission.

- **Breaking (pre-1.0):** Removed the public model/text-pair helpers
  `backend::features::document_symbol`, `backend::features::folding_range` and
  `backend::features::code_lens::code_lens`. Their server paths now consume
  source-bound analysis; code-lens anchors use parsed participant headers.

- **Breaking (pre-1.0):** Removed the public raw-text
  `backend::features::workspace_symbols_for_document` helper. Workspace-symbol
  requests now use source-bound parsed analysis, omitting unparsed drafts while
  retaining navigation for recognized headers and utterances in the current
  buffer. Symbol response shapes are unchanged.

- The unversioned `backend::features::rename::rename` edit builder is removed.
  Rename responses now pass through the server's current-document admission.

## Desktop integrations

- Desktop validation binds events and cancellation to the issuing run. Pending
  startup can be disposed, and late callbacks, failures or cancellation from a
  replaced run cannot change the newer run. The internal IPC contract now requires
  a run identity in validation/cancellation requests and event envelopes.

- Remove the dormant desktop `install_cli` command and its Rust protocol/error
  declarations. Desktop bundles do not contain a CLI; install it separately.
  The removed handler could delete an existing Unix CLI before replacement
  succeeded or overwrite a Windows executable during a failed copy.

## Distribution and support boundaries

Upgrade the CLI, standalone LSP and desktop as coordinated products. Desktop
installers do not include a CLI-install action; install the CLI separately.
The package name is `talkbank-chatter`, while its executable is `chatter`.
Registry publication remains a separate maintainer action; source builds and
GitHub artifacts do not prove crates.io installation support.

The default parser is unchanged. Both backends share the model and recovery
contracts; finite regression comparisons do not establish universal parser
parity. This release is not a 1.0 completeness or platform-acceptance claim.
Use the [release procedure](ci-and-release.md) and
[publication contract](crates-io-publication.md) for candidate qualification.
