# Merge Pipeline, Domain Types

**Status:** Draft
**Last modified:** 2026-05-29 18:43 EDT

This page specifies the typed Rust vocabulary shared by `chatter merge`,
`chatter speaker-id`, the override-file reader/writer, and any future
adjudication tooling (CLI, VS Code, web). Documenting these types
**before** writing the implementing code is deliberate: the types are
the spec, and they need to be designed against the user contract in
[chatter merge](../chatter/user-guide/merge.md) and
[chatter speaker-id](../chatter/user-guide/speaker-id.md) without
being inferred from prototype code.

The design follows the cross-cutting rules in this repo's root
`CLAUDE.md`:
newtypes over primitives at every stable boundary; no boolean
blindness; no tuple-packed seams; typed errors via `thiserror`;
deterministic `BTreeMap`/`BTreeSet` over hash maps for
serialized state.

## Where the types live

All new types live in `talkbank-model::merge`. Rationale:

- Existing CHAT-domain types (`SpeakerCode`, `ParticipantRole`,
  `ParticipantEntry`, `IDHeader`, `ChatFile`) already live in
  `talkbank-model`; the new merge-pipeline types reference them
  pervasively and benefit from being co-located.
- Consumers outside `talkbank-transform` (a future override-file
  reader in a small CLI, an adjudication UI, an orchestrator
  script's Rust port) want the types without pulling in the
  tree-sitter parser, the DP-aligner, etc. `talkbank-model` is
  the lightweight type-and-validation crate that fits.
- If `talkbank-model::merge` grows past the file-size budget
  (≤400 lines per file, ≤800 hard) we split into submodules
  (`merge::override_file`, `merge::scoring`, etc.), same crate.
  Hoisting to a separate `talkbank-merge-types` crate is a future
  option but not pre-emptively warranted.

## Existing types reused (not redefined)

| Type | Defined in | Used as |
|---|---|---|
| `SpeakerCode` | `talkbank-model::model::header::codes::speaker` | Identifier for `*<CODE>:` speakers, dictionary keys in mappings, `--retain` set elements |
| `ParticipantRole` | `talkbank-model::model::header::codes::participant` | Role-tag in `@Participants` and `@ID` (`Target_Child`, `Investigator`, `Mother`, etc.) |
| `ParticipantName` | `talkbank-model::model::header::codes::participant` | Optional participant name in `@Participants` |
| `ParticipantEntry` | `talkbank-model::model::header::codes::participant` | Single `@Participants` row |
| `IDHeader` | `talkbank-model::model::header::id` | Single `@ID` row |
| `ChatFile<S>` | `talkbank-model::model::file::chat_file::core` | The merge stages' inputs and outputs (parameter `S: ValidationState`) |

None of these are redefined; the merge module imports and references
them.

## New types (specification)

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

### `InsertedRole`

The CHAT code + role-tag pair to assign to renamed speakers in
the speaker-id stage. A struct rather than two function arguments
because the pair is meaningful as a unit (in TOML override files
it serializes as a nested table; in CLI it parses as `CODE:TAG`).

```rust,ignore
/// The CHAT identity to assign to non-anchor speakers in the
/// speaker-id stage. Example: `INV:Investigator`, `MOT:Mother`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InsertedRole {
    pub code: SpeakerCode,
    pub tag: ParticipantRole,
}

impl InsertedRole {
    pub fn investigator() -> Self;    // INV:Investigator
    pub fn mother() -> Self;          // MOT:Mother
    pub fn father() -> Self;          // FAT:Father
    pub fn adult() -> Self;           // PAR:Adult
}

impl FromStr for InsertedRole {
    type Err = InsertedRoleParseError;
    /// Parses `"INV:Investigator"`. Both halves required.
    fn from_str(s: &str) -> Result<Self, Self::Err>;
}

impl Display for InsertedRole { /* "INV:Investigator" */ }
```

The convenience constructors (`investigator()`, `mother()`, etc.)
are the closed-set anchor points; arbitrary
`InsertedRole { code, tag }` is also allowed for contributor-specific
roles.

### `MappingAction`

What happens to a particular speaker in the input under a
SpeakerMapping. Enum (not boolean) to avoid blindness and to
leave room for future variants (e.g. `RenameTo { code, tag }`
when multi-role renaming becomes a need).

```rust,ignore
/// Action to apply to one speaker in a SpeakerMapping.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum MappingAction {
    /// Remove this speaker's utterances and its @Participants /
    /// @ID rows entirely.
    Drop,
    /// Rename this speaker to the mapping's `inserted_role.code`.
    /// Rewrites speaker codes on every utterance and the
    /// corresponding @Participants and @ID entries.
    Rename,
}
```

The TOML serialization uses `"drop"` / `"rename"` lowercase
strings, matching the override-file format documented in
`speaker-id.md`.

### `SpeakerMapping`

The decision record produced by the speaker-id stage and
consumed by the speaker-id apply step. Carries enough information
to apply deterministically to a `ChatFile`.

```rust,ignore
/// A decision about how to relabel a ChatFile's speakers.
///
/// Produced by `identify_mapping` (reference mode, auto), by the
/// `--mapping` flag parser (explicit mode), or by reading an
/// override-file entry (override mode). Consumed by `apply_mapping`,
/// which rewrites a ChatFile per the assignments.
///
/// All speakers in the input must appear as keys in `assignments`
///, no defaulting. This is a precondition checked at apply time
/// and is intentional (we want every decision to be explicit).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SpeakerMapping {
    /// The CHAT identity assigned to every speaker whose action
    /// is `MappingAction::Rename`. All renamed speakers go to
    /// the same role in v1 of this schema.
    pub inserted_role: InsertedRole,

    /// Per-speaker action. Use BTreeMap for deterministic
    /// serialization order.
    pub assignments: BTreeMap<SpeakerCode, MappingAction>,
}
```

The "single inserted_role across all renamed speakers" constraint
matches the doc and keeps the most-common case clean. Future
multi-role-rename use cases (a 3-speaker file where two get
different roles) extend `MappingAction` with a `RenameTo` variant
rather than changing this struct's shape.

### `DecisionMode`

How a `MergeOverride` entry came to exist. Three variants matching
the three speaker-id operation modes.

```rust,ignore
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DecisionMode {
    /// Reference-mode auto-decide with Jaccard above threshold.
    Auto,
    /// Operator supplied --mapping directly on a one-off run.
    Explicit,
    /// Read from a prior override-file entry; this is a replay.
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
adjudication.

```rust,ignore
/// One per-session decision in an override file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MergeOverride {
    pub mode: DecisionMode,
    pub mapping: SpeakerMapping,

    /// Per-speaker Jaccard scores recorded for audit. Present
    /// when the entry was produced by reference mode or by an
    /// explicit mode that followed a reference attempt.
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub scores: BTreeMap<SpeakerCode, JaccardScore>,

    /// The decisive margin, if available.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub margin: Option<Margin>,

    pub operator: OperatorId,
    pub decided_at: DateTime<Utc>,

    /// Operator note. Highly recommended for non-auto decisions.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub note: Option<String>,

    /// Flags marking unusual situations.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub flags: Vec<MergeFlag>,
}
```

The struct embeds the timestamp via `chrono::DateTime<Utc>`; serde
serializes to RFC 3339 (`2026-05-27T08:41:00Z`) by default. TOML
preserves this format faithfully.

### `OverrideFile`

The top-level container. Holds schema version + per-session
entries. Read from / written to disk as TOML.

```rust,ignore
/// Top-level override-file container.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OverrideFile {
    /// Schema version. Currently 1. Reader refuses unknown
    /// versions with a typed error rather than guessing.
    pub schema_version: u32,

    /// Per-session entries. BTreeMap for deterministic
    /// on-disk ordering.
    #[serde(flatten)]
    pub entries: BTreeMap<SessionId, MergeOverride>,
}

impl OverrideFile {
    pub const CURRENT_SCHEMA_VERSION: u32 = 1;

    /// Read an override file from a path. Refuses unknown
    /// schema versions.
    pub fn read(path: &Path) -> Result<Self, OverrideFileError>;

    /// Write the override file to a path, replacing the file
    /// atomically.
    pub fn write(&self, path: &Path) -> Result<(), OverrideFileError>;

    /// Read an override file if it exists, else return an
    /// empty file at the current schema version. Used by the
    /// `--write-override` append flow.
    pub fn read_or_default(path: &Path) -> Result<Self, OverrideFileError>;

    pub fn get(&self, id: &SessionId) -> Option<&MergeOverride>;
    pub fn insert(&mut self, id: SessionId, entry: MergeOverride);
}
```

The `#[serde(flatten)]` on `entries` means the on-disk TOML is
flat tables keyed by session ID (as shown in the
[speaker-id.md schema](../chatter/user-guide/speaker-id.md#override-file-format)):

```toml
schema_version = 1

[NF203-2]
mode = "auto"
# ...
```

rather than nested under an `[entries]` table.

## Error types

Two `thiserror`-based enums covering the merge pipeline's failure
modes. Each variant carries enough information for the CLI to
produce a useful diagnostic and for callers to pattern-match
behavior.

### `SpeakerIdError`

```rust,ignore
#[derive(Debug, thiserror::Error)]
pub enum SpeakerIdError {
    #[error("reference file has no utterances for anchor speaker {anchor}")]
    AnchorMissingInReference { anchor: SpeakerCode },

    #[error("input has only {n} distinct speakers; speaker-id requires at least 2")]
    InsufficientSpeakers { n: usize },

    #[error("Jaccard margin {margin} is below confidence threshold {threshold}; scores={scores:?}")]
    LowConfidence {
        scores: BTreeMap<SpeakerCode, JaccardScore>,
        threshold: ConfidenceThreshold,
        margin: Margin,
    },

    #[error("speaker {speaker} present in input but not covered by --mapping")]
    SpeakerNotInMapping { speaker: SpeakerCode },

    #[error("--mapping references speaker {speaker} not present in input")]
    MappingSpeakerNotInInput { speaker: SpeakerCode },

    #[error("override file has no entry for session {session}")]
    OverrideEntryMissing { session: SessionId },

    #[error("parse error reading input: {0}")]
    Parse(#[from] talkbank_parser::ParseError),

    #[error("override file I/O: {0}")]
    OverrideIo(#[from] OverrideFileError),
}
```

The `LowConfidence` variant is the only "soft" failure, the
caller (CLI) maps it to exit code 4 and prints the scores.
Every other variant maps to exit code 1 or 2 per the user-guide
contract.

### `MergeError`

```rust,ignore
#[derive(Debug, thiserror::Error)]
pub enum MergeError {
    #[error("File 1 declares no utterances for retain set {retain:?}")]
    RetainSpeakersMissing { retain: RetainSet },

    #[error("File 1 has no time-bulleted utterances; cannot merge against a shared timeline")]
    NoTimelineInFile1,

    #[error("File 1 @Languages = {file1}, File 2 @Languages = {file2}; merge requires matching language")]
    LanguageMismatch {
        file1: LanguageCode,
        file2: LanguageCode,
    },

    #[error("speaker {speaker} appears in both files but is not in --retain; specify --retain to disambiguate")]
    AmbiguousSpeaker { speaker: SpeakerCode },

    #[error("parse error: {0}")]
    Parse(#[from] talkbank_parser::ParseError),
}
```

### `OverrideFileError`

Independent enum because override-file I/O is also called by
non-speaker-id code paths (the orchestrator, future
adjudication UIs).

```rust,ignore
#[derive(Debug, thiserror::Error)]
pub enum OverrideFileError {
    #[error("override file not found at {path}")]
    NotFound { path: PathBuf },

    #[error("override file at {path} has schema_version={found}, this binary supports {supported}")]
    UnsupportedSchemaVersion {
        path: PathBuf,
        found: u32,
        supported: u32,
    },

    #[error("override file at {path} failed to parse: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("override file at {path} failed to write: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("I/O reading override file at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
```

## Module layout

```text
talkbank-model/src/merge/
    mod.rs, pub re-exports
    scoring.rs, JaccardScore, ConfidenceThreshold, Margin
    role.rs, InsertedRole, MappingAction
    mapping.rs, SpeakerMapping
    retain.rs, RetainSet
    override_file.rs, DecisionMode, MergeFlag, OperatorId,
                            SessionId, MergeOverride, OverrideFile
    errors.rs, SpeakerIdError, MergeError, OverrideFileError
```

Each file aims for the ≤400-line target; if any grows we split
further (`override_file/` becomes a directory with separate
files for the schema, the I/O, and the version-migration
logic).

## Type design rules followed

A spot-check against the cross-cutting design rules in this repo's
root `CLAUDE.md`:

- **Newtypes over primitives.** Every numeric domain value
  (`JaccardScore`, `ConfidenceThreshold`, `Margin`) is wrapped;
  every string domain value (`SessionId`, `OperatorId`,
  `SpeakerCode`, `ParticipantRole`) is wrapped or reused from
  existing wrappers. ✓
- **No tuple-packed seams.** `InsertedRole` is a struct, not
  `(SpeakerCode, ParticipantRole)`. `MergeOverride` likewise. ✓
- **No boolean blindness.** `MappingAction`, `DecisionMode`,
  `MergeFlag` are enums, not bools. `Margin::Finite/Unbounded`
  is an enum, not `Option<f64>` or `f64::INFINITY`. ✓
- **Typed errors.** Three `thiserror` enums with named-field
  variants carrying full context. ✓
- **Deterministic seams.** `BTreeMap`/`BTreeSet` for every
  serialized collection. ✓
- **Module browseability.** Six files in `merge/`, each
  scoped to one concern. ✓
- **`Default` impls present where meaningful.**
  `ConfidenceThreshold::DEFAULT = 2.0`; `OverrideFile::default()`
  for the empty-file case. ✓
- **`Display` impls present where user-visible.** `JaccardScore`,
  `Margin`, `InsertedRole`. ✓
- **`FromStr` parsers at CLI boundary, not regex hacks in
  command code.** `RetainSet::from_str`, `InsertedRole::from_str`,
  and a `parse_mapping_spec` helper for `--mapping`. ✓

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
`Cargo.toml`. `talkbank-model::merge` uses the workspace version
verbatim via `chrono = { workspace = true }`. No new datetime dep.

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

### 6. Schema versioning policy: **strict refuse-with-clear-error**

`OverrideFile::read` refuses any `schema_version != CURRENT_SCHEMA_VERSION`
with a typed `OverrideFileError::UnsupportedSchemaVersion { found,
supported }`. No automatic migration in v1.

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

Document this in `OverrideFile::read`'s rustdoc so the policy is
explicit to callers.

### 7. Where the `--mapping` parser lives: **`talkbank-model::merge::mapping`**

`parse_mapping_spec("PAR0=drop,PAR1=INV:Investigator") -> Result<SpeakerMapping, MappingSpecParseError>`
lives in the model crate alongside the `SpeakerMapping` type it
returns.

Why:

- The spec format is part of the type's contract. A reader looking
  for "how do I construct a `SpeakerMapping` from a string?" should
  find the answer where the type is defined, not in the consumer
  CLI crate.
- A future non-CLI consumer (HTTP API, library wrapper, scripting
  binding) wants the same parser without re-implementing or
  depending on `chatter`.
- The model crate has no CLI-framework dependency (no `clap`),
  but a free function returning `Result<SpeakerMapping, _>` doesn't
  need one. The `clap` value-parser in `chatter` becomes a
  thin shim: `fn clap_mapping_value(s: &str) -> Result<SpeakerMapping, String>
  { parse_mapping_spec(s).map_err(|e| e.to_string()) }`.

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

Every type in this doc gets a spec entry in
`spec/constructs/merge-types/` once we move to implementation
one spec per type/invariant pair, regenerated into Rust tests
via the current `spec/tools` generators. Spec authoring sits between this doc and
the Rust implementation; types are designed here, behavior is
pinned by specs, code follows. The spec entries are also where
behavioral invariants (e.g. "`JaccardScore::new(NaN) → Err`")
become regression gates rather than rustdoc-only contracts.
