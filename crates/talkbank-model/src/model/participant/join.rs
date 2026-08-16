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
use crate::model::{ChatDate, Header, IDHeader, Line, Participant, SpeakerCode};
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span};

/// The join's result: the participant map, and what was wrong with the
/// declarations that produced it.
///
/// # Why this is a type rather than a `(map, errors)` pair
///
/// The map and the diagnostics are one answer, and the pair let a caller keep
/// half of it. The re2c backend did exactly that (`let (participants, _) =
/// ...`), so E522, E523 and E524 were silently absent from every file that
/// backend parsed, while the tree-sitter backend reported them: a validity
/// divergence with nothing in the types to notice it.
///
/// [`report_into`](Self::report_into) is the ONLY way to reach the map, and it
/// takes the sink. Possession of the map is therefore proof that the
/// diagnostics were reported. Dropping them is still possible, but only by
/// naming [`NullErrorSink`](crate::errors::NullErrorSink), which says so.
#[must_use = "the participant map is only reachable through `report_into`, which is also what reports the diagnostics"]
pub struct ParticipantJoin {
    participants: IndexMap<SpeakerCode, Participant>,
    diagnostics: ErrorVec,
}

impl ParticipantJoin {
    /// Report the join's diagnostics into `errors` and yield the participant map.
    pub fn report_into(
        self,
        errors: &(impl ErrorSink + ?Sized),
    ) -> IndexMap<SpeakerCode, Participant> {
        errors.report_vec(self.diagnostics);
        self.participants
    }
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
///   speaker that is not in the map.
pub fn build_participants_from_lines(lines: &[Line]) -> ParticipantJoin {
    let mut diagnostics = ErrorVec::new();
    let mut participants = IndexMap::new();

    // One pass over every line, which is the only unavoidable O(n) walk here:
    // nothing marks where the headers end. Everything after this touches only
    // the three header kinds the join is about, never the utterances again.
    //
    // The `@ID` and `@Birth` lists stay in source order and keep every
    // occurrence, because the orphan checks below report one diagnostic per
    // header rather than one per distinct speaker. Both lists are file-header
    // sized (tens of entries at most, against 2 to 6 participants), so the
    // lookups below scan them rather than building maps: two hash maps would
    // be more allocation than the scans they replace at these sizes.
    let mut declaration: Option<(&crate::model::ParticipantEntries, Span)> = None;
    let mut id_headers: Vec<(&IDHeader, Span)> = Vec::new();
    let mut births: Vec<(&SpeakerCode, &ChatDate, Span)> = Vec::new();

    for line in lines {
        let Line::Header { header, span, .. } = line else {
            continue;
        };
        match header.as_ref() {
            // First declaration wins, matching the single `@Participants`
            // header the format allows.
            Header::Participants { entries } if declaration.is_none() => {
                declaration = Some((entries, *span));
            }
            Header::ID(id) => id_headers.push((id, *span)),
            Header::Birth { participant, date } => births.push((participant, date, *span)),
            _ => {}
        }
    }

    let Some((entries, participants_span)) = declaration else {
        return ParticipantJoin {
            participants,
            diagnostics,
        };
    };

    for entry in entries {
        let speaker_code = entry.speaker_code.clone();
        let speaker_str = speaker_code.as_str();

        let matching_id = id_headers
            .iter()
            .find(|(id, _)| id.speaker.as_str() == speaker_str);

        match matching_id {
            Some((id, _)) => {
                let mut participant = Participant::new(entry.clone(), (*id).clone());

                if let Some((_, date, _)) = births
                    .iter()
                    .find(|(speaker, _, _)| speaker.as_str() == speaker_str)
                {
                    participant = participant.with_birth_date((*date).clone());
                }

                participants.insert(speaker_code, participant);
            }
            None => {
                diagnostics.push(
                    ParseError::new(
                        ErrorCode::SpeakerNotDefined,
                        Severity::Error,
                        SourceLocation::new(participants_span),
                        ErrorContext::new(speaker_str, 0..speaker_str.len(), speaker_str),
                        format!(
                            "Speaker '{}' declared in @Participants but has no matching @ID header",
                            speaker_str
                        ),
                    )
                    .with_suggestion(format!(
                        "Add @ID header: @ID:\t<lang>|<corpus>|{}|<age>|<sex>|<group>|<ses>|{}|<edu>|<custom>|",
                        speaker_str, entry.role
                    )),
                );
            }
        }
    }

    for (id, id_span) in &id_headers {
        if !participants.contains_key(&id.speaker) {
            let speaker_str = id.speaker.to_string();
            diagnostics.push(
                ParseError::new(
                    ErrorCode::OrphanIDHeader,
                    Severity::Error,
                    SourceLocation::new(*id_span),
                    ErrorContext::new(&speaker_str, 0..speaker_str.len(), &speaker_str),
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

    for (speaker, _, birth_span) in &births {
        if !participants.contains_key(*speaker) {
            diagnostics.push(
                ParseError::new(
                    ErrorCode::BirthUnknownParticipant,
                    Severity::Error,
                    SourceLocation::new(*birth_span),
                    ErrorContext::new(speaker.as_str(), 0..speaker.len(), speaker.as_str()),
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
