//! The `@Participants` / `@ID` / `@Birth` join, owned once.
//!
//! # Why this lives in the model rather than in a parser
//!
//! It is pure header logic over the LOWERED model: given `&[Line]`, decide
//! which speakers the file declares, in what order, and which declarations are
//! inconsistent. Nothing about it depends on how the bytes were parsed, so
//! every backend should reach the same answer by construction rather than by
//! two teams writing the same loop.
//!
//! It had been written three times. The tree-sitter parser had a production
//! copy over `&[Line]` and a test-only twin over `&[Header]` that the unit
//! tests actually exercised, so the tested one had no production caller and
//! the production one had no unit test. The re2c backend had a third,
//! hand-rolled, and that is the one that drifted: it built the map from the
//! `@ID` headers instead of from the declaration, which accounted for 445 of
//! the 446 whole-model divergences between the two backends across the
//! 107,403-file corpus, measured 2026-08-15.
//!
//! The rule, stated once so it stops being re-derived: **a speaker appears in
//! the map when `@Participants` declares it AND an `@ID` matches it**, in
//! declaration order. Everything else is a diagnostic: E522 for a declaration
//! with no `@ID`, E523 for an `@ID` with no declaration, E524 for an `@Birth`
//! naming neither.
//!
//! # CHAT format requirements
//!
//! ```chat
//! @Participants:    CHI Ruth Target_Child, INV Chiat Investigator
//! @ID:    eng|chiat|CHI|10;03.||||Target_Child|||
//! @ID:    eng|chiat|INV|||||Investigator|||
//! @Birth of CHI:    28-JUN-2001
//! ```
//!
//! yields CHI (name "Ruth", role "Target_Child", age "10;03.", born
//! 28-JUN-2001) and INV (name "Chiat", role "Investigator").

use indexmap::IndexMap;

use crate::errors::source_location::ErrorVec;
use crate::model::{ChatDate, Header, IDHeader, Line, Participant, ParticipantEntry, SpeakerCode};
use crate::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};

/// A borrowed join plan and its declaration/metadata diagnostics.
///
/// # Why this is a type rather than a `(map, errors)` pair
///
/// The map and the diagnostics are one answer, and the pair let a caller keep
/// half of it. The re2c backend did exactly that (`let (participants, _) =
/// ...`), so E522, E523 and E524 were silently absent from every file that
/// backend parsed, while the tree-sitter backend reported them: a validity
/// divergence with nothing in the types to notice it.
///
/// The reporting transitions are the only way to reach joined metadata; each
/// takes a sink. Possession of joined metadata is therefore proof that the
/// diagnostics were reported. Dropping them is still possible, but only by
/// naming [`NullErrorSink`](crate::errors::NullErrorSink), which says so.
#[must_use = "consume the join through a reporting transition"]
pub struct ParticipantJoin<'a> {
    participants: IndexMap<SpeakerCode, ParticipantView<'a>>,
    diagnostics: ErrorVec,
}

/// A declaration and its observed matching metadata; construction happens once
/// at the join boundary. Validation can consume diagnostics without cloning ASTs.
/// A view cannot survive an edit to the headers it describes:
///
/// ```compile_fail,E0502
/// # use talkbank_model::model::ChatFile;
/// # let mut file = ChatFile::new(vec![]);
/// let participant = file.get_participant("CHI");
/// file.lines.remove(0);
/// println!("{:?}", participant);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticipantView<'a> {
    entry: &'a ParticipantEntry,
    id: &'a IDHeader,
    birth: Option<&'a ChatDate>,
}

impl<'a> ParticipantView<'a> {
    /// Canonical speaker code text.
    pub fn speaker_code(&self) -> &'a str {
        self.code().as_str()
    }

    /// Whether a birth header was observed.
    pub fn has_birth_date(&self) -> bool {
        self.birth.is_some()
    }

    /// Speaker identity from the current declaration.
    pub fn code(&self) -> &'a SpeakerCode {
        &self.entry.speaker_code
    }

    /// Matching observed ID header; this is not a validity certificate.
    pub fn id(&self) -> &'a IDHeader {
        self.id
    }

    /// Observed birth date, if present.
    pub fn birth_date(&self) -> Option<&'a ChatDate> {
        self.birth
    }

    /// Name from the declaration.
    pub fn name(&self) -> Option<&'a crate::model::ParticipantName> {
        self.entry.name()
    }

    /// Role from the declaration.
    pub fn role(&self) -> Option<&'a crate::model::ParticipantRole> {
        self.entry.role()
    }

    /// Observed age text.
    pub fn age(&self) -> Option<&'a str> {
        self.id.age.as_ref().map(|age| age.as_str())
    }

    /// Observed sex metadata.
    pub fn sex(&self) -> Option<&'a crate::model::Sex> {
        self.id.sex.as_ref()
    }

    /// Languages from the ID header.
    pub fn languages(&self) -> &'a crate::model::LanguageCodes {
        &self.id.language
    }

    /// Nonempty corpus name from the ID header.
    pub fn corpus(&self) -> Option<&'a str> {
        let corpus = self.id.corpus.as_str();
        (!corpus.is_empty()).then_some(corpus)
    }

    /// Explicit detached snapshot, no longer tied to subsequent header edits.
    pub fn to_owned_participant(self) -> Participant {
        let mut participant = Participant::new(self.entry.clone(), self.id.clone());
        if let Some(date) = self.birth {
            participant = participant.with_birth_date(date.clone());
        }
        participant
    }
}

impl<'a> ParticipantJoin<'a> {
    /// Report diagnostics before exposing views borrowed from the source lines.
    pub fn report_borrowed_into(
        self,
        errors: &(impl ErrorSink + ?Sized),
    ) -> Vec<ParticipantView<'a>> {
        errors.report_vec(self.diagnostics);
        self.participants.into_values().collect()
    }

    /// Report the join's diagnostics into `errors` and yield the participant map.
    pub fn report_into(
        self,
        errors: &(impl ErrorSink + ?Sized),
    ) -> IndexMap<SpeakerCode, Participant> {
        self.report_borrowed_into(errors)
            .into_iter()
            .map(|joined| (joined.code().clone(), joined.to_owned_participant()))
            .collect()
    }

    /// Validate the join without constructing or replacing a metadata cache.
    pub fn report_diagnostics(self, errors: &(impl ErrorSink + ?Sized)) {
        errors.report_vec(self.diagnostics);
    }
}

/// Whether all observed ID headers were readable for absence claims.
enum IDCoverage {
    Complete,
    Rejected,
}

/// Build the participant map from parsed lines, preserving source spans in
/// diagnostics.
///
/// Scans `lines` for `@Participants`, `@ID` and `@Birth of` headers, then
/// cross-references them. Only `Line::Header` variants are inspected;
/// utterance and dependent-tier lines are skipped.
///
/// A file with no `@Participants` header declares no speakers, so the map is
/// empty and nothing is reported: without a declaration there is no
/// `@ID` to call orphaned.
///
/// # Diagnostics
///
/// - **E522** (`SpeakerNotDefined`): a speaker in `@Participants` has no `@ID`.
/// - **E523** (`OrphanIDHeader`): an `@ID` header has no `@Participants` entry.
/// - **E524** (`BirthUnknownParticipant`): an `@Birth of` header names a
///   speaker that is not declared, independently of ID metadata availability.
pub fn build_participants_from_lines(lines: &[Line]) -> ParticipantJoin<'_> {
    let mut diagnostics = ErrorVec::new();
    let mut participants = IndexMap::new();

    let Ok(declaration) =
        super::roster::admit_participant_declaration(lines.iter().filter_map(|line| match line {
            Line::Header {
                header, location, ..
            } => Some((header.as_ref(), *location)),
            _ => None,
        }))
    else {
        return ParticipantJoin {
            participants,
            diagnostics,
        };
    };
    let (entries, participants_location) = declaration.into_parts();

    // Scan metadata after declaration admission. Subsequent work touches only
    // these header kinds; the admitted entries and location need no rediscovery.
    //
    // The `@ID` and `@Birth` lists stay in source order and keep every
    // occurrence, because the orphan checks below report one diagnostic per
    // header rather than one per distinct speaker. Both lists are file-header
    // sized (tens of entries at most, against 2 to 6 participants), so the
    // lookups below scan them rather than building maps: two hash maps would
    // be more allocation than the scans they replace at these sizes.
    let mut id_headers: Vec<(&IDHeader, crate::Location)> = Vec::new();
    let mut id_coverage = IDCoverage::Complete;
    let mut births: Vec<(&SpeakerCode, &ChatDate, crate::Location)> = Vec::new();

    for line in lines {
        let Line::Header {
            header, location, ..
        } = line
        else {
            continue;
        };
        match header.as_ref() {
            Header::ID(id) => id_headers.push((id, *location)),
            Header::UnparsedID(_) => id_coverage = IDCoverage::Rejected,
            Header::Birth { participant, date } => births.push((participant, date, *location)),
            _ => {}
        }
    }

    for entry in entries {
        let speaker_code = entry.speaker_code.clone();
        let speaker_str = speaker_code.as_str();

        let matching_id = id_headers
            .iter()
            .find(|(id, _)| id.speaker.as_str() == speaker_str);

        match matching_id {
            Some((id, _)) => {
                let birth = births
                    .iter()
                    .find(|(speaker, _, _)| speaker.as_str() == speaker_str)
                    .map(|(_, date, _)| *date);
                participants.insert(
                    entry.speaker_code.clone(),
                    ParticipantView { entry, id, birth },
                );
            }
            None if matches!(id_coverage, IDCoverage::Rejected) => {
                // An unreadable ID may belong to this speaker. Absence is unproven.
            }
            None => {
                diagnostics.push(
                    ParseError::new(
                        ErrorCode::SpeakerNotDefined,
                        Severity::Error,
                        DiagnosticLocation::from(participants_location),
                        ErrorContext::new(
                            speaker_str,
                            crate::Span::new(0, speaker_str.len()),
                            speaker_str,
                        ),
                        format!(
                            "Speaker '{}' declared in @Participants but has no matching @ID header",
                            speaker_str
                        ),
                    )
                    .with_suggestion(format!(
                        "Add an @ID header for declared speaker '{}' with its observed metadata",
                        speaker_str
                    )),
                );
            }
        }
    }

    for (id, id_location) in &id_headers {
        if !entries.iter().any(|entry| entry.speaker_code == id.speaker) {
            let speaker_str = id.speaker.to_string();
            diagnostics.push(
                ParseError::new(
                    ErrorCode::OrphanIDHeader,
                    Severity::Error,
                    DiagnosticLocation::from(*id_location),
                    ErrorContext::new(
                        &speaker_str,
                        crate::Span::new(0, speaker_str.len()),
                        &speaker_str,
                    ),
                    format!(
                        "@ID header for '{}' but speaker not in @Participants",
                        speaker_str
                    ),
                )
                .with_suggestion(format!(
                    "Add to @Participants: {} <name> <role>",
                    speaker_str
                )),
            );
        }
    }

    for (speaker, _, birth_location) in &births {
        if !entries.iter().any(|entry| &entry.speaker_code == *speaker) {
            diagnostics.push(
                ParseError::new(
                    ErrorCode::BirthUnknownParticipant,
                    Severity::Error,
                    DiagnosticLocation::from(*birth_location),
                    ErrorContext::new(
                        speaker.as_str(),
                        crate::Span::new(0, speaker.len()),
                        speaker.as_str(),
                    ),
                    format!(
                        "@Birth header for '{}' but speaker not a declared participant",
                        speaker
                    ),
                )
                .with_suggestion(format!(
                    "Add to @Participants: {} <name> <role>, or remove @Birth header",
                    speaker
                )),
            );
        }
    }

    ParticipantJoin {
        participants,
        diagnostics,
    }
}

#[cfg(test)]
mod tests;
