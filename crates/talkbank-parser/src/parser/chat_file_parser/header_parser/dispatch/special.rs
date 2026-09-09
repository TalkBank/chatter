//! Per-kind parsing for option-driven and mixed-shape headers.
//!
//! Each function here is the LEVEL-2 entry for one `HeaderChoice` special
//! variant, reading the generated typed slots (`extract_<kind>(node).child_N`).
//!
//! The special family is more varied than the simple scalars:
//!
//! - `@Number` / `@Recording Quality` / `@Transcription`: one option content
//!   child (`child_2`), so each is one call to [`simple_header`] with its own
//!   words and a typed `from_text` constructor, exactly like the simple family.
//! - `@Birth of` / `@Birthplace of` / `@L1 of`: TWO content children, a
//!   `speaker` at `child_2` and the value at `child_4` (both model an optional
//!   `header_gap` at `child_1`). [`two_slots`] reads them IN ORDER, speaker
//!   first, and a failed speaker returns its recovery without reading the value
//!   slot, which keeps the diagnostic order the family always had.
//! - `@Comment`: the `text_with_bullets_and_pics` content child (`child_2`) fed
//!   to `parse_bullet_content`. A missing content child reports no diagnostic
//!   of its own and builds `Header::Unknown`, as it always did.
//! - `@Options`: the `options_contents` content child (`child_2`), then an inner
//!   `option_name` walk. Only the OUTER content access is typed; the inner
//!   `option_name` iteration stays a `node.kind()` walk. The generated
//!   `extract_options_contents` models the REPEAT (`child_1: Vec<..>` for the
//!   subsequent flags); migrating the inner walk onto it is deferred, see
//!   `option_flags`.
//!
//! Until 2026-09-08 the six slot-reading headers each matched the slot outcome
//! themselves, and a whole-workspace coverage run showed every one of those
//! recovery arms unreached: the grammar always supplies the content child, and
//! tree-sitter parks a malformed header line in a file-level ERROR node rather
//! than inside the header. The arms live once now, in `dispatch/simple.rs`.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Options_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Number_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Transcription_Header>

use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, BirthOfHeaderNode, BirthplaceOfHeaderNode, ChildSlot, CommentHeaderNode,
    L1OfHeaderNode, NamedKind, NumberHeaderNode, OptionsHeaderNode, RecordingQualityHeaderNode,
    TranscriptionHeaderNode, extract_birth_of_header, extract_birthplace_of_header,
    extract_comment_header, extract_l1_of_header, extract_number_header, extract_options_header,
    extract_recording_quality_header, extract_transcription_header,
};
use crate::model::{self, ChatOptionFlag, Header};
use crate::node_types::*;
use crate::parser::tree_parsing::bullet_content::parse_bullet_content;
use crate::parser::tree_parsing::parser_helpers::{
    HeaderSite, Refused, present, surface_displaced,
};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::simple::simple_header;
use crate::parser::tree_parsing::parser_helpers::{ContentSlot, read_simple_content};

/// `@Comment` -> `Header::Comment`. All bullet content is accepted.
pub(super) fn comment(
    typed: CommentHeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::of(&typed, input);
    // The bullet content child at the typed slot `child_2`. A missing content
    // child reported no diagnostic before the typed traversal either (it fell
    // through to `Header::Unknown` silently), so the non-Present path is
    // likewise silent.
    let children = extract_comment_header(typed);
    let outcome = match present(children.child_2.slot()) {
        Some(content) => ParseOutcome::parsed(Header::Comment {
            content: parse_bullet_content(content.raw_node(), input, errors),
        }),
        None => ParseOutcome::parsed(site.unknown("Missing comment content", None)),
    };
    surface_displaced(&children.unexpected, COMMENT_HEADER, input, errors);
    outcome
}

/// `@Number` -> `Header::Number`. All values accepted; validator flags unsupported.
pub(super) fn number(
    typed: NumberHeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::of(&typed, input);
    let children = extract_number_header(typed);
    simple_header(
        &site,
        children.child_2.slot(),
        &children.unexpected,
        &ContentSlot {
            missing: "Missing @Number option",
            suggested_fix: Some("Use @Number:\t1|2|3|4|5|more|audience"),
        },
        errors,
        |option_text| Header::Number {
            number: talkbank_model::model::Number::from_text(&option_text),
        },
    )
}

/// `@Recording Quality` -> `Header::RecordingQuality`. All values accepted;
/// validator flags unsupported.
pub(super) fn recording_quality(
    typed: RecordingQualityHeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::of(&typed, input);
    let children = extract_recording_quality_header(typed);
    simple_header(
        &site,
        children.child_2.slot(),
        &children.unexpected,
        &ContentSlot {
            missing: "Missing @Recording Quality option",
            suggested_fix: Some("Use @Recording Quality:\t1|2|3|4|5"),
        },
        errors,
        |option_text| Header::RecordingQuality {
            quality: talkbank_model::model::RecordingQuality::from_text(&option_text),
        },
    )
}

/// `@Transcription` -> `Header::Transcription`. All values accepted; validator
/// flags unsupported.
pub(super) fn transcription(
    typed: TranscriptionHeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::of(&typed, input);
    let children = extract_transcription_header(typed);
    simple_header(
        &site,
        children.child_2.slot(),
        &children.unexpected,
        &ContentSlot {
            missing: "Missing @Transcription option",
            suggested_fix: Some("Use a valid @Transcription option value"),
        },
        errors,
        |option_text| Header::Transcription {
            transcription: talkbank_model::model::Transcription::from_text(&option_text),
        },
    )
}

/// The two content slots of a `@X of` header, read in order: the speaker
/// first, then the value, and a speaker that fails yields its recovery
/// without the value slot being read, so the diagnostics arrive in the order
/// the family always gave them.
fn two_slots<'tree, 'w, S: AsRawNode<'tree> + NamedKind, V: AsRawNode<'tree> + NamedKind>(
    site: &HeaderSite<'tree, '_>,
    speaker: &ChildSlot<'tree, S>,
    speaker_words: &'w ContentSlot<'w>,
    value: &ChildSlot<'tree, V>,
    value_words: &'w ContentSlot<'w>,
    errors: &impl ErrorSink,
) -> Result<(String, String), Refused<'w>> {
    let speaker = read_simple_content(site, speaker, speaker_words, errors)?;
    let value = read_simple_content(site, value, value_words, errors)?;
    Ok((speaker, value))
}

/// `@Birth of` -> `Header::Birth`.
pub(super) fn birth_of(
    typed: BirthOfHeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::of(&typed, input);
    let children = extract_birth_of_header(typed);
    let header = match two_slots(
        &site,
        children.child_2.slot(),
        &ContentSlot {
            missing: "Missing participant code in @Birth of header",
            suggested_fix: None,
        },
        children.child_4.slot(),
        &ContentSlot {
            missing: "Missing date value in @Birth of header",
            suggested_fix: None,
        },
        errors,
    ) {
        Ok((participant, date)) => Header::Birth {
            participant: model::SpeakerCode::new(participant),
            date: model::ChatDate::new(date),
        },
        Err(refused) => refused.into_header(&site),
    };
    surface_displaced(&children.unexpected, BIRTH_OF_HEADER, input, errors);
    ParseOutcome::parsed(header)
}

/// `@Birthplace of` -> `Header::Birthplace`.
pub(super) fn birthplace_of(
    typed: BirthplaceOfHeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::of(&typed, input);
    let children = extract_birthplace_of_header(typed);
    let header = match two_slots(
        &site,
        children.child_2.slot(),
        &ContentSlot {
            missing: "Missing participant code in @Birthplace of header",
            suggested_fix: None,
        },
        children.child_4.slot(),
        &ContentSlot {
            missing: "Missing place value in @Birthplace of header",
            suggested_fix: None,
        },
        errors,
    ) {
        Ok((participant, place)) => Header::Birthplace {
            participant: model::SpeakerCode::new(participant),
            place: model::BirthplaceDescription::new(place),
        },
        Err(refused) => refused.into_header(&site),
    };
    surface_displaced(&children.unexpected, BIRTHPLACE_OF_HEADER, input, errors);
    ParseOutcome::parsed(header)
}

/// `@L1 of` -> `Header::L1Of`.
pub(super) fn l1_of(
    typed: L1OfHeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::of(&typed, input);
    let children = extract_l1_of_header(typed);
    let participant_words = ContentSlot {
        missing: "Missing participant code in @L1 of header",
        suggested_fix: None,
    };
    let language_words = ContentSlot {
        missing: "Missing language value in @L1 of header",
        suggested_fix: None,
    };
    let slots = two_slots(
        &site,
        children.child_2.slot(),
        &participant_words,
        children.child_4.slot(),
        &language_words,
        errors,
    );
    surface_displaced(&children.unexpected, L1_OF_HEADER, input, errors);
    let (participant, language) = match slots {
        Ok(slots) => slots,
        Err(refused) => return ParseOutcome::parsed(refused.into_header(&site)),
    };
    // @L1 of values are ISO 639-3 codes (typed model migration,
    // 2026-07-16); an empty value cannot form a code and falls back to
    // an unknown header carrying the reason, like the missing-value path.
    let language = match model::LanguageCode::new(language) {
        Ok(code) => code,
        Err(_empty) => {
            return ParseOutcome::parsed(
                site.unknown("Empty language value in @L1 of header", None),
            );
        }
    };
    ParseOutcome::parsed(Header::L1Of {
        participant: model::SpeakerCode::new(participant),
        language,
    })
}

/// `@Options` -> `Header::Options`.
pub(super) fn options(
    typed: OptionsHeaderNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    // The options_contents child at the typed slot `child_2`. An absent
    // options_contents gave an empty flag list before the typed traversal and
    // a MISSING node has no children (also empty), so every non-Present state
    // yields an empty list. Validation reports E533 on the resulting empty
    // @Options list downstream.
    let children = extract_options_header(typed);
    let flags = match present(children.child_2.slot()) {
        Some(contents) => option_flags(contents.raw_node(), input),
        None => Vec::new(),
    };
    surface_displaced(&children.unexpected, OPTIONS_HEADER, input, errors);
    ParseOutcome::parsed(Header::Options {
        options: flags.into(),
    })
}

/// Iterate the `option_name` children of an `options_contents` node into typed
/// `ChatOptionFlag` values.
///
/// The inner iteration stays a `node.kind()` walk (not the generated typed
/// repeat slot): this raw walk already enumerates EVERY `option_name` child in
/// document order, so migrating it onto the typed `Vec` shape is a pure
/// refactor still owed. All values are accepted; unsupported ones are flagged
/// by the validator. An empty `option_name` (from grammar recovery for
/// `@Options:\t`) is skipped, leaving an empty flag list that validation
/// reports as E533.
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
