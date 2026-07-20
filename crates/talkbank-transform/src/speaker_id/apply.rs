//! Apply a [`MappingSpec`] to a CHAT file: rewrite speaker codes,
//! drop/rename `@Participants` entries and `@ID` rows accordingly.

use talkbank_model::ParseValidateOptions;
use talkbank_model::model::header::{Header, ParticipantEntries, ParticipantEntry};
use talkbank_model::model::{ChatFile, Line};

use crate::pipeline::parse_and_validate;
use crate::serialize::to_chat_string;

use super::error::SpeakerIdError;
use super::mapping::{MappingSpec, SpeakerAssignment};

/// Apply a [`MappingSpec`] to `content`, returning the relabeled CHAT
/// text. Utterances for dropped speakers are removed; utterances for
/// renamed speakers have their main-tier speaker prefix rewritten
/// (everything else byte-stable). `@Participants` and `@ID` headers
/// reconcile per the contract in `speaker-id.md`.
///
/// Speakers present in the input but absent from the mapping are
/// currently passed through unchanged. Enforcing "every input speaker
/// must be in the mapping" as an explicit-mode precondition is a
/// follow-up.
pub fn apply_mapping(
    content: &str,
    mapping: &MappingSpec,
    options: ParseValidateOptions,
) -> Result<String, SpeakerIdError> {
    let chat = parse_and_validate(content, options)?;
    Ok(apply_mapping_chat(&chat, mapping))
}

/// Apply a [`MappingSpec`] to an already-parsed [`ChatFile`]. Used
/// when the caller has the parsed AST in hand (e.g. reference mode,
/// which parses the donor once to feed `identify_mapping`), avoids
/// the redundant second parse that the `&str` entry point would
/// otherwise do.
pub fn apply_mapping_chat(chat: &ChatFile, mapping: &MappingSpec) -> String {
    let mut new_lines: Vec<Line> = Vec::new();

    for line in chat.lines.0.iter() {
        match line {
            Line::Utterance(u) => match mapping.get(&u.main.speaker) {
                Some(SpeakerAssignment::Drop) => { /* skip */ }
                Some(SpeakerAssignment::Rename { code, .. }) => {
                    let mut cloned = u.as_ref().clone();
                    cloned.main.speaker = code.clone();
                    new_lines.push(Line::Utterance(Box::new(cloned)));
                }
                None => new_lines.push(line.clone()),
            },
            Line::Header {
                header,
                span,
                separator,
            } => match rewrite_header(header.as_ref(), mapping) {
                HeaderRewrite::Keep(h) => new_lines.push(Line::Header {
                    header: Box::new(h),
                    span: *span,
                    separator: *separator,
                }),
                HeaderRewrite::Drop => { /* skip */ }
            },
        }
    }

    to_chat_string(&ChatFile::new(new_lines))
}

/// Outcome of applying the mapping to a single header line.
//
// `large_enum_variant`: Header is ~256 bytes, Drop is 0. Boxing the
// Keep payload would add an allocation per header. Since this enum is
// constructed transiently inside `rewrite_header()` and consumed
// immediately by the caller, the inline-size cost is acceptable and
// the allocation cost of boxing is not worth saving 256 bytes of
// stack residency in a one-shot.
#[allow(clippy::large_enum_variant)]
enum HeaderRewrite {
    /// Replace the line with this (possibly rewritten) header.
    Keep(Header),
    /// Remove the line entirely (e.g. `@ID` row for a dropped speaker).
    Drop,
}

/// Rewrite a single header per the mapping. `@Participants` filters +
/// rewrites its entry list; `@ID` rewrites code + role or drops; every
/// other header is pass-through.
fn rewrite_header(header: &Header, mapping: &MappingSpec) -> HeaderRewrite {
    match header {
        Header::Participants { entries } => {
            let mut new_entries: Vec<ParticipantEntry> = Vec::new();
            for entry in entries.iter() {
                match mapping.get(&entry.speaker_code) {
                    Some(SpeakerAssignment::Drop) => { /* skip */ }
                    Some(SpeakerAssignment::Rename {
                        code,
                        role,
                        specific_role,
                    }) => {
                        new_entries.push(ParticipantEntry {
                            speaker_code: code.clone(),
                            name: specific_role.clone().or_else(|| entry.name.clone()),
                            role: role.clone(),
                        });
                    }
                    None => new_entries.push(entry.clone()),
                }
            }
            HeaderRewrite::Keep(Header::Participants {
                entries: ParticipantEntries::new(new_entries),
            })
        }
        Header::ID(id) => match mapping.get(&id.speaker) {
            Some(SpeakerAssignment::Drop) => HeaderRewrite::Drop,
            Some(SpeakerAssignment::Rename { code, role, .. }) => {
                let mut new_id = id.clone();
                new_id.speaker = code.clone();
                new_id.role = role.clone();
                HeaderRewrite::Keep(Header::ID(new_id))
            }
            None => HeaderRewrite::Keep(Header::ID(id.clone())),
        },
        other => HeaderRewrite::Keep(other.clone()),
    }
}

#[cfg(test)]
mod tests {
    use talkbank_model::{ParticipantName, ParticipantRole, SpeakerCode};

    use super::*;

    const FIX_TWO_DONOR_SPEAKERS: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR0 Adult, PAR1 Adult
@ID:\teng|corpus|PAR0|||||Adult|||
@ID:\teng|corpus|PAR1|||||Adult|||
@Media:\tspecific-role, audio
*PAR0:\thello there . \u{15}0_1000\u{15}
*PAR1:\thi yourself . \u{15}1000_2000\u{15}
@End
";

    /// `Rename` with `specific_role: Some(...)` must use it as the
    /// `@Participants` name/specific-role field, overriding whatever the
    /// donor's original entry carried (nothing, here).
    #[test]
    fn rename_with_specific_role_sets_participant_name() {
        let mut mapping = MappingSpec::new();
        mapping.insert(
            SpeakerCode::new("PAR0"),
            SpeakerAssignment::Rename {
                code: SpeakerCode::new("INV1"),
                role: ParticipantRole::new("Investigator"),
                specific_role: Some(ParticipantName::new("First_Investigator")),
            },
        );
        mapping.insert(
            SpeakerCode::new("PAR1"),
            SpeakerAssignment::Rename {
                code: SpeakerCode::new("INV2"),
                role: ParticipantRole::new("Investigator"),
                specific_role: Some(ParticipantName::new("Second_Investigator")),
            },
        );
        let result = apply_mapping(
            FIX_TWO_DONOR_SPEAKERS,
            &mapping,
            talkbank_model::ParseValidateOptions::default(),
        )
        .expect("apply_mapping should succeed");

        assert!(
            result.contains("INV1 First_Investigator Investigator"),
            "expected INV1's @Participants entry to carry the specific-role label.\n{result}"
        );
        assert!(
            result.contains("INV2 Second_Investigator Investigator"),
            "expected INV2's @Participants entry to carry the specific-role label.\n{result}"
        );
    }

    /// `Rename` with `specific_role: None` must fall back to the donor's
    /// original `@Participants` name field (today, `None`), matching the
    /// pre-existing single-role behavior exactly.
    #[test]
    fn rename_without_specific_role_preserves_donor_name() {
        let mut mapping = MappingSpec::new();
        mapping.insert(
            SpeakerCode::new("PAR0"),
            SpeakerAssignment::Rename {
                code: SpeakerCode::new("INV"),
                role: ParticipantRole::new("Investigator"),
                specific_role: None,
            },
        );
        mapping.insert(SpeakerCode::new("PAR1"), SpeakerAssignment::Drop);
        let result = apply_mapping(
            FIX_TWO_DONOR_SPEAKERS,
            &mapping,
            talkbank_model::ParseValidateOptions::default(),
        )
        .expect("apply_mapping should succeed");

        assert!(
            result.contains("INV Investigator") && !result.contains("INV_"),
            "expected plain 'INV Investigator' with no specific-role label.\n{result}"
        );
    }
}
