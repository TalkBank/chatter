# Merge Pipeline, Domain Types

**Status:** Draft
**Last modified:** 2026-07-07 21:17 EDT

This page specifies the typed Rust vocabulary shared by `chatter merge`,
`chatter speaker-id`, the override-file reader/writer, and the
adjudication tooling (CLI today; a VS Code or web UI would share the
same types). It was originally written **before** the implementing
code, as a deliberate design-first specification against the user
contract in [chatter merge](../chatter/user-guide/merge.md) and
[chatter speaker-id](../chatter/user-guide/speaker-id.md). The
implementation has since shipped, and this page now records the
shipped form: where the implementation departed from the original
design (the owning crate, several type names, and the schema-v2
per-speaker role map), the affected section says so explicitly
instead of silently rewriting history.

The design follows the cross-cutting rules in this repo's root
`CLAUDE.md`:
newtypes over primitives at every stable boundary; no boolean
blindness; no tuple-packed seams; typed errors via `thiserror`;
deterministic `BTreeMap`/`BTreeSet` over hash maps for
serialized state.

## Where the types live

The merge-pipeline types live in
`crates/talkbank-transform/src/speaker_id/`, co-located with the
algorithms (`identify_mapping`, `apply_mapping`) that produce and
consume them, and are re-exported at
`talkbank_transform::speaker_id::*` (see that module's `mod.rs`). The
structural-merge error type (`MergeError`) lives beside the merge
algorithm in `crates/talkbank-transform/src/transcript_merge.rs`.

**Design history.** The original design placed the types in a new
`talkbank-model::merge` module, on co-location-with-CHAT-types and
lightweight-dependency grounds. That module was never created: the
implementation kept the types next to the algorithms whose invariants
they encode, in `talkbank-transform`. `talkbank-model` still owns the
CHAT-domain vocabulary the merge types reference (`SpeakerCode`,
`ParticipantRole`, `ParticipantEntry`, `IDHeader`, `ChatFile`); a
consumer that wants the merge types depends on `talkbank-transform`,
which the CLI, LSP, and desktop app already do.

## Designed vs shipped (quick map)

The sections below preserve the original type specification, updated
in place for the types most central to the override-file contract.
This table maps each designed name to what actually shipped, so a
reader grepping the codebase finds the right symbol. All shipped
paths are relative to `crates/talkbank-transform/src/`.

| Designed (this page, 2026-05) | Shipped |
|---|---|
| `InsertedRole` | `InsertedRoleSpec` (`speaker_id/override_file.rs`): on-disk `code` / `tag` strings plus optional `specific_role` |
| `MappingAction` | `SpeakerAction` (`speaker_id/override_file.rs`): `Rename` / `Drop` |
| `DecisionMode` | `OverrideMode` (`speaker_id/override_file.rs`): `Auto` / `Explicit` / `Override` |
| `SpeakerMapping` (single shared `inserted_role`) | On disk: `MergeOverride.mapping` plus the per-speaker `MergeOverride.adult_roles` map (schema v2). In memory: `MappingSpec = HashMap<SpeakerCode, SpeakerAssignment>` (`speaker_id/mapping.rs`), each `Rename` carrying its own code / role / specific-role |
| `Margin` enum (`Finite` / `Unbounded`) | `ConfidenceMargin(f64)` (`speaker_id/types.rs`); the unbounded case is `f64::INFINITY`, and the on-disk `margin` is a plain number |
| `JaccardScore` (fallible serde newtype) | `JaccardScore(pub f64)` (`speaker_id/types.rs`), a plain newtype; on-disk scores are bare `f64` values |
| `ConfidenceThreshold` (associated `DEFAULT`) | `ConfidenceThreshold(pub f64)` (`speaker_id/types.rs`) plus `DEFAULT_CONFIDENCE_THRESHOLD` (`speaker_id/identify.rs`) |
| `RetainSet` newtype | Not shipped; `merge_chats` takes `retain: &[SpeakerCode]` (`transcript_merge.rs`) |
| `MergeFlag` enum | Not shipped; `MergeOverride.flags` is `Vec<String>` |
| `OperatorId` / `SessionId` newtypes | `MergeOverride.operator` is `String`; override entries are keyed by `String` session IDs (a `SessionId` newtype exists in the `speaker_id/judgment/` submodule for the LLM-judgment surface) |
| `OverrideFile::CURRENT_SCHEMA_VERSION = 1` | Module-level `CURRENT_SCHEMA_VERSION: u32 = 2` (`speaker_id/override_file.rs`) |
| `SpeakerIdError` / `MergeError` / `OverrideFileError` variant sets | Shipped with revised variants; see the updated Error types section below |

## Existing types reused (not redefined)

| Type | Defined in | Used as |
|---|---|---|
| `SpeakerCode` | `talkbank-model::model::header::codes::speaker` | Identifier for `*<CODE>:` speakers, dictionary keys in mappings, `--retain` set elements |
| `ParticipantRole` | `talkbank-model::model::header::codes::participant` | Role-tag in `@Participants` and `@ID` (`Target_Child`, `Investigator`, `Mother`, etc.) |
| `ParticipantName` | `talkbank-model::model::header::codes::participant` | Optional participant name in `@Participants` |
| `ParticipantEntry` | `talkbank-model::model::header::codes::participant` | Single `@Participants` row |
| `IDHeader` | `talkbank-model::model::header::id` | Single `@ID` row |
| `ChatFile<S>` | `talkbank-model::model::file::chat_file::core` | The merge stages' inputs and outputs (parameter `S: ValidationState`) |

None of these are redefined; the `speaker_id` and `transcript_merge`
modules import and reference them.

## New types (specification)

The subsections below are the type specification. The ones central
to the override-file contract (`InsertedRoleSpec`, `SpeakerAction`,
the speaker-mapping pair, `OverrideMode`, `MergeOverride`,
`OverrideFile`, and the three error enums) have been updated in
place to the shipped form. The remaining subsections
(`JaccardScore`, `ConfidenceThreshold`, `Margin`, `RetainSet`,
`MergeFlag`, `OperatorId`, `SessionId`) are preserved as the
original design; where the shipped form differs (it does for each of
those), the designed-vs-shipped table above is authoritative for the
current symbol and shape.

### `JaccardScore`

A multiset-Jaccard similarity value, by construction in the closed
range `[0.0, 1.0]`.

```rust,ignore
/// Multiset Jaccard similarity between two bags of tokens.
///
/// By construction in [0.0, 1.0]. `JaccardScore::zero()` is the
/// no-overlap point; `JaccardScore::one()` is identical-bag.
///
/// Used by the speaker-id stage to score how well each donor
/// speaker matches a reference anchor's content.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "f64", into = "f64")]
pub struct JaccardScore(f64);

impl JaccardScore {
    pub fn new(v: f64) -> Result<Self, JaccardScoreError>;
    pub fn zero() -> Self;
    pub fn one() -> Self;
    pub fn value(self) -> f64;
}

impl Display for JaccardScore { /* "0.735" three-digit */ }
impl TryFrom<f64> for JaccardScore { /* validates range */ }
impl From<JaccardScore> for f64 { /* infallible widen */ }
```

Construction is fallible: `JaccardScore::new(1.5)` returns
`Err(JaccardScoreError::OutOfRange(1.5))`. NaN is also rejected.
Internal computation that's guaranteed in-range by construction
(the multiset formula) uses an internal `from_unchecked` private
constructor; public API is fallible.

### `ConfidenceThreshold`

The minimum Jaccard margin (`winner / loser`) the speaker-id stage
will auto-accept. By construction in `[1.0, ∞)`, a threshold of
< 1.0 makes no sense (means the loser scores higher than the
winner, which can't happen). Default 2.0 per the empirical
calibration recorded in
[`chatter speaker-id`](../chatter/user-guide/speaker-id.md).

```rust,ignore
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "f64", into = "f64")]
pub struct ConfidenceThreshold(f64);

impl ConfidenceThreshold {
    pub const DEFAULT: Self = Self(2.0);
    pub fn new(v: f64) -> Result<Self, ConfidenceThresholdError>;
    pub fn value(self) -> f64;
}

impl Default for ConfidenceThreshold {
    fn default() -> Self { Self::DEFAULT }
}
```

### `Margin`

The decisive ratio between the highest-scoring speaker and the
runner-up. Distinguished from `ConfidenceThreshold` by intent
(this is observed; the threshold is configured) and from
`JaccardScore` by range (margin is `≥ 1.0`; score is `≤ 1.0`).

Uses an enum rather than a bare float to model the
divide-by-zero case (runner-up has zero Jaccard) cleanly. Avoids
the `f64::INFINITY` sentinel that doesn't round-trip through
all serializers.

```rust,ignore
/// Ratio of winning speaker's score to runner-up's score.
///
/// `Finite(r)` for `r >= 1.0`. `Unbounded` when the runner-up
/// has zero score (winner scored anything, runner-up scored
/// nothing). Compares meaningfully against `ConfidenceThreshold`
/// regardless of variant.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Margin {
    Finite(f64),
    /// Serialized as the JSON/TOML string "unbounded"; never as
    /// f64::INFINITY (which round-trips inconsistently).
    Unbounded,
}

impl Margin {
    pub fn from_scores(winner: JaccardScore, loser: JaccardScore) -> Self;
    pub fn meets(self, threshold: ConfidenceThreshold) -> bool;
}

impl Display for Margin { /* "3.81x" or "∞" */ }
```

### `RetainSet`

The set of speaker codes specified by `--retain` on `chatter merge`.
A `BTreeSet<SpeakerCode>` wrapped in a newtype so the type
signatures of merge functions communicate intent. Empty is
allowed (means "no speakers come from File 1; File 1 contributes
only headers", a degenerate but legal case).

```rust,ignore
/// Speakers whose utterances come from the first input to
/// `chatter merge`. All other speakers come from the second
/// input.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RetainSet(BTreeSet<SpeakerCode>);

impl RetainSet {
    pub fn new() -> Self;
    pub fn from_iter<I: IntoIterator<Item = SpeakerCode>>(it: I) -> Self;
    pub fn contains(&self, code: &SpeakerCode) -> bool;
    pub fn iter(&self) -> impl Iterator<Item = &SpeakerCode>;
    pub fn is_empty(&self) -> bool;
}

impl FromStr for RetainSet {
    type Err = RetainSetParseError;
    /// Parses `"CHI,SI2"` → `{CHI, SI2}`. Empty entries rejected.
    fn from_str(s: &str) -> Result<Self, Self::Err>;
}
```

### `InsertedRoleSpec` (designed as `InsertedRole`)

The CHAT identity recorded for one renamed speaker: a speaker code, a
standard role tag, and (only when needed) a specific-role label. A
struct rather than separate function arguments because the triple is
meaningful as a unit (in TOML override files it serializes as an
inline table; on the CLI a `CODE:TAG` pair parses into one). Shipped
in `speaker_id/override_file.rs` under the name `InsertedRoleSpec`,
with on-disk `String` fields (this is the serialized form written
into override files) rather than the designed `SpeakerCode` /
`ParticipantRole` newtypes; `MergeOverride::to_mapping_spec` lifts
the strings back into the typed CHAT primitives at the read boundary.

```rust,ignore
/// Inline-table form of the inserted-role spec recorded in each
/// override entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InsertedRoleSpec {
    /// CHAT speaker code (e.g. `INV`, or `INV1` when disambiguated from
    /// a same-role collision).
    pub code: String,
    /// CHAT standard role tag (e.g. `Investigator`).
    pub tag: String,
    /// Specific-role label for `@Participants`' name/specific-role slot
    /// (e.g. `First_Investigator`), set only when two adults in the same
    /// judgment share `tag` and need the CHAT manual's `CHI1`/`CHI2`-style
    /// disambiguation. `None` for the ordinary single-adult-per-role case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specific_role: Option<String>,
}
```

The `specific_role` field is never operator-typed: it is filled by
the same-role auto-disambiguation described under the speaker-mapping
section below. On the CLI, `--inserted-role INV:Investigator` and
each `OLD=CODE:ROLE` assignment in `--mapping` supply the code / tag
pair; both halves are required.

### `SpeakerAction` (designed as `MappingAction`)

What happens to a particular speaker in the input. Enum (not
boolean) to avoid blindness. Shipped in
`speaker_id/override_file.rs` under the name `SpeakerAction`.

```rust,ignore
/// Action applied to one speaker in the input file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpeakerAction {
    /// Rename the speaker per its own entry in `adult_roles`.
    /// Rewrites speaker codes on every utterance and the
    /// corresponding @Participants and @ID entries.
    Rename,
    /// Remove this speaker's utterances and its @Participants /
    /// @ID rows entirely.
    Drop,
}
```

The TOML serialization uses `"drop"` / `"rename"` lowercase
strings, matching the override-file format documented in
[merge-overrides.md](../chatter/integrating/merge-overrides.md).
The design left room for a future `RenameTo { code, tag }` variant;
that never became necessary, because schema v2 instead resolves every
`Rename` through the per-speaker `adult_roles` map, which carries
each speaker's own target identity (next section).

### Speaker mapping: on-disk `mapping` + `adult_roles`, in-memory `MappingSpec` (designed as `SpeakerMapping`)

The decision record produced by the speaker-id stage and consumed by
the apply step. Carries enough information to apply deterministically
to a `ChatFile`. The original design was a `SpeakerMapping` struct
with a single shared `inserted_role: InsertedRole` field and the
constraint "all renamed speakers go to the same role in v1 of this
schema". Schema v2 replaced that constraint with a **per-speaker role
map**, and the shipped code splits the concept into an on-disk shape
and an in-memory shape.

**On disk**, two sibling fields of `MergeOverride`
(`speaker_id/override_file.rs`):

```rust,ignore
/// Per-donor-speaker-code role assignment, for every speaker whose
/// `mapping` action is `Rename`. Invariant: every `Rename` key in
/// `mapping` has a matching entry here.
pub adult_roles: BTreeMap<String, InsertedRoleSpec>,

/// Map from input speaker codes to actions. Every speaker that
/// exists in the input must appear here.
pub mapping: BTreeMap<String, SpeakerAction>,
```

Every `Rename` resolves via that speaker's **own** `adult_roles`
entry, so one entry can rename two speakers to two different roles
(`PAR0 -> INV:Investigator`, `PAR1 -> FAT:Father`). When two adults
in the same session are assigned the *same* role, the writer
auto-disambiguates per the CHAT manual's `CHI1`/`CHI2` convention:
numbered speaker codes (`INV1`, `INV2`), the shared standard role tag
unchanged, and ordinal specific-role labels (`First_Investigator`,
`Second_Investigator`, falling back to bare numerals past `Fourth`)
recorded in each spec's `specific_role` field
(`speaker_id/judgment/consume.rs`, `disambiguate_adult_roles`). A
hand-edited file that records a `Rename` with no matching
`adult_roles` entry fails closed at replay time with
`SpeakerIdError::OverrideRenameMissingRole`; the sanctioned
constructors (`MergeOverride::auto_decision`,
`MergeOverride::operator_decision`) maintain the covering invariant.

**In memory** (`speaker_id/mapping.rs`), the apply step consumes a
typed per-speaker assignment map:

```rust,ignore
/// What to do with a speaker named in the input file.
pub enum SpeakerAssignment {
    /// Drop the speaker entirely.
    Drop,
    /// Rename the speaker to `code` with role tag `role` (and an
    /// optional specific-role label for `@Participants`).
    Rename {
        code: SpeakerCode,
        role: ParticipantRole,
        specific_role: Option<ParticipantName>,
    },
}

/// Operator-supplied mapping from input speaker codes to
/// post-relabeling assignments.
pub type MappingSpec = HashMap<SpeakerCode, SpeakerAssignment>;
```

`MergeOverride::to_mapping_spec` converts the on-disk pair into a
`MappingSpec` for `apply_mapping`; `parse_mapping_spec` builds one
directly from the CLI `--mapping` string. The on-disk contract
requires every speaker that exists in the input to appear in
`mapping` (we want every decision to be explicit). Note a shipped
gap: `apply_mapping` currently passes through unchanged any speaker
absent from the in-memory `MappingSpec`; enforcing the
every-input-speaker precondition at apply time is a documented
follow-up (`speaker_id/apply.rs`).

### `OverrideMode` (designed as `DecisionMode`)

How a `MergeOverride` entry came to exist. Three variants matching
the three speaker-id operation modes. Shipped in
`speaker_id/override_file.rs` under the name `OverrideMode`.

```rust,ignore
/// How a speaker-id decision was made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverrideMode {
    /// Reference-mode auto-decide above confidence threshold.
    Auto,
    /// Operator-supplied `--mapping` (typically after a low-confidence
    /// reference-mode attempt).
    Explicit,
    /// Replay of a prior decision read from another override file.
    Override,
}
```

### `MergeFlag`

Extensible operator-supplied flags on an override entry. Closed
variants for known cases plus a `Custom(String)` escape hatch.

```rust,ignore
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MergeFlag {
    /// ASR diarization mixed multiple real-world roles into one
    /// speaker label. The rename may still be the best available
    /// approximation but the output is imperfect.
    DiarizationMixed,
    /// The operator could not confidently determine which speaker
    /// is which; mapping is best-guess.
    BestGuess,
    /// Open variant for contributor-specific flag vocabulary.
    /// Serializes as the inner string verbatim.
    #[serde(untagged)]
    Custom(String),
}
```

### `OperatorId`

Who made the decision. String newtype.

```rust,ignore
string_newtype!(
    /// Identifier of the operator who created an override entry.
    /// Free-form; typically a username or initials. Recorded as
    /// audit trail.
    pub struct OperatorId;
);
```

### `SessionId`

Identifies an entry within an override file. Typically the
basename stem of the input CHAT file, but the override-file
schema doesn't constrain its shape, contributors may use any
stable identifier they like (`<participant>-<timepoint>`,
`<recording-id>`, etc.).

```rust,ignore
string_newtype!(
    /// Identifies a session within an override file. Free-form
    /// stable string; typically the CHAT-file basename stem.
    pub struct SessionId;
);
```

### `MergeOverride`

A single per-session decision record. The unit of operator
adjudication. As shipped (`speaker_id/override_file.rs`):

```rust,ignore
/// A single override-file entry: the operator decision for one
/// session. See `merge-overrides.md` for field semantics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeOverride {
    /// How the decision was made.
    pub mode: OverrideMode,

    /// Per-donor-speaker-code role assignment, for every speaker
    /// whose `mapping` action is `Rename` (schema v2; see the
    /// speaker-mapping section above).
    pub adult_roles: BTreeMap<String, InsertedRoleSpec>,

    /// Map from input speaker codes to actions. Every speaker that
    /// exists in the input must appear here.
    pub mapping: BTreeMap<String, SpeakerAction>,

    /// Per-speaker Jaccard scores recorded at decision time.
    /// Present for `Auto` (and `Explicit` decisions that followed a
    /// low-confidence reference-mode attempt).
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub scores: BTreeMap<String, f64>,

    /// Winner-score / runner-up-score margin. Serialized as a
    /// number; the divide-by-zero case is `f64::INFINITY`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub margin: Option<f64>,

    /// Free-form identifier of the operator who made the decision.
    pub operator: String,

    /// When the decision was made (RFC 3339).
    pub decided_at: DateTime<Utc>,

    /// Free-text operator note. Strongly recommended for `Explicit`
    /// and `Override` modes.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub note: Option<String>,

    /// Operator-supplied audit flags (e.g. `"diarization-mixed"`,
    /// `"best-guess"`).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub flags: Vec<String>,

    /// Which engine produced this decision. Absent in pre-provenance
    /// files, which deserialize as `Deterministic`.
    #[serde(default)]
    pub engine: DecisionEngine,

    /// LLM audit trail; present only for `engine = Llm` decisions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub judgment: Option<JudgmentProvenance>,
}
```

The struct embeds the timestamp via `chrono::DateTime<Utc>`; serde
serializes to RFC 3339 (`2026-05-27T08:41:00Z`) by default. TOML
preserves this format faithfully. The `engine` / `judgment`
provenance fields postdate the original design (they record whether a
decision was deterministic or LLM-made; see
`speaker_id/provenance.rs`); they were added without a schema bump
because they are backward compatible in both directions, as
documented in
[merge-overrides.md](../chatter/integrating/merge-overrides.md).

### `OverrideFile`

The top-level container. Holds schema version + per-session
entries. Read from / written to disk as TOML.

```rust,ignore
/// Current schema version supported by this binary (module-level
/// const in `speaker_id/override_file.rs`). Readers refuse files
/// with any other value; there is no implicit version, no fallback,
/// no auto-migration. Bumped from 1 to 2 for the per-speaker
/// `adult_roles` map (was `inserted_role`, a single shared field).
pub const CURRENT_SCHEMA_VERSION: u32 = 2;

/// The full override-file document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverrideFile {
    /// Schema version. Currently 2. Always `CURRENT_SCHEMA_VERSION`
    /// when this binary writes; readers reject other values with a
    /// typed error rather than guessing.
    pub schema_version: u32,

    /// Per-session entries, alphabetically ordered by session ID
    /// via the `BTreeMap` default.
    #[serde(flatten)]
    pub entries: BTreeMap<String, MergeOverride>,
}

impl OverrideFile {
    /// Read an override file from disk, or return an empty default
    /// (with the current schema version) if the path does not
    /// exist. Refuses any `schema_version != CURRENT_SCHEMA_VERSION`.
    /// Used by the `--write-override` append flow.
    pub fn read_or_default(path: &Path) -> Result<Self, OverrideFileError>;

    /// Serialize to TOML and write via `.tmp` + rename, so a crash
    /// mid-write leaves the prior file intact rather than truncated.
    pub fn write(&self, path: &Path) -> Result<(), OverrideFileError>;

    /// Insert (or replace) the entry for `session_id`.
    pub fn upsert(&mut self, session_id: String, entry: MergeOverride);

    pub fn get(&self, session_id: &str) -> Option<&MergeOverride>;
}
```

(The designed standalone `read` never shipped; `read_or_default` is
the single read path, and the designed `insert` shipped as `upsert`.
Iteration helpers `session_ids`, `auto_entries`, and `llm_entries`
were added for diagnostics, the post-merge sanity scan, and LLM
audits respectively.)

The `#[serde(flatten)]` on `entries` means the on-disk TOML is
flat tables keyed by session ID (as shown in the
[speaker-id.md schema](../chatter/user-guide/speaker-id.md#override-file-format)):

```toml
schema_version = 2

[NF203-2]
mode = "auto"
adult_roles = { PAR0 = { code = "INV", tag = "Investigator" } }
# ...
```

rather than nested under an `[entries]` table.

## Error types

Two `thiserror`-based enums covering the merge pipeline's failure
modes. Each variant carries enough information for the CLI to
produce a useful diagnostic and for callers to pattern-match
behavior.

### `SpeakerIdError`

As shipped (`speaker_id/error.rs`; several designed variant names
changed, and the low-confidence payload became the full
`DonorMatchReport` rather than loose fields):

```rust,ignore
#[derive(Debug, thiserror::Error)]
pub enum SpeakerIdError {
    /// The `--mapping` spec couldn't be parsed.
    #[error("invalid --mapping spec: {0}")]
    InvalidMappingSpec(String),

    /// Reference mode: no utterances for the requested anchor
    /// speaker in the reference transcript.
    #[error("reference transcript has no utterances for anchor speaker {anchor}")]
    ReferenceMissingAnchor { anchor: SpeakerCode },

    /// Reference mode: fewer than two distinct donor speakers, so
    /// there is nothing for multiset-Jaccard to choose between.
    DonorTooFewSpeakers { speakers: Vec<SpeakerCode> },

    /// Reference mode: winner-to-runner-up margin below the
    /// confidence threshold; the auto-decision is refused.
    LowConfidence {
        /// Full match report: would-be winner, per-speaker scores,
        /// margin. `--write-pending` records it for adjudication.
        report: DonorMatchReport,
        threshold: ConfidenceThreshold,
    },

    /// Override-file replay: the requested session ID is not in the
    /// override file; the available IDs are surfaced.
    SessionIdNotFound { session_id: String, available: Vec<String> },

    /// Override-file replay: a `Rename` action with no matching
    /// `adult_roles` entry (hand-corrupted file); fails closed.
    OverrideRenameMissingRole { speaker: SpeakerCode },

    /// Underlying parse error from the input file.
    #[error("parse error: {0}")]
    Parse(#[from] PipelineError),
}
```

The `LowConfidence` variant is the only "soft" failure: the caller
(CLI) maps it to exit code 4 and prints the scores. `Parse` maps to
exit 1 (invalid input); every other variant maps to exit 2
(precondition violation) per the user-guide contract. The mapping is
the CLI layer's job; `SpeakerIdError` itself just classifies the
failure mode. (The designed `SpeakerNotInMapping` /
`MappingSpeakerNotInInput` variants did not ship: `apply_mapping`
currently passes through speakers absent from the mapping unchanged,
and enforcing the every-input-speaker precondition is a documented
follow-up in `speaker_id/apply.rs`. The designed `OverrideIo`
wrapping also did not ship; override-file I/O failures surface as
`OverrideFileError` directly.)

### `MergeError`

As shipped (`transcript_merge.rs`; the designed `RetainSet` payload
is a `Vec<SpeakerCode>`, and a fifth precondition variant,
`ParticipantAlreadyDeclared`, was added for the dedupe-on-insert
rule on `@Participants`):

```rust,ignore
#[derive(Debug, thiserror::Error)]
pub enum MergeError {
    /// File 1 declares no utterances for any speaker in the retain
    /// set; the merge would produce a degenerate output.
    RetainSpeakersMissing { retain: Vec<SpeakerCode> },

    /// File 1 has retained-speaker utterances but none carry a time
    /// bullet; no shared timeline to merge against.
    NoTimelineInFile1,

    /// File 2 (the donor) declares an `@Languages` code not present
    /// in File 1's set. Donor under-claiming is fine; donor
    /// over-claiming is refused (see below).
    LanguageMismatch {
        file1: LanguageCodes,
        file2: LanguageCodes,
    },

    /// A speaker code outside the retain set appears in both files'
    /// utterances; no rule to choose between the two versions.
    AmbiguousSpeaker { speaker: SpeakerCode },

    /// A donor participant code (outside --retain) is already
    /// declared in File 1 with real utterances or conflicting
    /// metadata; silent dedupe would discard content or paper over
    /// an identity mismatch.
    ParticipantAlreadyDeclared {
        speaker: SpeakerCode,
        file1_role: ParticipantRole,
        donor_role: ParticipantRole,
    },

    /// Underlying parse error from either input file.
    #[error("parse error: {0}")]
    Parse(#[from] PipelineError),
}
```

Two shipped rules worth calling out because they refine the designed
"exact `@Languages` match" and "concatenate `@Participants`"
contracts:

- **`@Languages` is donor-subset matching, not exact equality.**
  File 2 (the donor, typically ASR output) may declare a *subset* of
  File 1's languages (an ASR run in a fixed language mode
  under-claims; that is expected). Only donor **over-claiming**, a
  donor language absent from File 1, raises `LanguageMismatch`, since
  it may signal a wrong-file pairing or a language the annotator
  missed.
- **`@Participants` insertion dedupes.** A donor entry whose speaker
  code File 1 already declares is silently skipped (not inserted
  twice) when File 1's declaration is vestigial: zero utterances
  under that code, and role/name metadata matching the donor's. If
  File 1 has real utterances under the code, or the two declarations
  disagree, the merge refuses with `ParticipantAlreadyDeclared`
  instead. The same dedupe set filters the inserted `@ID` rows.

### `OverrideFileError`

Independent enum because override-file I/O is also called by
non-speaker-id code paths (the adjudication tool, future UIs). As
shipped (`speaker_id/override_file.rs`), it is leaner than the
designed five-variant version: read/write/parse failures collapse
into `Io` and `Toml`, and `found` is an `Option<u32>` so a *missing*
`schema_version` field is reported distinctly from a wrong one:

```rust,ignore
#[derive(Debug, thiserror::Error)]
pub enum OverrideFileError {
    /// The file's `schema_version` is missing or not equal to
    /// `CURRENT_SCHEMA_VERSION` (currently 2). The binary refuses to
    /// interpret unknown versions rather than risk silent misreads.
    #[error("unsupported override-file schema_version {found:?}; this binary supports {supported}")]
    UnsupportedSchemaVersion {
        /// The schema version as read from the file (None if the
        /// field was absent entirely).
        found: Option<u32>,
        /// The schema version this binary supports.
        supported: u32,
    },

    /// I/O error reading or writing the file.
    #[error("override-file I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// TOML parse / serialize error.
    #[error("override-file TOML error: {0}")]
    Toml(String),
}
```

(The designed `NotFound` variant is unnecessary: `read_or_default`
treats a missing file as the empty-file default, and any other I/O
failure surfaces through `Io`.)

## Module layout

As shipped. The design's `talkbank-model/src/merge/` layout was
never created (see "Where the types live"); the real layout is:

```text
crates/talkbank-transform/src/speaker_id/
    mod.rs             pub re-exports (the crate-facing surface)
    types.rs           JaccardScore, ConfidenceMargin, ConfidenceThreshold
    mapping.rs         MappingSpec, SpeakerAssignment, parse_mapping_spec
    identify.rs        identify_mapping, DonorMatchReport,
                       DEFAULT_CONFIDENCE_THRESHOLD
    apply.rs           apply_mapping, apply_mapping_chat
    override_file.rs   CURRENT_SCHEMA_VERSION, OverrideMode,
                       SpeakerAction, InsertedRoleSpec, MergeOverride,
                       OverrideFile, OverrideFileError
    provenance.rs      DecisionEngine, JudgmentProvenance, ModelId, ...
    error.rs           SpeakerIdError
    judgment/          LLM holistic-judgment surface (sampling, prompt
                       rendering, provider, consume; home of the
                       adult_roles same-role auto-disambiguation)

crates/talkbank-transform/src/transcript_merge.rs
    merge_chats, MergeError, DEFAULT_STRIP_TIERS
```

Each file aims for the ≤400-line target; concerns that outgrew a
single file (the LLM judgment surface) became the `judgment/`
subdirectory, exactly the split-further move this section
anticipated.

## Type design rules followed

A spot-check against the cross-cutting design rules in this repo's
root `CLAUDE.md`, restated against the shipped code:

- **Newtypes over primitives.** Every numeric domain value
  (`JaccardScore`, `ConfidenceMargin`, `ConfidenceThreshold`) is
  wrapped; CHAT-domain strings reuse the existing `SpeakerCode` /
  `ParticipantRole` / `ParticipantName` wrappers. (The designed
  `SessionId` / `OperatorId` newtypes shipped as plain `String` at
  the on-disk serialization boundary; see the designed-vs-shipped
  table.) ✓
- **No tuple-packed seams.** `InsertedRoleSpec` is a struct, not
  `(code, tag)`; `SpeakerAssignment::Rename` carries named fields;
  `MergeOverride` likewise. ✓
- **No boolean blindness.** `SpeakerAction` and `OverrideMode` are
  enums, not bools. (The designed `Margin::Finite/Unbounded` enum
  shipped as `ConfidenceMargin(f64)` with `f64::INFINITY` for the
  unbounded case, a deliberate simplification recorded in the
  table.) ✓
- **Typed errors.** Three `thiserror` enums (`SpeakerIdError`,
  `MergeError`, `OverrideFileError`) with named-field variants
  carrying full context. ✓
- **Deterministic seams.** `BTreeMap` for every serialized
  collection (`adult_roles`, `mapping`, `scores`, `entries`). The
  in-memory `MappingSpec` is a `HashMap`; it is never serialized
  directly. ✓
- **Module browseability.** One file per concern in `speaker_id/`,
  with the LLM judgment surface split into its own `judgment/`
  subdirectory. ✓
- **`Default` impls present where meaningful.**
  `DEFAULT_CONFIDENCE_THRESHOLD` (2.0); `OverrideFile::default()`
  for the empty-file case. ✓
- **`Display` impls present where user-visible.** `JaccardScore`,
  `ConfidenceMargin`, `ConfidenceThreshold`. ✓
- **Parse functions at the CLI boundary, not regex hacks in
  command code.** `parse_mapping_spec` for `--mapping`; the
  `CODE:ROLE` pair parse for `--inserted-role`. ✓

## Decisions on the seven open questions

Resolved 2026-05-27, captured here so implementers don't re-litigate.

### 1. `JaccardScore` representation: **`f64`**

Multiset Jaccard `J(A, B) = sum_w min(A[w], B[w]) / sum_w max(A[w], B[w])`
is computed from `u64` token counts, which fit in `f64`'s 53-bit
mantissa for any plausible CHAT bag-of-words. The division is
inexact in general but IEEE 754 makes it bit-deterministic given
the same inputs across every platform that implements 754 (all of
ours: Windows, macOS, Linux, x86_64, arm64).

The bit-deterministic reproducibility property is **load-bearing**
because the override-file audit trail records scores; a researcher
re-running speaker-id years later on the same inputs must compute
the same score to verify the decision. `f64` arithmetic provides
this for free given workspace platform constraints. Document the
property in the type's rustdoc.

A rational `u64/u64` representation was considered for "true"
reproducibility but adds boilerplate and a comparison-against-
threshold operation that loses the same precision in the end (the
threshold is a ratio too). Reject.

### 2. `DateTime<Utc>` crate: **`chrono`**

The workspace already pins `chrono = "0.4"` at the root
`Cargo.toml`. The merge code (in `talkbank-transform`) uses the
workspace version verbatim via `chrono = { workspace = true }`. No
new datetime dep.

The "succession-aware" rule from the workspace-root `CLAUDE.md`
contributor guide (outside the book) and the analogous
`feedback_no_terraform_only_opentofu` discipline from operator
memory says: do not fragment the ecosystem by introducing a
second tool when a workspace tool already does the job. `jiff` is
a fine library but adopting it for one new module would mean two
datetime crates in tree.

Override-file timestamps serialize as RFC 3339 UTC; chrono's serde
feature handles this with `#[serde(with = "chrono::serde::ts_rfc3339")]`
or the default `Serialize`/`Deserialize` impl.

### 3. TOML library: **`toml`** (the workspace-pinned crate)

Workspace already pins `toml = "^1.1.2"`. That crate reads AND
writes, no need to combine `toml` and `toml_edit` for the v1
override-file format.

`toml_edit` was considered for its formatting/comment preservation
across in-place edits. The case for it is hypothetical right now:
override files are primarily machine-written by `chatter speaker-id
--write-override`; human edits exist but are not the dominant
workflow. The cost of `toml_edit` is the second TOML dep (workspace
churn, plus the friction every contributor pays parsing TOML
through one API and writing through another).

If a workflow emerges where operators heavily hand-edit override
files and lose formatting on each batch re-run, swap to `toml_edit`
then. Defer.

### 4. `MergeOverride::flags`: **`Vec<MergeFlag>`**

Operator-supplied flags are semantically set-like (each flag
present or absent), but `Vec` is the right representation because:

- `MergeFlag` includes a `Custom(String)` `#[serde(untagged)]`
  variant. Deriving `Ord` on this enum requires a manual `Ord`
  impl that hashes the discriminator + the inner string. Doable
  but adds maintenance load.
- The order of flags in the on-disk file isn't load-bearing for
  correctness; deterministic single-source-write produces a
  deterministic Vec.
- Duplicates are noise but not corrupting. Document in the field's
  rustdoc that consumers should treat as set semantics
  (deduplicate before comparing).

The writer (speaker-id `--write-override` path) inserts flags in a
deterministic order; on-disk Vec is fully reproducible. If a
hand-edited file has an out-of-order or duplicated flag list, that
shows up as a non-corrupting noise in subsequent diffs, acceptable.

### 5. `SpeakerMapping::assignments`: **`BTreeMap<SpeakerCode, MappingAction>`**

Confirmed. `BTreeMap` gives:

- One-action-per-speaker by construction (no duplicate keys).
- Deterministic serialization order (alphabetical by `SpeakerCode`).
- Cheap membership tests during apply.

The CLAUDE.md "no tuple-packed seams" rule targets raw tuples *as
struct fields or function arguments*. A `BTreeMap`'s internal
key-value pairing is not a domain seam exposed to the API; it's
the representation. Approved.

(As shipped, this decision holds for the serialized shape:
`MergeOverride.mapping` is `BTreeMap<String, SpeakerAction>` and
`MergeOverride.adult_roles` is `BTreeMap<String, InsertedRoleSpec>`.
The in-memory `MappingSpec` is a `HashMap` because it is never
serialized directly.)

### 6. Schema versioning policy: **strict refuse-with-clear-error**

The reader (`OverrideFile::read_or_default`, as shipped) refuses any
`schema_version != CURRENT_SCHEMA_VERSION` with a typed
`OverrideFileError::UnsupportedSchemaVersion { found, supported }`.
No automatic migration.

This is the conservative default. Reasons:

- We have no upgrade history yet; building a migration framework
  for a problem that doesn't exist is premature abstraction
  (`CLAUDE.md` "Always Fix Root Causes" + the
  general "no premature abstraction" instinct).
- The override file is fundamentally a record of operator
  decisions. If the schema breaks, operators re-adjudicate; the
  prior file becomes a historical artifact that can be read by
  scripts with old binaries.
- When a real schema change lands and there is real upgrade
  friction, that's the moment to write a one-shot migration
  (`chatter merge migrate-overrides --from <path> --to <path>`).
  Until that happens, premature migration code is dead weight.

Document this in the reader's rustdoc so the policy is explicit to
callers. The policy has since been exercised for real: the 2026-07
v1 -> v2 bump (the per-speaker `adult_roles` map) was a breaking,
non-migrating change exactly as designed here; v1 files are refused
and their sessions re-adjudicated. The version-to-version diff and
migration instructions live in
[merge-overrides.md](../chatter/integrating/merge-overrides.md).

### 7. Where the `--mapping` parser lives: **beside the mapping type**

`parse_mapping_spec("PAR0=drop,PAR1=INV:Investigator") -> Result<MappingSpec, SpeakerIdError>`
lives alongside the `MappingSpec` type it returns, in
`talkbank_transform::speaker_id::mapping` as shipped (the design
said `talkbank-model::merge::mapping`; the parser moved with the
types when they landed in `talkbank-transform`, see "Where the types
live").

Why:

- The spec format is part of the type's contract. A reader looking
  for "how do I construct a `MappingSpec` from a string?" should
  find the answer where the type is defined, not in the consumer
  CLI crate.
- A future non-CLI consumer (HTTP API, library wrapper, scripting
  binding) wants the same parser without re-implementing or
  depending on `chatter`.
- `talkbank-transform` has no CLI-framework dependency (no `clap`),
  but a free function returning `Result<MappingSpec, _>` doesn't
  need one. The `clap` value-parser in `chatter` becomes a
  thin shim over `parse_mapping_spec`.

If at some point a SECOND mapping syntax becomes useful (e.g.,
JSON-inline, or a TOML fragment), add a `parse_mapping_json`
sibling rather than reshaping `parse_mapping_spec`. The existing
parser stays the lingua franca.

---

These decisions are the design baseline going into spec authoring
and implementation. Future revisions to any of them require an
explicit doc update plus a deprecation/migration plan, not a
silent change in the implementation.

## Relationship to specs and tests

The design intended a spec entry in `spec/constructs/merge-types/`
per type/invariant pair, regenerated into Rust tests via the
`spec/tools` generators. That directory was never created: as
shipped, the behavioral invariants are pinned directly by the Rust
test suites instead, per the layered scheme in the
[Test Plan](./merge-test-plan.md): transform-level tests
(`crates/talkbank-transform/tests/speaker_id_tests.rs`,
`transcript_merge_tests.rs`, `adjudication_tests.rs`), CLI
subprocess tests (`crates/chatter/tests/merge_tests.rs`,
`speaker_id_tests.rs`, `adjudication_tests.rs`), and per-module
`#[cfg(test)]` unit tests beside the types themselves (e.g. the
round-trip and per-speaker-role tests in
`speaker_id/override_file.rs`). Folding the fragment-level cases
(token cleaning, Jaccard goldens) into `spec/constructs/` remains an
open option, not a shipped mechanism.
