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
//! Four preconditions REFUSE rather than merging: File 1 declaring no
//! retained utterances, File 1 carrying no timeline to position File 2
//! against, a non-retained speaker appearing in both files, and a
//! donor participant colliding with a File 1 declaration that has real
//! content or disagreeing metadata. Each is a case where the merge has
//! no rule to choose, and choosing silently would damage a corpus.
//!
//! Two entry points, and the split matters. [`merge_chats`] takes and
//! returns TEXT, for a caller holding two files on disk;
//! [`merge_chat_files`] takes and returns the MODEL, for a caller that
//! built or edited a `ChatFile` in memory. The second exists because
//! serializing such a file only to have this module parse it again is
//! re-parsing our own output, which quietly makes the serializer's
//! canonicalization part of the merge's semantics.
//!
//! The earlier note here described a cycle-1 skeleton with no
//! preconditions, no tier stripping and no domain newtypes. All three
//! arrived; the note did not.

use talkbank_model::ParseValidateOptions;
use talkbank_model::ParticipantRole;
use talkbank_model::SpeakerCode;
use talkbank_model::UtteranceIdx;
use talkbank_model::WriteChat;
use talkbank_model::model::header::{Header, LanguageCodes, ParticipantEntries, ParticipantEntry};
use talkbank_model::model::{ChatFile, Line, Utterance};

use crate::PipelineError;
use crate::pipeline::parse_and_validate;
use crate::serialize::to_chat_string;

/// Errors that can arise from the merge operation.
///
/// Each variant maps to a CLI exit code per the user-guide contract:
/// `Parse` → exit 1 (invalid input); everything else → exit 2
/// (precondition violation). The CLI layer is responsible for the
/// mapping; `MergeError` itself just classifies the failure mode.
///
/// Documented design home: `book/src/architecture/merge-domain-types.md`.
/// This enum lives in `talkbank-transform::transcript_merge` for v1;
/// it may move to `talkbank-model::merge::errors` once an
/// out-of-transform consumer needs it.
#[derive(Debug, thiserror::Error)]
pub enum MergeError {
    /// File 1 declares no utterances for any speaker in the retain
    /// set. The merge would produce a file with no retained content
    /// (a degenerate output that researchers would mistake for a
    /// successful merge); we refuse instead.
    #[error("File 1 declares no utterances for any speaker in --retain ({retain:?})")]
    RetainSpeakersMissing {
        /// The retain set passed to `merge_chats`, surfaced so the
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

    /// Underlying parse error from either input file.
    #[error("parse error: {0}")]
    Parse(#[from] PipelineError),
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

/// Where one output utterance came from.
///
/// The ordinal is over the source file's own `utterances()` sequence, which is
/// the space consumers reason in. Resolving one back to an utterance costs a
/// walk: `ChatFile` exposes `utterances()` as an iterator and has no
/// random-access accessor, so this is not an O(1) index into the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MergeOrigin {
    /// Utterance `n` of File 1, kept because its speaker is in `retain`.
    Retained(UtteranceIdx),
    /// Utterance `n` of File 2, inserted because its speaker is not retained.
    Inserted(UtteranceIdx),
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
    Inserted,
    /// Kept out because its speaker is in `retain`, so File 1's version wins.
    ///
    /// A variant rather than a bare "excluded" flag: a future second reason to
    /// omit a donor utterance is a new variant here, which every consumer's
    /// exhaustive match then has to acknowledge, instead of silently widening
    /// the meaning of one that already exists.
    ExcludedByRetain,
}

/// A merged transcript together with the provenance of every utterance in it.
///
/// # Why the merge returns this rather than a bare `ChatFile`
///
/// [`merge_chat_files`] always knew this: it walks each input in order building
/// two lists, then stable-sorts the combination by `start_ms`. Returning only
/// the file was a total function discarding information it had, and the cost
/// was paid by every caller that needed to join the output back to its inputs.
/// Reconstructing the mapping afterwards means matching on `(speaker, raw
/// bullet)`, which is correct only while two facts hold that a caller cannot
/// check: that the sort is stable on `start_ms`, and that inserted utterances
/// are cloned unedited.
///
/// # The invariant, and where it is enforced
///
/// There is exactly one origin per output utterance, in the same order. That
/// is checked by [`Merged::assemble`], the single private constructor, so a
/// future edit to any of the header collections that fed `out_lines` fails at
/// that seam instead of shipping origins silently shifted against the file.
///
/// Prefer [`Merged::utterances_with_origin`] to pairing the two accessors by
/// hand: zipping [`Merged::origins`] against `file().lines` type-checks and is
/// wrong by the number of header lines.
#[derive(Debug, Clone)]
pub struct Merged {
    file: ChatFile,
    origins: Vec<MergeOrigin>,
    donor_fates: Vec<DonorFate>,
}

impl Merged {
    /// Build the result from the pieces, in output order.
    ///
    /// The birth site of the proof, and it takes the utterances still PAIRED
    /// with their origins. That is the whole reason there is no count check
    /// here: splitting the pairs is this function's own job, so "one origin
    /// per output utterance" holds by construction rather than by an
    /// arithmetic comparison that a caller could fail. An earlier version
    /// split them at the call site and compared the two lengths afterwards,
    /// which needed an error variant for a state the code then had to be
    /// trusted not to reach.
    ///
    /// The one assumption left is that `pre_end_headers` holds no utterance.
    /// It is true by construction today (only the header arm of the File 1
    /// walk pushes into it) but it is not carried by the type, because `Line`
    /// has no header-only form. A `HeaderLine` newtype in `talkbank-model`
    /// would close it; that is a model change and deliberately not made here.
    fn assemble(
        pre_end_headers: Vec<Line>,
        utterances: Vec<(Line, MergeOrigin)>,
        end_marker: Option<Line>,
        donor_fates: Vec<DonorFate>,
    ) -> Self {
        let mut origins = Vec::with_capacity(utterances.len());
        let mut out_lines = pre_end_headers;
        // One reservation for the utterances and the `@End` marker. Pushing in
        // a loop otherwise loses what `extend` gave free: `Vec::extend` from a
        // `vec::IntoIter` reserves once and bulk-copies, where repeated `push`
        // regrows (16 -> 32 -> ...) and re-moves everything each time.
        out_lines.reserve(utterances.len() + usize::from(end_marker.is_some()));
        for (line, origin) in utterances {
            out_lines.push(line);
            origins.push(origin);
        }
        if let Some(end) = end_marker {
            out_lines.push(end);
        }

        Self {
            file: ChatFile::new(out_lines),
            origins,
            donor_fates,
        }
    }

    /// The merged transcript.
    #[must_use]
    pub fn file(&self) -> &ChatFile {
        &self.file
    }

    /// Each output utterance with where it came from, in output order.
    ///
    /// The accessor to reach for: it cannot be mis-paired, because the pairing
    /// is done here rather than by the caller.
    pub fn utterances_with_origin(&self) -> impl Iterator<Item = (&Utterance, MergeOrigin)> {
        self.file.utterances().zip(self.origins.iter().copied())
    }

    /// Where each output utterance came from, in output order.
    #[must_use]
    pub fn origins(&self) -> &[MergeOrigin] {
        &self.origins
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
    pub fn donor_fate(&self, index: UtteranceIdx) -> Option<DonorFate> {
        self.donor_fates.get(index.raw()).copied()
    }

    /// Donor utterances the `retain` filter kept out, in donor order.
    ///
    /// Derived from [`Merged::donor_fates`] rather than stored beside it, so
    /// the two cannot disagree.
    ///
    /// NOT a complete account of everything the merge omits: File 1
    /// utterances whose speaker is not retained are dropped with no record,
    /// and donor headers other than `@ID` and `@Comment` are not carried.
    /// Named for the one axis it does cover.
    pub fn excluded_by_retain(&self) -> impl Iterator<Item = UtteranceIdx> + '_ {
        self.donor_fates
            .iter()
            .enumerate()
            .filter(|(_, fate)| matches!(fate, DonorFate::ExcludedByRetain))
            .map(|(index, _)| UtteranceIdx::new(index))
    }

    /// Take the merged transcript, discarding the provenance.
    ///
    /// Named so that dropping the provenance is a thing the caller says out
    /// loud, rather than what happens by default.
    #[must_use]
    pub fn into_file(self) -> ChatFile {
        self.file
    }

    /// Take every part, for a caller that must mutate the file and keep the
    /// provenance.
    ///
    /// A named struct rather than a three-slot tuple: the three values are
    /// exactly the ones [`Merged`] exists to keep together, and a tuple return
    /// of that width is the shape this workspace treats as a missing type.
    ///
    /// Splitting them is deliberate, which is the difference between this and
    /// handing out borrows. Note the provenance describes the file AS MERGED:
    /// mutate the file in a way that adds or removes utterances and the
    /// origins no longer line up with it.
    #[must_use]
    pub fn into_parts(self) -> MergedParts {
        MergedParts {
            file: self.file,
            origins: self.origins,
            donor_fates: self.donor_fates,
        }
    }
}

/// The owned pieces of a [`Merged`], for a caller that must mutate the file.
///
/// Public fields, deliberately: once a caller has asked for the pieces, the
/// pairing is theirs to maintain, and pretending otherwise with accessors
/// would suggest a guarantee this type cannot make.
#[derive(Debug, Clone)]
pub struct MergedParts {
    /// The merged transcript.
    pub file: ChatFile,
    /// Where each output utterance came from, in output order.
    pub origins: Vec<MergeOrigin>,
    /// What became of each donor utterance, in donor order.
    pub donor_fates: Vec<DonorFate>,
}

/// Merge two CHAT files. Retained-set speakers' utterances come from
/// `file1_content`; all other speakers' utterances come from
/// `file2_content`. The merged file's headers come from `file1_content`.
///
/// `strip_tiers` lists dependent-tier kinds (lowercase, matching
/// `DependentTier::kind()`, `"wor"`, `"mor"`, `"gra"`, `"pho"`, etc.)
/// that should be removed from inserted-speaker utterances before
/// they are emitted. The strip set never affects retained-speaker
/// utterances. Callers that want the standard pipeline behavior
/// pass [`DEFAULT_STRIP_TIERS`]; an empty slice preserves every
/// dependent tier verbatim.
///
/// Utterances are ordered ascending by `start_ms`; utterances missing
/// a main-tier bullet sort to the end.
///
/// Returns a `String`, so the per-utterance provenance is dropped. A caller
/// that needs it works on the model with [`merge_chat_files`].
pub fn merge_chats(
    file1_content: &str,
    file2_content: &str,
    retain: &[SpeakerCode],
    strip_tiers: &[String],
    options: ParseValidateOptions,
) -> Result<String, MergeError> {
    let f1 = parse_and_validate(file1_content, options.clone())?;
    let f2 = parse_and_validate(file2_content, options)?;
    // `into_file`, not `file`: dropping the provenance is said out loud, and
    // the origins are freed before serialization allocates.
    Ok(to_chat_string(
        &merge_chat_files(&f1, &f2, retain, strip_tiers)?.into_file(),
    ))
}

/// Merge two ALREADY-PARSED CHAT files, returning the merged model and
/// the provenance of every utterance in it (see [`Merged`]).
///
/// The typed core of [`merge_chats`], which is now a thin wrapper that parses
/// its two inputs and serializes the result.
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

    // Collect File 2's @ID rows for speakers NOT in `retain`,
    // these are injected after File 1's last @ID row.
    let inserted_id_lines: Vec<Line> = f2
        .lines
        .as_slice()
        .iter()
        .filter(|line| match line {
            Line::Header { header, .. } => match header.as_ref() {
                Header::ID(id) => !in_retain(&id.speaker) && !dedupe_codes.contains(&id.speaker),
                _ => false,
            },
            _ => false,
        })
        .cloned()
        .collect();

    // Collect File 2's @Comment rows verbatim. Donor @Comment
    // content carries provenance (ASR engine identification, run
    // timestamps, processing notes) that the merged file's audit
    // trail must preserve.
    let inserted_comment_lines: Vec<Line> = f2
        .lines
        .as_slice()
        .iter()
        .filter(|line| match line {
            Line::Header { header, .. } => matches!(header.as_ref(), Header::Comment { .. }),
            _ => false,
        })
        .cloned()
        .collect();

    // Indices of File 1's last @ID and last @Comment lines, if any.
    // We use these as the "insert after" points for the
    // corresponding File 2 rows. The helper centralizes the
    // shared shape (reverse-scan for last matching header).
    let f1_last_id_idx = last_header_index(f1, |h| matches!(h, Header::ID(_)));
    let f1_last_comment_idx = last_header_index(f1, |h| matches!(h, Header::Comment { .. }));

    // Split File 1's lines into pre-@End headers and the @End marker.
    // The @Participants header (if any) is rewritten to concatenate
    // File 1's entries with `inserted_participants`. Utterances from
    // File 1 are kept only if their speaker is in `retain`.
    let mut pre_end_headers: Vec<Line> = Vec::new();
    let mut end_marker: Option<Line> = None;
    // Paired with its origin from the moment it is collected, so the sort
    // below moves the two together and no later step can re-derive the
    // pairing wrongly.

    for (i, line) in f1.lines.as_slice().iter().enumerate() {
        match line {
            Line::Header {
                header,
                span,
                separator,
            } => {
                if matches!(header.as_ref(), Header::End) {
                    end_marker = Some(line.clone());
                } else if let Header::Participants { entries } = header.as_ref() {
                    let mut combined: Vec<ParticipantEntry> = entries.iter().cloned().collect();
                    combined.extend(inserted_participants.iter().cloned());
                    let merged_header = Header::Participants {
                        entries: ParticipantEntries::new(combined),
                    };
                    pre_end_headers.push(Line::Header {
                        header: Box::new(merged_header),
                        span: *span,
                        separator: *separator,
                    });
                } else {
                    pre_end_headers.push(line.clone());
                }
            }
            // Utterances are collected in their own pass below. This walk
            // owns the headers and the `@End` marker, and its `i` is a LINE
            // index; mixing an utterance ordinal into it was a counter kept
            // correct by a comment.
            Line::Utterance(_) => {}
        }
        // After emitting File 1's last @ID row, inject File 2's
        // non-retained @ID rows so they appear contiguously with
        // File 1's @ID block. After File 1's last @Comment row,
        // inject File 2's @Comment rows so donor provenance is
        // preserved in the audit trail. Both follow the
        // user-guide contract: "File 1's rows first, then File 2's
        // rows in original order."
        if Some(i) == f1_last_id_idx {
            pre_end_headers.extend(inserted_id_lines.iter().cloned());
        }
        if Some(i) == f1_last_comment_idx {
            pre_end_headers.extend(inserted_comment_lines.iter().cloned());
        }
    }

    // File 1 may have no row of the kind at all, and then the "insert after
    // File 1's last one" rule above never fires and File 2's rows are DROPPED.
    // That is silent data loss, and for `@Comment` it is loss of exactly the
    // provenance the contract says to preserve: an ASR donor records its engine
    // and run time there, and a hand-coded reference typically carries no
    // `@Comment` at all, so the case is the common one rather than a corner.
    //
    // Found by porting a Python merge that had this fall-through and diffing
    // the two outputs: 1,014 donor `@Comment` rows across 345 sessions appeared
    // on the Python side and nowhere on ours.
    // IDs before comments, so a file needing both fall-throughs still gets its
    // `@ID` block above its `@Comment` block, which is where a reader looks.
    //
    // The `@ID` arm is DEFENSIVE and currently unreachable: a valid File 1 has
    // an `@ID` row for every declared participant (E522), so the insertion
    // point always exists. Kept for symmetry with the comment arm, and said
    // here rather than left for a reader to work out, because a test for it
    // passes whether or not the arm is present.
    if f1_last_id_idx.is_none() {
        pre_end_headers.extend(inserted_id_lines);
    }
    if f1_last_comment_idx.is_none() {
        pre_end_headers.extend(inserted_comment_lines);
    }

    // From File 2, take only utterances whose speaker is NOT in
    // `retain`. (Header reconciliation beyond "File 1 wins" is a
    // later cycle.) Strip dependent tiers in DEFAULT_STRIP_TIERS so
    // the merged file enters downstream align / morphotag stages in
    // the expected "no derived tiers" state.
    // `utterances().enumerate()` rather than a hand-rolled counter over
    // `lines`: this walk reads nothing but utterances, so the ordinal being
    // an index into `f2.utterances()` is true by construction instead of by
    // a comment saying so.
    // BOTH files' utterances are collected the same way: over
    // `utterances().enumerate()`, so the ordinal in a `MergeOrigin` indexes
    // that file's own `utterances()` by construction. The File 1 half used to
    // be a `mut` counter inside the header walk, which no test could catch
    // getting out of step: with every File 1 speaker retained, the retained
    // subset and the full sequence are the same list.
    let mut retained_utts: Vec<(Line, MergeOrigin)> = Vec::with_capacity(f1.lines.len());
    for (ordinal, u) in f1.utterances().enumerate() {
        if in_retain(&u.main.speaker) {
            retained_utts.push((
                Line::Utterance(Box::new(u.clone())),
                MergeOrigin::Retained(UtteranceIdx::new(ordinal)),
            ));
        }
    }

    // One fate per donor utterance, in donor order, so the record is a
    // partition rather than a list that has to be proved complete.
    // `with_capacity(lines.len())` is an O(1) upper bound that avoids the
    // 4-8-16-... regrow ladder; `donor_fates` is small and cheap either way.
    let mut inserted_utts: Vec<(Line, MergeOrigin)> = Vec::with_capacity(f2.lines.len());
    let mut donor_fates: Vec<DonorFate> = Vec::new();
    for (ordinal, u) in f2.utterances().enumerate() {
        if in_retain(&u.main.speaker) {
            donor_fates.push(DonorFate::ExcludedByRetain);
            continue;
        }
        let mut cloned = u.clone();
        cloned
            .dependent_tiers
            .retain(|tier| !strip_tiers.iter().any(|s| s == tier.tier.kind()));
        inserted_utts.push((
            Line::Utterance(Box::new(cloned)),
            MergeOrigin::Inserted(UtteranceIdx::new(ordinal)),
        ));
        donor_fates.push(DonorFate::Inserted);
    }

    // Combine and sort by start_ms. Utterances without a main-tier
    // bullet sort to the end with `u64::MAX` so they don't disturb
    // the ordering of timed utterances.
    let mut all_utts: Vec<(Line, MergeOrigin)> = retained_utts;
    all_utts.extend(inserted_utts);
    // `sort_by_key` is STABLE, which is what makes two utterances sharing a
    // start_ms keep File 1 ahead of File 2. The origins ride along in the same
    // tuple, so that guarantee no longer has to be re-derived by a caller.
    all_utts.sort_by_key(|(line, _)| line_start_ms(line));

    // Assemble: `assemble` splits the pairs itself, which is what makes
    // one-origin-per-utterance structural instead of checked.
    Ok(Merged::assemble(
        pre_end_headers,
        all_utts,
        end_marker,
        donor_fates,
    ))
}

/// Extract an utterance's main-tier `start_ms`. Returns `u64::MAX`
/// for non-utterance lines and for utterances without a main-tier
/// bullet, so those entries sort to the end of the timeline.
fn line_start_ms(line: &Line) -> u64 {
    match line {
        Line::Utterance(u) => u
            .main
            .content
            .bullet
            .as_ref()
            .map(|b| b.timing.start_ms)
            .unwrap_or(u64::MAX),
        Line::Header { .. } => u64::MAX,
    }
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

/// Find the index of the last header line in `chat_file` whose
/// `Header` payload matches `predicate`. Returns `None` if no
/// matching header is present.
///
/// Used by the header-reconciliation logic to identify the slot at
/// which File 2's contributions of a given header kind (e.g. @ID,
/// @Comment) should be inserted to keep the kind contiguous in the
/// merged output.
fn last_header_index<F>(chat_file: &ChatFile, predicate: F) -> Option<usize>
where
    F: Fn(&Header) -> bool,
{
    chat_file
        .lines
        .as_slice()
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, line)| match line {
            Line::Header { header, .. } if predicate(header.as_ref()) => Some(i),
            _ => None,
        })
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
