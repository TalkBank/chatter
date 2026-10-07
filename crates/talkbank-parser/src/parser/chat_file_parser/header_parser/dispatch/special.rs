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
    AsRawNode, BirthOfHeaderNode, BirthplaceOfHeaderNode, CommentHeaderNode, L1OfHeaderNode,
    NamedKind, NarrowedKindSlot, NoChild, NumberHeaderNode, OptionNameNode, OptionsContentsNode,
    OptionsHeaderNode, RecordingQualityHeaderNode, SourceBound, SourceBoundKind, SourceField,
    SourceRecovery, SourceSlotView, SpeakerNode, TranscriptionHeaderNode, extract_options_contents,
    extract_options_header,
};
use crate::model::{self, ChatOptionFlag, Header};
use crate::node_types::*;
use crate::parser::tree_parsing::bullet_content::parse_bullet_content;
use crate::parser::tree_parsing::parser_helpers::{
    ContentReadError, HeaderSite, extract_utf8_text, present, surface_displaced,
};
use talkbank_model::ParseOutcome;

use super::simple::simple_header;
use crate::parser::tree_parsing::parser_helpers::{ContentSlot, read_source_content};
use crate::parser::typed_cst::AnyKindSlot;

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
    let Ok(children) = crate::parser::typed_cst::report_reconstruction(
        typed.extract(),
        typed.raw_node(),
        typed.source(),
        errors,
    ) else {
        return ParseOutcome::Rejected;
    };
    let header = match children.field_child_2().slot().view() {
        SourceSlotView::Present(content) => {
            match crate::parser::typed_cst::read_source_field(content, errors) {
                Some(content) => Header::Comment {
                    // Bullet-text lowering is still a transitional leaf adapter;
                    // its node and source come from this single admitted body.
                    content: match parse_bullet_content(content.into(), errors) {
                        Ok(content) => content,
                        Err(fault) => {
                            crate::parser::typed_cst::report_cst_failure(
                                content.raw_node(),
                                content.source(),
                                fault,
                                errors,
                            );
                            return ParseOutcome::Rejected;
                        }
                    },
                },
                None => return ParseOutcome::Rejected,
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
    let Ok(children) = crate::parser::typed_cst::report_reconstruction(
        typed.extract(),
        typed.raw_node(),
        typed.source(),
        errors,
    ) else {
        return ParseOutcome::Rejected;
    };
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
    let Ok(children) = crate::parser::typed_cst::report_reconstruction(
        typed.extract(),
        typed.raw_node(),
        typed.source(),
        errors,
    ) else {
        return ParseOutcome::Rejected;
    };
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
    let Ok(children) = crate::parser::typed_cst::report_reconstruction(
        typed.extract(),
        typed.raw_node(),
        typed.source(),
        errors,
    ) else {
        return ParseOutcome::Rejected;
    };
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
/// Generic over each slot's `Missing` payload, as `read_source_content` is.
fn two_slots<'value, 'tree: 'value, 'source, 'w, V, MS, MV>(
    site: &HeaderSite<'tree, '_>,
    speaker: SourceField<'value, 'tree, 'source, AnyKindSlot<'tree, SpeakerNode<'tree>, MS>>,
    speaker_words: &'w ContentSlot<'w>,
    value: SourceField<'value, 'tree, 'source, AnyKindSlot<'tree, V, MV>>,
    value_words: &'w ContentSlot<'w>,
    errors: &impl ErrorSink,
) -> Result<ParticipantValue<'source>, ContentReadError<'w>>
where
    V: SourceBoundKind<'tree> + NamedKind,
    MS: SourceRecovery<'value, 'tree, 'source>,
    MV: SourceRecovery<'value, 'tree, 'source>,
{
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
    let Ok(children) = crate::parser::typed_cst::report_reconstruction(
        typed.extract(),
        typed.raw_node(),
        typed.source(),
        errors,
    ) else {
        return ParseOutcome::Rejected;
    };
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
        }) => ParseOutcome::parsed(Header::Birth {
            participant,
            date: model::ChatDate::new(date),
        }),
        Err(failure) => failure.into_outcome(&site, errors),
    };
    surface_displaced(
        &children.children().unexpected,
        BIRTH_OF_HEADER,
        typed.source(),
        errors,
    );
    header
}

/// `@Birthplace of` -> `Header::Birthplace`.
pub(super) fn birthplace_of<'tree>(
    typed: SourceBound<'tree, '_, BirthplaceOfHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let Ok(children) = crate::parser::typed_cst::report_reconstruction(
        typed.extract(),
        typed.raw_node(),
        typed.source(),
        errors,
    ) else {
        return ParseOutcome::Rejected;
    };
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
        }) => ParseOutcome::parsed(Header::Birthplace {
            participant,
            place: model::BirthplaceDescription::new(place),
        }),
        Err(failure) => failure.into_outcome(&site, errors),
    };
    surface_displaced(
        &children.children().unexpected,
        BIRTHPLACE_OF_HEADER,
        typed.source(),
        errors,
    );
    header
}

/// `@L1 of` -> `Header::L1Of`.
pub(super) fn l1_of<'tree>(
    typed: SourceBound<'tree, '_, L1OfHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let Ok(children) = crate::parser::typed_cst::report_reconstruction(
        typed.extract(),
        typed.raw_node(),
        typed.source(),
        errors,
    ) else {
        return ParseOutcome::Rejected;
    };
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
        Err(failure) => return failure.into_outcome(&site, errors),
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
    let Ok(children) = crate::parser::typed_cst::report_reconstruction(
        extract_options_header(typed),
        typed.raw_node(),
        input,
        errors,
    ) else {
        return ParseOutcome::Rejected;
    };
    let flags = match present(children.child_2.slot()) {
        Some(contents) => match crate::parser::typed_cst::report_reconstruction(
            option_flags(*contents, input, errors),
            typed.raw_node(),
            input,
            errors,
        ) {
            Ok(ParseOutcome::Parsed(flags)) => flags,
            Ok(ParseOutcome::Rejected) | Err(_) => return ParseOutcome::Rejected,
        },
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
) -> Result<ParseOutcome<Vec<ChatOptionFlag>>, crate::generated_traversal::ReconstructionFault> {
    let children = extract_options_contents(options_contents)?;
    let mut flags = Vec::new();
    let mut admit = |slot: &NarrowedKindSlot<'_, OptionNameNode<'_>>| -> ParseOutcome<()> {
        if let Some(name) = present(slot) {
            let ParseOutcome::Parsed(text) =
                extract_utf8_text(name.raw_node(), input, errors, "option name")
            else {
                return ParseOutcome::Rejected;
            };
            if !text.is_empty() {
                flags.push(ChatOptionFlag::from_text(text));
            }
        }
        ParseOutcome::parsed(())
    };
    if admit(children.child_0.slot()).is_none() {
        return Ok(ParseOutcome::Rejected);
    }
    for element in children.child_1.slot() {
        if let Some(sequence) = present(element.slot()) {
            if admit(sequence.child_2.slot()).is_none() {
                return Ok(ParseOutcome::Rejected);
            }
            surface_displaced(&sequence.unexpected, OPTIONS_CONTENTS, input, errors);
        }
    }
    surface_displaced(&children.unexpected, OPTIONS_CONTENTS, input, errors);
    Ok(ParseOutcome::parsed(flags))
}

#[cfg(test)]
mod option_admission_tests {
    use super::*;
    use crate::generated_traversal::FromNodeKind;

    #[test]
    fn unreadable_option_names_reject_instead_of_becoming_empty_flags() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/annotation/long-features.cha"
        ));
        let parser = crate::TreeSitterParser::new().expect("grammar");
        let parsed = parser
            .parse_source_incremental(source, None)
            .expect("reference");
        let mut pending = vec![parsed.root_node()];
        let mut witnessed = 0;
        while let Some(node) = pending.pop() {
            let mut cursor = node.walk();
            pending.extend(node.children(&mut cursor));
            let Some(contents) = OptionsContentsNode::from_node(node) else {
                continue;
            };
            let errors = talkbank_model::ErrorCollector::new();
            assert!(
                option_flags(contents, source, &errors)
                    .expect("producer")
                    .is_some()
            );
            assert!(errors.into_vec().is_empty());
            let errors = talkbank_model::ErrorCollector::new();
            assert!(
                option_flags(contents, "", &errors)
                    .expect("producer")
                    .is_none()
            );
            let findings = errors.into_vec();
            assert_eq!(findings.len(), 1);
            assert_eq!(findings[0].code, talkbank_model::ErrorCode::InternalError);
            assert!(talkbank_model::CompletedDiagnostics::admit(findings).is_err());
            witnessed += 1;
        }
        assert!(witnessed > 0);
    }
}
