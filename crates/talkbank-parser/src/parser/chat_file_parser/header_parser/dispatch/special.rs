//! Per-kind parsing for option-driven and mixed-shape headers.
//!
//! Each function here is the LEVEL-2 entry for one `HeaderChoice` special
//! variant. The INTERNAL content access reads the NEW backend's free, typed,
//! POSITIONAL `extract_<kind>(node).child_N.slot` (replacing the
//! `node.kind()`-based `find_child_by_kind` / `get_required_content_by_kind` /
//! `parse_options_flags` scan, and, as of Task B2, the OLD
//! `TypedTraversal.extract_<kind>` trait-receiver call). The migration is
//! BEHAVIOUR-PRESERVING: each produced `Header` value and every diagnostic stays
//! byte-identical.
//!
//! The special family is more varied than the simple scalars:
//!
//! - `@Number` / `@Recording Quality` / `@Transcription`: one option content
//!   child (`child_2`) decoded through the shared `read_simple_content` helper
//!   (which reproduces `get_required_content_by_kind`'s text + "missing content"
//!   diagnostic exactly), then a typed `from_text`. A `Fallback` slot builds the
//!   same `Header::Unknown` recovery the pre-migration `else` arm built.
//! - `@Birth of` / `@Birthplace of` / `@L1 of`: TWO content children, a
//!   `speaker` at `child_2` and the value at `child_4` (unchanged indices from
//!   the OLD module: both already modeled an optional `header_gap` at `child_1`).
//!   Each is read through `read_simple_content` IN ORDER (speaker first),
//!   preserving the pre-migration early-return-on-missing-participant behaviour
//!   and its diagnostic emission order.
//! - `@Comment`: the `text_with_bullets_and_pics` content child (`child_2`) fed
//!   to `parse_bullet_content`. The pre-migration `find_child_by_kind` reported
//!   NO diagnostic on a missing content child (it silently built
//!   `Header::Unknown`), so the non-`Present` arm here is likewise silent.
//! - `@Options`: the `options_contents` content child (`child_2`), then an inner
//!   `option_name` walk. Only the OUTER content access is migrated to the typed
//!   slot; the inner `option_name` iteration stays a `node.kind()` walk. The NEW
//!   backend's `extract_options_contents` in fact now models the REPEAT
//!   correctly (`child_1: Vec<..>` for the subsequent flags), unlike the OLD
//!   module which slotted only the FIRST `option_name`; migrating the inner walk
//!   onto that typed repeat is deliberately DEFERRED (out of scope for B2: it
//!   would be a behavior-visible change to a currently-correct raw-node walk,
//!   not a pure call-convention migration) rather than folded in here. See
//!   `option_flags`.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Options_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Number_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Transcription_Header>

use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, BirthOfHeaderNode, BirthplaceOfHeaderNode, CommentHeaderNode, L1OfHeaderNode,
    NumberHeaderNode, OptionsHeaderNode, RecordingQualityHeaderNode, TranscriptionHeaderNode,
    extract_birth_of_header, extract_birthplace_of_header, extract_comment_header,
    extract_l1_of_header, extract_number_header, extract_options_header,
    extract_recording_quality_header, extract_transcription_header,
};
use crate::model::{self, ChatOptionFlag, Header};
use crate::node_types::*;
use crate::parser::tree_parsing::bullet_content::parse_bullet_content;
use crate::parser::tree_parsing::parser_helpers::surface_unexpected;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::simple::{SimpleContent, read_simple_content};

/// Construct a best-effort `Header::Unknown` when a special header fails to parse.
fn unknown_header_from_node(
    header_actual: Node,
    input: &str,
    reason: impl Into<String>,
    suggested_fix: Option<&str>,
) -> Header {
    let text = match header_actual.utf8_text(input.as_bytes()) {
        Ok(raw) if !raw.is_empty() => raw.to_string(),
        _ => header_actual.kind().to_string(),
    };

    Header::Unknown {
        text: model::WarningText::new(text),
        parse_reason: Some(reason.into()),
        suggested_fix: suggested_fix.map(str::to_string),
    }
}

/// `@Comment` -> `Header::Comment`. All bullet content is accepted.
pub(super) fn comment(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    // LEVEL 2: read the bullet content child through the typed positional slot
    // (extract_comment_header child_2). `present_or_recover().ok()` keeps only a
    // Present content node; the pre-migration find_child_by_kind reported NO
    // diagnostic for a missing content child (it silently fell through to
    // Header::Unknown), so the non-Present (else) path is likewise SILENT.
    let children = extract_comment_header(CommentHeaderNode(header_actual));
    let outcome = match children.child_2.slot.present_or_recover().ok() {
        Some(content) => ParseOutcome::parsed(Header::Comment {
            content: parse_bullet_content(content.raw_node(), input, errors),
        }),
        None => ParseOutcome::parsed(unknown_header_from_node(
            header_actual,
            input,
            "Missing comment content",
            None,
        )),
    };
    surface_unexpected(&children.unexpected, input, errors);
    outcome
}

/// `@Number` -> `Header::Number`. All values accepted; validator flags unsupported.
pub(super) fn number(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let children = extract_number_header(NumberHeaderNode(header_actual));
    let outcome = match read_simple_content(
        children.child_2.slot,
        header_actual,
        input,
        NUMBER_OPTION,
        NUMBER_HEADER,
        errors,
    ) {
        // All values accepted; unsupported ones flagged by the validator.
        SimpleContent::Decoded(option_text) => ParseOutcome::parsed(Header::Number {
            number: talkbank_model::model::Number::from_text(&option_text),
        }),
        SimpleContent::Fallback => ParseOutcome::parsed(unknown_header_from_node(
            header_actual,
            input,
            "Missing @Number option",
            Some("Use @Number:\t1|2|3|4|5|more|audience"),
        )),
    };
    surface_unexpected(&children.unexpected, input, errors);
    outcome
}

/// `@Recording Quality` -> `Header::RecordingQuality`.
pub(super) fn recording_quality(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let children = extract_recording_quality_header(RecordingQualityHeaderNode(header_actual));
    let outcome = match read_simple_content(
        children.child_2.slot,
        header_actual,
        input,
        RECORDING_QUALITY_OPTION,
        RECORDING_QUALITY_HEADER,
        errors,
    ) {
        // All values accepted; unsupported ones flagged by the validator.
        SimpleContent::Decoded(option_text) => ParseOutcome::parsed(Header::RecordingQuality {
            quality: talkbank_model::model::RecordingQuality::from_text(&option_text),
        }),
        SimpleContent::Fallback => ParseOutcome::parsed(unknown_header_from_node(
            header_actual,
            input,
            "Missing @Recording Quality option",
            Some("Use @Recording Quality:\t1|2|3|4|5"),
        )),
    };
    surface_unexpected(&children.unexpected, input, errors);
    outcome
}

/// `@Transcription` -> `Header::Transcription`.
pub(super) fn transcription(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let children = extract_transcription_header(TranscriptionHeaderNode(header_actual));
    let outcome = match read_simple_content(
        children.child_2.slot,
        header_actual,
        input,
        TRANSCRIPTION_OPTION,
        TRANSCRIPTION_HEADER,
        errors,
    ) {
        // All values accepted; unsupported ones flagged by the validator.
        SimpleContent::Decoded(option_text) => ParseOutcome::parsed(Header::Transcription {
            transcription: talkbank_model::model::Transcription::from_text(&option_text),
        }),
        SimpleContent::Fallback => ParseOutcome::parsed(unknown_header_from_node(
            header_actual,
            input,
            "Missing @Transcription option",
            Some("Use a valid @Transcription option value"),
        )),
    };
    surface_unexpected(&children.unexpected, input, errors);
    outcome
}

/// `@Birth of` -> `Header::Birth`.
pub(super) fn birth_of(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    // Two content children: speaker (child_2) read FIRST, then date_contents
    // (child_4). The speaker read must run (and report) before the date read, and
    // a missing speaker returns early WITHOUT reading the date slot, exactly as
    // the pre-migration two-step `get_required_content_by_kind` chain did.
    let children = extract_birth_of_header(BirthOfHeaderNode(header_actual));
    let participant = match read_simple_content(
        children.child_2.slot,
        header_actual,
        input,
        SPEAKER,
        BIRTH_OF_HEADER,
        errors,
    ) {
        SimpleContent::Decoded(participant) => participant,
        SimpleContent::Fallback => {
            let outcome = ParseOutcome::parsed(unknown_header_from_node(
                header_actual,
                input,
                "Missing participant code in @Birth of header",
                None,
            ));
            surface_unexpected(&children.unexpected, input, errors);
            return outcome;
        }
    };
    let date = match read_simple_content(
        children.child_4.slot,
        header_actual,
        input,
        DATE_CONTENTS,
        BIRTH_OF_HEADER,
        errors,
    ) {
        SimpleContent::Decoded(date) => date,
        SimpleContent::Fallback => {
            let outcome = ParseOutcome::parsed(unknown_header_from_node(
                header_actual,
                input,
                "Missing date value in @Birth of header",
                None,
            ));
            surface_unexpected(&children.unexpected, input, errors);
            return outcome;
        }
    };
    surface_unexpected(&children.unexpected, input, errors);
    ParseOutcome::parsed(Header::Birth {
        participant: model::SpeakerCode::new(participant),
        date: model::ChatDate::new(date),
    })
}

/// `@Birthplace of` -> `Header::Birthplace`.
pub(super) fn birthplace_of(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let children = extract_birthplace_of_header(BirthplaceOfHeaderNode(header_actual));
    let participant = match read_simple_content(
        children.child_2.slot,
        header_actual,
        input,
        SPEAKER,
        BIRTHPLACE_OF_HEADER,
        errors,
    ) {
        SimpleContent::Decoded(participant) => participant,
        SimpleContent::Fallback => {
            let outcome = ParseOutcome::parsed(unknown_header_from_node(
                header_actual,
                input,
                "Missing participant code in @Birthplace of header",
                None,
            ));
            surface_unexpected(&children.unexpected, input, errors);
            return outcome;
        }
    };
    let place = match read_simple_content(
        children.child_4.slot,
        header_actual,
        input,
        FREE_TEXT,
        BIRTHPLACE_OF_HEADER,
        errors,
    ) {
        SimpleContent::Decoded(place) => place,
        SimpleContent::Fallback => {
            let outcome = ParseOutcome::parsed(unknown_header_from_node(
                header_actual,
                input,
                "Missing place value in @Birthplace of header",
                None,
            ));
            surface_unexpected(&children.unexpected, input, errors);
            return outcome;
        }
    };
    surface_unexpected(&children.unexpected, input, errors);
    ParseOutcome::parsed(Header::Birthplace {
        participant: model::SpeakerCode::new(participant),
        place: model::BirthplaceDescription::new(place),
    })
}

/// `@L1 of` -> `Header::L1Of`.
pub(super) fn l1_of(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let children = extract_l1_of_header(L1OfHeaderNode(header_actual));
    let participant = match read_simple_content(
        children.child_2.slot,
        header_actual,
        input,
        SPEAKER,
        L1_OF_HEADER,
        errors,
    ) {
        SimpleContent::Decoded(participant) => participant,
        SimpleContent::Fallback => {
            let outcome = ParseOutcome::parsed(unknown_header_from_node(
                header_actual,
                input,
                "Missing participant code in @L1 of header",
                None,
            ));
            surface_unexpected(&children.unexpected, input, errors);
            return outcome;
        }
    };
    let language = match read_simple_content(
        children.child_4.slot,
        header_actual,
        input,
        LANGUAGE_CODE,
        L1_OF_HEADER,
        errors,
    ) {
        SimpleContent::Decoded(language) => language,
        SimpleContent::Fallback => {
            let outcome = ParseOutcome::parsed(unknown_header_from_node(
                header_actual,
                input,
                "Missing language value in @L1 of header",
                None,
            ));
            surface_unexpected(&children.unexpected, input, errors);
            return outcome;
        }
    };
    surface_unexpected(&children.unexpected, input, errors);
    ParseOutcome::parsed(Header::L1Of {
        participant: model::SpeakerCode::new(participant),
        language: model::LanguageName::new(language),
    })
}

/// `@Options` -> `Header::Options`.
pub(super) fn options(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    // LEVEL 2: read the options_contents child through the typed positional slot
    // (extract_options_header child_2). `present_or_recover().ok()` keeps only a
    // Present options_contents; the pre-migration find_child_by_kind returned
    // None for an absent options_contents (-> empty flag list) and a MISSING node
    // has no children (-> also empty), so every non-Present state yields an empty
    // list. Validation reports E533 on the resulting empty @Options list
    // downstream.
    let children = extract_options_header(OptionsHeaderNode(header_actual));
    let flags = match children.child_2.slot.present_or_recover().ok() {
        Some(contents) => option_flags(contents.raw_node(), input),
        None => Vec::new(),
    };
    surface_unexpected(&children.unexpected, input, errors);
    ParseOutcome::parsed(Header::Options {
        options: flags.into(),
    })
}

/// Iterate the `option_name` children of an `options_contents` node into typed
/// `ChatOptionFlag` values, preserving `parse_options_flags`' inner walk exactly.
///
/// The inner iteration stays a `node.kind()` walk (not the NEW backend's typed
/// repeat slot): this raw walk already correctly enumerates EVERY `option_name`
/// child in document order (unlike the OLD module, whose generated
/// `extract_options_contents` slotted only the FIRST one), so migrating it onto
/// the newly-available typed `Vec` shape is a pure refactor with no behavior
/// upside, deliberately DEFERRED out of scope for B2 (see the module doc
/// comment). All values are accepted; unsupported ones are flagged by the
/// validator. An empty `option_name` (from grammar recovery for `@Options:\t`)
/// is skipped, leaving an empty flag list that validation reports as E533.
fn option_flags(options_contents: Node, input: &str) -> Vec<ChatOptionFlag> {
    let mut flags = Vec::new();
    let mut cursor = options_contents.walk();
    for child in options_contents.children(&mut cursor) {
        if child.kind() == OPTION_NAME
            && let Ok(text) = child.utf8_text(input.as_bytes())
        {
            if text.is_empty() {
                // Empty option_name comes from grammar recovery for "@Options:\t".
                // Represent as empty options list and let validation report E533.
                continue;
            }
            // All values are accepted; unsupported ones are flagged by the validator.
            flags.push(ChatOptionFlag::from_text(text));
        }
    }
    flags
}
