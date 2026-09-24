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
//!   `option_name` slots, including the generated repeat for subsequent flags.
//!   Missing option names remain recovery, not empty unsupported flags.
//!
//! The three single-value option headers and three participant/value headers
//! retain producer-bound source identity through their payload projections.
//! Comment retains association through body admission, then uses the existing
//! bullet-text leaf adapter. Options remains transitional. Malformed-slot recovery stays;
//! finite coverage observations do not prove those states impossible.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Options_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Number_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Transcription_Header>

use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, BirthOfHeaderNode, BirthplaceOfHeaderNode, CommentHeaderNode, KindSlot,
    L1OfHeaderNode, NamedKind, NoChild, NumberHeaderNode, OptionNameNode, OptionsContentsNode,
    OptionsHeaderNode, RecordingQualityHeaderNode, SourceBound, SourceBoundKind, SourceField,
    SourceSlotView, SpeakerNode, TranscriptionHeaderNode, extract_options_contents,
    extract_options_header,
};
use crate::model::{self, ChatOptionFlag, Header};
use crate::node_types::*;
use crate::parser::tree_parsing::bullet_content::parse_bullet_content;
use crate::parser::tree_parsing::parser_helpers::{
    HeaderSite, Refused, present, surface_displaced,
};
use talkbank_model::ParseOutcome;

use super::simple::simple_header;
use crate::parser::tree_parsing::parser_helpers::{ContentSlot, read_source_content};

/// `@Comment` -> `Header::Comment`. All bullet content is accepted.
pub(super) fn comment<'tree>(
    typed: SourceBound<'tree, '_, CommentHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    // The bullet content child at the typed slot `child_2`. A missing content
    // child reported no diagnostic before the typed traversal either (it fell
    // through to `Header::Unknown` silently), so the non-Present path is
    // likewise silent.
    let children = typed.extract();
    let header = match children.field_child_2().slot().view() {
        SourceSlotView::Present(content) => {
            match crate::parser::typed_cst::read_source_field(content, errors) {
                Some(content) => Header::Comment {
                    // Bullet-text lowering is still a transitional leaf adapter;
                    // its node and source come from this single admitted body.
                    content: parse_bullet_content(content.into(), errors),
                },
                None => site.unknown("Unreadable comment content", None),
            }
        }
        SourceSlotView::Missing(_) | SourceSlotView::Error(_) | SourceSlotView::Absent(NoChild) => {
            site.unknown("Missing comment content", None)
        }
    };
    surface_displaced(
        &children.children().unexpected,
        COMMENT_HEADER,
        typed.source(),
        errors,
    );
    ParseOutcome::parsed(header)
}

/// `@Number` -> `Header::Number`. All values accepted; validator flags unsupported.
pub(super) fn number<'tree>(
    typed: SourceBound<'tree, '_, NumberHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let children = typed.extract();
    simple_header(
        &site,
        children.field_child_2().slot(),
        &children.children().unexpected,
        &ContentSlot {
            missing: "Missing @Number option",
            suggested_fix: Some("Use @Number:\t1|2|3|4|5|more|audience"),
        },
        errors,
        |option_text| Header::Number {
            number: talkbank_model::model::Number::from_text(option_text),
        },
    )
}

/// `@Recording Quality` -> `Header::RecordingQuality`. All values accepted;
/// validator flags unsupported.
pub(super) fn recording_quality<'tree>(
    typed: SourceBound<'tree, '_, RecordingQualityHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let children = typed.extract();
    simple_header(
        &site,
        children.field_child_2().slot(),
        &children.children().unexpected,
        &ContentSlot {
            missing: "Missing @Recording Quality option",
            suggested_fix: Some("Use @Recording Quality:\t1|2|3|4|5"),
        },
        errors,
        |option_text| Header::RecordingQuality {
            quality: talkbank_model::model::RecordingQuality::from_text(option_text),
        },
    )
}

/// `@Transcription` -> `Header::Transcription`. All values accepted; validator
/// flags unsupported.
pub(super) fn transcription<'tree>(
    typed: SourceBound<'tree, '_, TranscriptionHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let children = typed.extract();
    simple_header(
        &site,
        children.field_child_2().slot(),
        &children.children().unexpected,
        &ContentSlot {
            missing: "Missing @Transcription option",
            suggested_fix: Some("Use a valid @Transcription option value"),
        },
        errors,
        |option_text| Header::Transcription {
            transcription: talkbank_model::model::Transcription::from_text(option_text),
        },
    )
}

/// Both source fields were admitted, but value semantics remain unvalidated.
/// Participant identity and header payload cannot be interchanged as strings.
struct ParticipantValue<'source> {
    participant: model::SpeakerCode,
    value: &'source str,
}

/// Read speaker first; refusal returns without reading the value, preserving
/// diagnostic order. Only the generated speaker kind can supply participant text.
fn two_slots<'tree, 'source, 'w, V: SourceBoundKind<'tree> + NamedKind>(
    site: &HeaderSite<'tree, '_>,
    speaker: SourceField<'_, 'tree, 'source, KindSlot<'tree, SpeakerNode<'tree>>>,
    speaker_words: &'w ContentSlot<'w>,
    value: SourceField<'_, 'tree, 'source, KindSlot<'tree, V>>,
    value_words: &'w ContentSlot<'w>,
    errors: &impl ErrorSink,
) -> Result<ParticipantValue<'source>, Refused<'w>> {
    let speaker = read_source_content(site, speaker, speaker_words, errors)?;
    let value = read_source_content(site, value, value_words, errors)?;
    Ok(ParticipantValue {
        participant: model::SpeakerCode::new(speaker),
        value,
    })
}

/// `@Birth of` -> `Header::Birth`.
pub(super) fn birth_of<'tree>(
    typed: SourceBound<'tree, '_, BirthOfHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let children = typed.extract();
    let header = match two_slots(
        &site,
        children.field_child_2().slot(),
        &ContentSlot {
            missing: "Missing participant code in @Birth of header",
            suggested_fix: None,
        },
        children.field_child_4().slot(),
        &ContentSlot {
            missing: "Missing date value in @Birth of header",
            suggested_fix: None,
        },
        errors,
    ) {
        Ok(ParticipantValue {
            participant,
            value: date,
        }) => Header::Birth {
            participant,
            date: model::ChatDate::new(date),
        },
        Err(refused) => refused.into_header(&site),
    };
    surface_displaced(
        &children.children().unexpected,
        BIRTH_OF_HEADER,
        typed.source(),
        errors,
    );
    ParseOutcome::parsed(header)
}

/// `@Birthplace of` -> `Header::Birthplace`.
pub(super) fn birthplace_of<'tree>(
    typed: SourceBound<'tree, '_, BirthplaceOfHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let children = typed.extract();
    let header = match two_slots(
        &site,
        children.field_child_2().slot(),
        &ContentSlot {
            missing: "Missing participant code in @Birthplace of header",
            suggested_fix: None,
        },
        children.field_child_4().slot(),
        &ContentSlot {
            missing: "Missing place value in @Birthplace of header",
            suggested_fix: None,
        },
        errors,
    ) {
        Ok(ParticipantValue {
            participant,
            value: place,
        }) => Header::Birthplace {
            participant,
            place: model::BirthplaceDescription::new(place),
        },
        Err(refused) => refused.into_header(&site),
    };
    surface_displaced(
        &children.children().unexpected,
        BIRTHPLACE_OF_HEADER,
        typed.source(),
        errors,
    );
    ParseOutcome::parsed(header)
}

/// `@L1 of` -> `Header::L1Of`.
pub(super) fn l1_of<'tree>(
    typed: SourceBound<'tree, '_, L1OfHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let children = typed.extract();
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
        children.field_child_2().slot(),
        &participant_words,
        children.field_child_4().slot(),
        &language_words,
        errors,
    );
    surface_displaced(
        &children.children().unexpected,
        L1_OF_HEADER,
        typed.source(),
        errors,
    );
    let ParticipantValue {
        participant,
        value: language,
    } = match slots {
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
        participant,
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
        Some(contents) => option_flags(*contents, input, errors),
        None => Vec::new(),
    };
    surface_displaced(&children.unexpected, OPTIONS_HEADER, input, errors);
    ParseOutcome::parsed(Header::Options {
        options: flags.into(),
    })
}

/// Admit only present option-name slots, in generated grammar order.
///
/// An option-name wrapper may be present around a zero-width missing leaf.
/// Keep the empty-text recovery check until the generated producer admits
/// only nonempty descendants. Recovery slots stay owned by the whole-tree backstop, while
/// each generated carrier's displaced nodes are surfaced here.
fn option_flags(
    options_contents: OptionsContentsNode<'_>,
    input: &str,
    errors: &impl ErrorSink,
) -> Vec<ChatOptionFlag> {
    let children = extract_options_contents(options_contents);
    let mut flags = Vec::new();
    let mut admit = |slot: &KindSlot<'_, OptionNameNode<'_>>| {
        if let Some(name) = present(slot)
            && let Ok(text) = name.raw_node().utf8_text(input.as_bytes())
            && !text.is_empty()
        {
            flags.push(ChatOptionFlag::from_text(text));
        }
    };
    admit(children.child_0.slot());
    for element in children.child_1.slot() {
        if let Some(sequence) = present(element.slot()) {
            admit(sequence.child_2.slot());
            surface_displaced(&sequence.unexpected, OPTIONS_CONTENTS, input, errors);
        }
    }
    surface_displaced(&children.unexpected, OPTIONS_CONTENTS, input, errors);
    flags
}
