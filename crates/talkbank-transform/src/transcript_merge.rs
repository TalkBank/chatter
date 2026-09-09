//! Structural merge of two CHAT transcripts sharing a media timeline.
//!
//! See `book/src/chatter/user-guide/merge.md` for the user contract
//! and `book/src/architecture/merge-test-plan.md` for the cycle plan
//! that drives this module's incremental growth.
//!
//! Retained-set speakers' utterances come from File 1 and everything
//! else from File 2, interleaved by start time, with File 1's headers
//! extended by File 2's participants, `@ID` rows and `@Comment` rows.
//!
//! Preconditions REFUSE rather than merging: File 1 declaring no
//! retained utterances, File 1 carrying no timeline to position File 2
//! against, a non-retained speaker appearing in both files, and a
//! donor participant colliding with a File 1 declaration that has real
//! content or disagreeing metadata. Each is a case where the merge has
//! no rule to choose, and choosing silently would damage a corpus. Selected
//! utterances also require time bullets and nondecreasing source start times;
//! the merge never sorts away a source-order conflict.
//!
//! One typed entry point, and its phase split matters. [`merge_chat_files`]
//! returns [`Merged`], which must transition through [`Merged::report`] before
//! serialization can expose a [`Reported`] file. Parsing remains at the caller
//! boundary, so an in-memory `ChatFile` is never serialized merely to be
//! parsed again.
//!
//! The earlier note here described a cycle-1 skeleton with no
//! preconditions, no tier stripping and no domain newtypes. All three
//! arrived; the note did not.

mod ordered;

use talkbank_model::ParticipantRole;
use talkbank_model::SpeakerCode;
use talkbank_model::UtteranceIdx;
use talkbank_model::WriteChat;
use talkbank_model::model::header::{Header, LanguageCodes, ParticipantEntries, ParticipantEntry};
use talkbank_model::model::{ChatFile, Line, Utterance};

/// Errors that can arise from the merge operation.
///
/// Every variant is a PRECONDITION the merge refuses on, so every one maps to
/// exit 2. There is no parse variant: `merge_chat_files` takes files that are
/// already parsed, so a parse failure is the caller's to report before the
/// merge is reached. `chatter`'s `merge_exit_code` owns the mapping.
///
/// Documented design home: `book/src/architecture/merge-domain-types.md`.
/// This enum lives in `talkbank-transform::transcript_merge` for v1;
/// it may move to `talkbank-model::merge::errors` once an
/// out-of-transform consumer needs it.
#[derive(Debug, thiserror::Error)]
pub enum MergeError {
    /// Reordering an invalid donor's opening metadata would silently repair it.
    #[error("donor @ID occurs after an opening @Comment; repair header order before merging")]
    DonorMetadataOrder,
    /// Source timing bounds do not determine which side of a section marker
    /// owns a competing utterance. No arbitrary boundary is chosen.
    #[error(
        "cannot place {competing:?} around {header}: preceding end {previous_end:?}, following start {next_start:?}"
    )]
    AmbiguousSectionPlacement {
        /// The source marker retained for adjudication.
        header: String,
        /// End of the preceding selected source utterance, if one exists.
        previous_end: Option<u64>,
        /// Start of the following selected source utterance, if one exists.
        next_start: Option<u64>,
        /// The competing utterance's exact source identity.
        competing: MergeOrigin,
    },
    /// Two sources' section timing brackets do not establish their ordering.
    #[error("section order is ambiguous between reference {reference} and donor {donor}")]
    AmbiguousSectionOrder {
        /// Reference marker.
        reference: String,
        /// Donor marker.
        donor: String,
    },
    /// The assembled transcript must pass full model validation before it can
    /// become a reportable merge result.
    #[error("merged output is invalid: {0}")]
    InvalidOutput(Box<talkbank_model::validation::ValidationFailure>),
    /// Reconciled participant headers cannot produce a consistent model map.
    #[error("merged participant declarations are inconsistent: {diagnostics:?}")]
    InvalidParticipantJoin {
        /// Diagnostics retained from the canonical model join.
        diagnostics: Vec<talkbank_model::ParseError>,
    },
    /// An utterance selected for insertion has no admitted timeline position.
    #[error(
        "selected utterance {origin:?} has no time bullet; supply an explicit placement before merging"
    )]
    UnpositionedUtterance {
        /// Source identity of the unpositioned utterance.
        origin: MergeOrigin,
    },
    /// Source order and ascending start-time order disagree. Sorting would
    /// alter the source transcript, so placement must be resolved by the caller.
    #[error(
        "source timeline reverses between {previous:?} and {current:?}; source order cannot be changed by merge"
    )]
    SourceTimelineReversal {
        /// Earlier source utterance with the later start time.
        previous: MergeOrigin,
        /// Later source utterance with the earlier start time.
        current: MergeOrigin,
    },
    /// File 1 declares no utterances for any speaker in the retain
    /// set. The merge would produce a file with no retained content
    /// (a degenerate output that researchers would mistake for a
    /// successful merge); we refuse instead.
    #[error("File 1 declares no utterances for any speaker in --retain ({retain:?})")]
    RetainSpeakersMissing {
        /// The retain set passed to [`merge_chat_files`], surfaced so the
        /// operator sees which speaker codes were searched for without
        /// re-reading the invoking command.
        retain: Vec<SpeakerCode>,
    },

    /// File 1 has retained-speaker utterances but none carry a time
    /// bullet. Without a bulleted utterance the merge has no shared
    /// timeline against which to position File 2's content, so any
    /// "merge" would be a meaningless start-time-less concatenation.
    #[error("File 1 has no time-bulleted utterances; cannot merge against a shared timeline")]
    NoTimelineInFile1,

    /// File 2 (the donor) declares an `@Languages` code not present in
    /// File 1 (the reference)'s set. Reference is treated as authoritative
    /// (typically hand-coded); donor under-claiming (e.g., ASR run in a
    /// fixed language mode) is expected and fine, but donor over-claiming
    /// is suspicious enough to refuse: it may signal a wrong-file pairing,
    /// or a language the annotator missed, either way needs a human look
    /// rather than a silent merge. Both files' declared code lists are
    /// preserved in the payload so the operator can diagnose the mismatch
    /// without re-reading the inputs.
    #[error(
        "File 2 declares language(s) not present in File 1's @Languages; \
         File 1 = {f1} ; File 2 = {f2}",
        f1 = file1.to_chat_string(),
        f2 = file2.to_chat_string(),
    )]
    LanguageMismatch {
        /// File 1's declared `@Languages` code list (empty if the file
        /// had no `@Languages` header at all).
        file1: LanguageCodes,
        /// File 2's declared `@Languages` code list (empty if the file
        /// had no `@Languages` header at all).
        file2: LanguageCodes,
    },

    /// A speaker code outside the retain set appears in both files.
    /// The merge has no rule to choose between File 1's version of the
    /// speaker's utterances and File 2's, so it refuses. The operator
    /// resolves by either adding the code to `--retain` (File 1's
    /// version wins) or by renaming the conflicting code in File 2 as
    /// a preprocessing step.
    #[error(
        "speaker {speaker} appears in both files but is not in --retain; \
         add it to --retain or rename it in File 2"
    )]
    AmbiguousSpeaker {
        /// The conflicting speaker code, named so the operator does
        /// not have to diff participant lists to identify it.
        speaker: SpeakerCode,
    },

    /// A participant code the donor uses (outside `--retain`) is already
    /// declared in File 1 with either real utterances or metadata that
    /// disagrees with the donor's declaration for that code. Silently
    /// keeping one side's declaration would either discard real content
    /// or paper over a genuine identity mismatch, so the merge refuses.
    #[error(
        "speaker {speaker} is already declared in File 1 (role {file1_role}) and also appears \
         in File 2's non-retained participants (role {donor_role}); this is ambiguous, resolve \
         by adding {speaker} to --retain or renaming it in File 2"
    )]
    ParticipantAlreadyDeclared {
        /// The colliding speaker code.
        speaker: SpeakerCode,
        /// File 1's declared role for this code.
        file1_role: ParticipantRole,
        /// The role the donor's entry for this code declares.
        donor_role: ParticipantRole,
    },
}

/// Default set of dependent-tier kinds stripped from inserted-speaker
/// utterances during merge. Each of these has an authoritative
/// producer stage downstream of merge (`align` regenerates `%wor`;
/// `morphotag` regenerates `%mor` / `%gra`; FA owns `%pho`), so
/// carrying them across the merge boundary leaves the merged file in
/// an inconsistent half-state. Stripping at merge time pushes the
/// merged file into a clean "no derived tiers" state that downstream
/// stages can own end-to-end.
///
/// Listed lowercase to match `DependentTier::kind()`. Callers that
/// want a `Vec<String>`-form of this set (e.g. CLI argument
/// defaulting) use [`default_strip_tiers`].
pub const DEFAULT_STRIP_TIERS: &[&str] = &["wor", "mor", "gra", "pho"];

/// `Vec<String>` form of [`DEFAULT_STRIP_TIERS`] for boundary code
/// (CLI argument parsing, library calls that hold owned strings)
/// that needs an allocated owned value rather than the static
/// `&[&str]` constant.
pub fn default_strip_tiers() -> Vec<String> {
    DEFAULT_STRIP_TIERS
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

/// An utterance ordinal in FILE 1's `utterances()` sequence.
///
/// A newtype over [`UtteranceIdx`] rather than the bare model type, because
/// this merge handles two files and their ordinals are different spaces. While
/// both were `UtteranceIdx`, `reference_fate` and `donor_fate` had identical
/// signatures, so handing one a `dropped_not_retained()` ordinal and the other
/// an `excluded_by_retain()` one compiled and answered about the wrong file:
/// not `None`, but a confident wrong fate. Only this module mints these, in
/// the two walks that enumerate each file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReferenceIdx(UtteranceIdx);

impl ReferenceIdx {
    /// Name an utterance of File 1 by ordinal.
    ///
    /// Public because a consumer legitimately asks about an ordinal it
    /// computed. Minting one is a deliberate act naming the file; what the
    /// newtype prevents is the accidental case, where an ordinal obtained from
    /// one side is passed to the other side's accessor and answers confidently
    /// about the wrong file.
    #[must_use]
    pub fn new(index: UtteranceIdx) -> Self {
        Self(index)
    }

    /// The underlying utterance ordinal, for indexing File 1.
    #[must_use]
    pub fn utterance(self) -> UtteranceIdx {
        self.0
    }
}

/// An utterance ordinal in FILE 2's `utterances()` sequence.
///
/// The donor counterpart of [`ReferenceIdx`]; see there for why the two spaces
/// are different types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DonorIdx(UtteranceIdx);

impl DonorIdx {
    /// Name an utterance of File 2 by ordinal.
    ///
    /// Public because a consumer legitimately asks about an ordinal it
    /// computed. Minting one is a deliberate act naming the file; what the
    /// newtype prevents is the accidental case, where an ordinal obtained from
    /// one side is passed to the other side's accessor and answers confidently
    /// about the wrong file.
    #[must_use]
    pub fn new(index: UtteranceIdx) -> Self {
        Self(index)
    }

    /// The underlying utterance ordinal, for indexing File 2.
    #[must_use]
    pub fn utterance(self) -> UtteranceIdx {
        self.0
    }
}

/// Where one output utterance came from.
///
/// The ordinal is over the source file's own `utterances()` sequence, which is
/// the space consumers reason in. Resolving one back to an utterance costs a
/// walk: `ChatFile` exposes `utterances()` as an iterator and has no
/// random-access accessor, so this is not an O(1) index into the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MergeOrigin {
    /// Utterance `n` of File 1, kept because its speaker is in `retain`.
    Retained(ReferenceIdx),
    /// Utterance `n` of File 2, inserted because its speaker is not retained.
    Inserted(DonorIdx),
}

/// What became of one donor (File 2) utterance.
///
/// One entry per donor utterance, in donor order, so this IS the partition
/// rather than one half of it. An earlier version returned only the excluded
/// ordinals and proved completeness with arithmetic (`inserted + excluded ==
/// donor count`), which needed an error variant and was blind to the failure
/// that mattered: shift every ordinal by the donor's header count and the
/// counts still balance. Indexing by donor ordinal makes "unaccounted for"
/// unwritable and answers "what happened to utterance n" in O(1), which is
/// the question a consumer actually asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DonorFate {
    /// Carried into the output, under the matching `Inserted` origin.
    Inserted {
        /// How many dependent tiers `strip_tiers` removed from this utterance.
        ///
        /// Zero means it arrived byte-preserved. A bare `Inserted` used to
        /// claim that of every donor utterance, which is a lie of omission:
        /// stripping applies to the donor side and to it alone, so an inserted
        /// utterance is carried over AND edited. The merge knew the number at
        /// the moment it did the work and threw it away.
        ///
        /// A count rather than the kinds, deliberately. WHICH kinds is already
        /// known to the caller: they are the subset of the `strip_tiers` the
        /// caller passed in that this utterance actually carried. Recording
        /// the names would allocate a collection per donor utterance to say
        /// something the caller can already derive, against the standing rule
        /// about materializing over six-figure corpora.
        tiers_stripped: usize,
    },
    /// Kept out because its speaker is in `retain`, so File 1's version wins.
    ///
    /// A variant rather than a bare "excluded" flag: a future second reason to
    /// omit a donor utterance is a new variant here, which every consumer's
    /// exhaustive match then has to acknowledge, instead of silently widening
    /// the meaning of one that already exists.
    ExcludedByRetain,
}

/// What became of one reference (File 1) utterance.
///
/// The mirror of [`DonorFate`]. It exists because the donor side was closed
/// first and the asymmetry was a real hole: a File 1 speaker outside the
/// retain set is dropped, and `AmbiguousSpeaker` does not catch it, since that
/// fires only when a code appears in BOTH files. A reference-only `MOT` with
/// `retain = [CHI]` therefore passed every precondition, kept its
/// `@Participants` row in the output, and lost every utterance silently.
// NOT `Copy`: see the `speaker` field below.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ReferenceFate {
    /// Carried into the output, under the matching `Retained` origin.
    Retained,
    /// Dropped because its speaker is not in `retain`. See the type doc.
    DroppedNotRetained {
        /// The speaker whose utterance this was.
        ///
        /// Carried because the merge HAS it at the moment it decides to drop.
        /// Recording only "dropped" makes every consumer resolve the ordinal
        /// back against a `ChatFile` it happens to hold, which is both a scan
        /// per dropped utterance and a hazard no newtype can reach: `Merged`
        /// does not own its inputs, so resolving against the WRONG file
        /// type-checks. Carrying the value deletes the resolution rather than
        /// guarding it.
        ///
        /// `SpeakerCode` is an interned `Arc<str>`, so this is a refcount bump
        /// per dropped utterance and no allocation.
        speaker: SpeakerCode,
    },
}

/// A merged transcript together with the provenance of every utterance in it.
///
/// # Why the merge returns this rather than a bare `ChatFile`
///
/// [`merge_chat_files`] assigns each selected utterance its source ordinal
/// before admitting it to a forward-only stream. Assembly consumes the whole
/// utterance with that origin. Returning only the file would discard this
/// evidence and force callers to reconstruct identity from speaker and timing,
/// which need not uniquely identify an utterance.
///
/// # The invariant, and where it is enforced
///
/// There is exactly one origin per output utterance, in the same order. That
/// is constructed by ordered assembly: each utterance and its origin are
/// consumed together. The assembled model must also pass validation before
/// this state is constructed.
///
/// Prefer [`Merged::utterances_with_origin`] to pairing the two accessors by
/// hand: zipping [`Merged::origins`] against `file().lines` type-checks and is
/// wrong by the number of header lines.
#[derive(Debug, Clone)]
pub struct Merged {
    file: talkbank_model::validation::ValidChatFile,
    origins: Vec<MergeOrigin>,
    reference_fates: Vec<ReferenceFate>,
    donor_fates: Vec<DonorFate>,
}

impl Merged {
    /// Each output utterance with where it came from, in output order.
    ///
    /// The accessor to reach for: it cannot be mis-paired, because the pairing
    /// is done here rather than by the caller.
    pub fn utterances_with_origin(&self) -> impl Iterator<Item = (&Utterance, MergeOrigin)> {
        self.file
            .document()
            .utterances()
            .zip(self.origins.iter().copied())
    }

    /// Where each output utterance came from, in output order.
    #[must_use]
    pub fn origins(&self) -> &[MergeOrigin] {
        &self.origins
    }

    /// What became of every File 1 utterance, in file order.
    ///
    /// Indexed by File 1's own `utterances()` ordinal, so
    /// `reference_fates()[n]` is the fate of File 1 utterance `n` and lines up
    /// with the ordinal in a [`MergeOrigin::Retained`].
    #[must_use]
    pub fn reference_fates(&self) -> &[ReferenceFate] {
        &self.reference_fates
    }

    /// The fate of one File 1 utterance, or `None` if there is no such
    /// utterance. The pointwise query, in O(1).
    #[must_use]
    pub fn reference_fate(&self, index: ReferenceIdx) -> Option<&ReferenceFate> {
        self.reference_fates.get(index.utterance().raw())
    }

    /// File 1 utterances dropped because their speaker is not retained, in
    /// file order.
    ///
    /// Derived from [`Merged::reference_fates`] rather than stored beside it,
    /// so the two cannot disagree.
    pub fn dropped_not_retained(&self) -> impl Iterator<Item = ReferenceIdx> + '_ {
        self.reference_fates
            .iter()
            .enumerate()
            .filter(|(_, fate)| matches!(fate, ReferenceFate::DroppedNotRetained { .. }))
            .map(|(index, _)| ReferenceIdx(UtteranceIdx::new(index)))
    }

    /// The distinct speakers whose File 1 utterances were dropped, with how
    /// many each lost, in first-appearance order.
    ///
    /// The reporting question, answered from the merge's own record. A caller
    /// does not need File 1 to ask it, which is the point: resolving ordinals
    /// against a `ChatFile` the caller happens to hold is how a report ends up
    /// describing the wrong file.
    pub fn dropped_speakers(&self) -> Vec<(SpeakerCode, usize)> {
        let mut counts: Vec<(SpeakerCode, usize)> = Vec::new();
        for fate in &self.reference_fates {
            // Exhaustive, not `let ... else`: a third variant must be decided
            // here rather than falling through a `continue`.
            let speaker = match fate {
                ReferenceFate::DroppedNotRetained { speaker } => speaker,
                ReferenceFate::Retained => continue,
            };
            match counts.iter_mut().find(|(code, _)| code == speaker) {
                Some((_, count)) => *count += 1,
                None => counts.push((speaker.clone(), 1)),
            }
        }
        counts
    }

    /// What became of every donor utterance, in donor order.
    ///
    /// Indexed by the donor's own `utterances()` ordinal, so
    /// `donor_fates()[n]` is the fate of donor utterance `n`.
    #[must_use]
    pub fn donor_fates(&self) -> &[DonorFate] {
        &self.donor_fates
    }

    /// The fate of one donor utterance, or `None` if the donor has no such
    /// utterance.
    ///
    /// The pointwise query, in O(1). It is what lets a consumer tell an
    /// utterance kept out BY POLICY from one that went missing, which an
    /// absence from [`Merged::origins`] alone cannot express.
    #[must_use]
    pub fn donor_fate(&self, index: DonorIdx) -> Option<&DonorFate> {
        self.donor_fates.get(index.utterance().raw())
    }

    /// Donor utterances the `retain` filter kept out, in donor order.
    ///
    /// Derived from [`Merged::donor_fates`] rather than stored beside it, so
    /// the two cannot disagree.
    ///
    /// Named for the one axis it covers. Its File 1 counterpart is
    /// [`Merged::dropped_not_retained`].
    ///
    /// THE BOUNDARY: utterance-level provenance is TOTAL over both inputs,
    /// and HEADER-level provenance is absent. Under that line sit a donor
    /// `@Participants` / `@ID` row deduped against a vestigial File 1
    /// declaration, which is a judgement rather than a flat policy, and donor
    /// headers other than `@ID` and `@Comment`.
    ///
    /// Tier stripping is reported, not omitted: see [`DonorFate::Inserted`].
    pub fn excluded_by_retain(&self) -> impl Iterator<Item = DonorIdx> + '_ {
        self.donor_fates
            .iter()
            .enumerate()
            .filter(|(_, fate)| matches!(fate, DonorFate::ExcludedByRetain))
            .map(|(index, _)| DonorIdx(UtteranceIdx::new(index)))
    }

    /// Hand every File 1 utterance the merge dropped to `sink`, and yield the
    /// file.
    ///
    /// THE ONLY ROUTE from a merge to a serializable file, and that is the
    /// point rather than ceremony. Dropping a File 1 speaker empties it while
    /// its `@Participants` row survives, so a consumer that never looks is a
    /// consumer that ships a transcript contradicted by its own header. Two
    /// commands in this workspace did exactly that until they were fixed by
    /// hand, one at a time; a third consumer would have repeated it, because
    /// nothing but prose said the obligation existed.
    ///
    /// A caller that genuinely wants silence writes `report(|_| {})`, which is
    /// an explicit and greppable act rather than an omission.
    #[must_use]
    pub fn report(self, mut sink: impl FnMut(&SpeakerCode, usize)) -> Reported {
        for (speaker, count) in self.dropped_speakers() {
            sink(&speaker, count);
        }
        Reported(self.file)
    }
}

/// A merged transcript whose notices have been offered to a sink.
///
/// The existence of one of these is the proof that [`Merged::report`] ran. It
/// has no other constructor, so "serialize a merge without ever asking what it
/// dropped" is not a thing that can be written.
#[derive(Debug, Clone)]
pub struct Reported(talkbank_model::validation::ValidChatFile);

impl Reported {
    /// Consume the validated result into an editable model. Any subsequent
    /// edit requires fresh validation before publication.
    #[must_use]
    pub fn into_file(self) -> ChatFile {
        self.0.into_unchecked()
    }

    /// The merged transcript, borrowed.
    #[must_use]
    pub fn file(&self) -> &ChatFile {
        self.0.document()
    }
}

/// Merge two ALREADY-PARSED CHAT files, returning the merged model and
/// the provenance of every utterance in it (see [`Merged`]).
///
/// Split out because a caller that has built or edited a [`ChatFile`] in memory
/// has nowhere else to go: serializing it back to a string only to have this
/// function re-parse it is re-parsing our own output, which this codebase bans
/// for good reason (it makes the serializer's canonicalization part of the
/// merge's semantics, silently). Returning the model rather than a string is
/// the same argument at the other end: a caller that wants to keep working on
/// the merged file should not have to parse it again either.
pub fn merge_chat_files(
    f1: &ChatFile,
    f2: &ChatFile,
    retain: &[SpeakerCode],
    strip_tiers: &[String],
) -> Result<Merged, MergeError> {
    // Precondition: donor (File 2) must not declare a language reference
    // (File 1) doesn't have. Donor under-claiming (ASR run in a fixed
    // language mode) is expected and fine; donor over-claiming is
    // suspicious enough to refuse (a wrong-file pairing, or a language
    // the annotator missed either way needs a human look, not a silent
    // merge). Exact-equality is the special case where both sets match.
    let f1_langs = extract_languages(f1);
    let f2_langs = extract_languages(f2);
    let donor_over_claims = f2_langs
        .as_slice()
        .iter()
        .any(|code| !f1_langs.as_slice().contains(code));
    if donor_over_claims {
        return Err(MergeError::LanguageMismatch {
            file1: f1_langs,
            file2: f2_langs,
        });
    }

    let in_retain = |speaker: &SpeakerCode| retain.iter().any(|s| s == speaker);

    // Precondition: File 1 must declare at least one utterance for
    // some speaker in `retain`. Without this, the merge would emit a
    // file with no retained content, a degenerate output that
    // looks like a successful merge but is actually missing the
    // authoritative data the operator wanted to preserve. Refuse
    // loudly instead.
    let retained_utts_in_f1: Vec<&Line> = f1
        .lines
        .as_slice()
        .iter()
        .filter(|line| match line {
            Line::Utterance(u) => in_retain(&u.main.speaker),
            _ => false,
        })
        .collect();
    if retained_utts_in_f1.is_empty() {
        return Err(MergeError::RetainSpeakersMissing {
            retain: retain.to_vec(),
        });
    }

    // Precondition: at least one retained utterance must carry a
    // time bullet. The merge orders all utterances by `start_ms`
    // and positions File 2's content against File 1's bullets; with
    // zero bullets there is no anchor for the shared timeline.
    let any_bulleted = retained_utts_in_f1.iter().any(|line| match line {
        Line::Utterance(u) => u.main.content.bullet.is_some(),
        _ => false,
    });
    if !any_bulleted {
        return Err(MergeError::NoTimelineInFile1);
    }

    // Precondition: a non-retained speaker appearing in both files is
    // ambiguous, the merge has no rule to choose between File 1's and
    // File 2's versions. Detect by walking File 2's utterances in
    // document order; the first non-retained speaker that also appears
    // in File 1 is reported. Document-order traversal gives a
    // deterministic, reproducible error across runs.
    let f1_speakers: std::collections::HashSet<SpeakerCode> =
        f1.unique_utterance_speakers().into_iter().collect();
    for line in f2.lines.as_slice().iter() {
        if let Line::Utterance(u) = line {
            let sp = &u.main.speaker;
            if !in_retain(sp) && f1_speakers.contains(sp) {
                return Err(MergeError::AmbiguousSpeaker {
                    speaker: sp.clone(),
                });
            }
        }
    }

    // Precondition: a donor participant code (outside `--retain`) that
    // File 1 already declares must either be a safe silent dedupe
    // (File 1's declaration is vestigial: zero utterances, matching
    // role/name metadata) or a refusal (File 1 has real content under
    // that code, or the two declarations disagree). Build the dedupe
    // set up front so the insertion filters below can consult it.
    let f1_declared = declared_participants(f1);
    let mut dedupe_codes: std::collections::HashSet<SpeakerCode> = std::collections::HashSet::new();
    for line in f2.lines.as_slice().iter() {
        if let Line::Header { header, .. } = line
            && let Header::Participants { entries } = header.as_ref()
        {
            for donor_entry in entries.iter() {
                if in_retain(&donor_entry.speaker_code) {
                    continue;
                }
                if let Some(f1_entry) = f1_declared.get(&donor_entry.speaker_code) {
                    let vestigial = utterance_count_for(f1, &donor_entry.speaker_code) == 0;
                    let roles_match = f1_entry.role == donor_entry.role;
                    // Name is part of the dedupe metadata only when BOTH
                    // sides actually declare one; if either side has no
                    // name there is nothing to compare on that dimension,
                    // so it must not by itself force a refusal.
                    let names_match = match (&f1_entry.name, &donor_entry.name) {
                        (Some(f1_name), Some(donor_name)) => f1_name == donor_name,
                        (None, _) | (_, None) => true,
                    };
                    let metadata_matches = roles_match && names_match;
                    if !vestigial || !metadata_matches {
                        return Err(MergeError::ParticipantAlreadyDeclared {
                            speaker: donor_entry.speaker_code.clone(),
                            file1_role: f1_entry.role.clone(),
                            donor_role: donor_entry.role.clone(),
                        });
                    }
                    dedupe_codes.insert(donor_entry.speaker_code.clone());
                }
            }
        }
    }

    // Collect File 2's participant entries for speakers NOT in
    // `retain`; these will extend File 1's @Participants header.
    let inserted_participants: Vec<ParticipantEntry> = f2
        .lines
        .as_slice()
        .iter()
        .filter_map(|line| match line {
            Line::Header { header, .. } => match header.as_ref() {
                Header::Participants { entries } => Some(entries),
                _ => None,
            },
            _ => None,
        })
        .flat_map(|entries| entries.iter().cloned())
        .filter(|entry| {
            !in_retain(&entry.speaker_code) && !dedupe_codes.contains(&entry.speaker_code)
        })
        .collect();

    ordered::merge(
        f1,
        f2,
        retain,
        strip_tiers,
        ordered::OpeningAdditions::from_donor(f2, retain, &dedupe_codes, inserted_participants)?,
    )
}

/// Extract the declared `@Languages` codes from `chat_file`. Returns
/// an empty `LanguageCodes` when no `@Languages` header is present; if
/// multiple `@Languages` rows somehow appear, the first wins (CHAT
/// validation should already have rejected the duplicate, but the
/// merge precondition stays robust against malformed input).
fn extract_languages(chat_file: &ChatFile) -> LanguageCodes {
    chat_file
        .headers()
        .find_map(|h| match h {
            Header::Languages { codes } => Some(codes.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// Speaker codes declared in `chat_file`'s `@Participants` header,
/// mapped to their full entry. Empty if the file has no
/// `@Participants` header line (CHAT expects exactly one; this stays
/// defensive rather than assuming).
fn declared_participants(
    chat_file: &ChatFile,
) -> std::collections::HashMap<SpeakerCode, ParticipantEntry> {
    chat_file
        .lines
        .as_slice()
        .iter()
        .filter_map(|line| match line {
            Line::Header { header, .. } => match header.as_ref() {
                Header::Participants { entries } => Some(entries),
                _ => None,
            },
            _ => None,
        })
        .flat_map(|entries| entries.iter().cloned())
        .map(|entry| (entry.speaker_code.clone(), entry))
        .collect()
}

/// Number of main-tier utterances in `chat_file` whose speaker is `code`.
fn utterance_count_for(chat_file: &ChatFile, code: &SpeakerCode) -> usize {
    chat_file
        .lines
        .as_slice()
        .iter()
        .filter(|line| matches!(line, Line::Utterance(u) if &u.main.speaker == code))
        .count()
}
