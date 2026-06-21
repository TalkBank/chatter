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
            Line::Header { header, span } => match rewrite_header(header.as_ref(), mapping) {
                HeaderRewrite::Keep(h) => new_lines.push(Line::Header {
                    header: Box::new(h),
                    span: *span,
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
                    Some(SpeakerAssignment::Rename { code, role }) => {
                        new_entries.push(ParticipantEntry {
                            speaker_code: code.clone(),
                            name: entry.name.clone(),
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
            Some(SpeakerAssignment::Rename { code, role }) => {
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
