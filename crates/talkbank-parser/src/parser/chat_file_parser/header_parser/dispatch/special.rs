//! Per-kind parsing for option-driven and mixed-shape headers.
//!
//! Each function here is the LEVEL-2 entry for one `HeaderChoice` special
//! variant, reading the generated typed slots (`extract_<kind>(node).child_N`).
//!
//! The special family is more varied than the simple scalars:
//!
//! - `@Number` / `@Recording Quality` / `@Transcription`: one option content
//!   child (`child_2`), so each is one call to [`content_header`] with its
//!   own words and its `TextKind`, exactly like the simple family.
//! - `@Birth of` / `@Birthplace of` / `@L1 of`: TWO content children, a
//!   `speaker` at `child_2` and the value at `child_4` (both model an optional
//!   `header_gap` at `child_1`). Read speaker first; a failed speaker returns
//!   its recovery before reading the value. [`participant_header`] owns the
//!   two slots and builds through the `QualifiedKind` for all three.
//! - `@Comment`: the `text_with_bullets_and_pics` content child (`child_2`) fed
//!   to `parse_bullet_content`. An ERROR where the body belongs reports no
//!   diagnostic of its own (the whole-tree backstop does) and keeps the line
//!   as an unknown header with its text.
//! - `@Options`: the `options_contents` content child (`child_2`), read by the
//!   model's one option-list reader (`ChatOptionFlags::parse`). A list with an
//!   empty position is recovery, never an unsupported flag named `""`.
//!
//! Every content child is optional in the grammar: an omitted or blank one is
//! `Header::EmptyContent`, decided once by `read_body`.
//!
//! The three single-value option headers and three participant/value headers
//! retain producer-bound source identity through their payload projections.
//! Comment retains association through body admission, then uses the existing
//! bullet-text leaf adapter. Ordinary eligible envelopes consume compiled
//! per-rule admission; nested malformed-slot recovery stays broad.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Options_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Number_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Transcription_Header>

use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, BirthOfHeaderNode, BirthplaceOfHeaderNode, CommentHeaderNode, L1OfHeaderNode,
    NamedKind, NumberHeaderNode, OptionsHeaderNode, RecordingQualityHeaderNode, SourceBound,
    SourceBoundKind, SourceField, SourceRecovery, SpeakerNode, TranscriptionHeaderNode,
};
use crate::model::{
    self, ChatOptionFlags, Header, PlainKind, QualifiedKind, StructuredKind, TextKind,
};
use crate::node_types::*;
use crate::parser::tree_parsing::bullet_content::parse_bullet_content;
use crate::parser::tree_parsing::parser_helpers::{
    Body, ContentReadError, ContentSlot, HeaderSite, read_body, read_source_content,
    surface_displaced,
};
use talkbank_model::ParseOutcome;

use super::simple::content_header;
use crate::parser::typed_cst::{AnyKindSlot, AnySelectedKindSlot};

/// Why an `@Comment` whose nonblank body lowered to blank content is refused.
const COMMENT_LOST_CONTENT: &str = "@Comment content lowered to nothing";

/// `@Comment` -> `Header::Comment`. All bullet content that writes something
/// is accepted.
pub(super) fn comment<'tree>(
    typed: SourceBound<'tree, '_, CommentHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    // The ordinary envelope proves this composite bullet body present.
    // Nested bullet recovery and source-read failures remain independent.
    let children = match crate::parser::typed_cst::report_admission(
        crate::parser::typed_cst::extract_envelope(typed),
        site.actual(),
        site.input(),
        errors,
    ) {
        Ok(children) => children,
        Err(_) => return ParseOutcome::Rejected,
    };
    // The body is optional in the grammar; an omitted or blank one is the
    // one empty-content state, refused by validation (E553). An ERROR where
    // the body belongs is unparsable text, the whole-tree backstop's to
    // report, and the line is kept as an unknown header with its text.
    let words = ContentSlot {
        missing: "Missing @Comment content",
        suggested_fix: None,
    };
    let header = match read_body(&site, children.field_child_2().slot(), &words, errors) {
        Ok(Body::Empty(separator)) => {
            PlainKind::Structured(StructuredKind::Comment).empty(separator)
        }
        // Bullet-text lowering is still a transitional leaf adapter; its
        // node and source come from this single admitted body.
        Ok(Body::Written { body, .. }) => {
            let body = body.into_inner();
            match parse_bullet_content(body.into(), errors) {
                Ok(ParseOutcome::Parsed(content)) => {
                    match model::CommentContent::try_from_bullets(content) {
                        Ok(content) => Header::Comment { content },
                        // The body's text is nonblank, so lowering it to content that
                        // writes nothing lost what the author wrote: malformed
                        // content (E316) kept as an unknown header, exactly as re2c
                        // refuses the same line, never a blank comment.
                        Err(model::BlankContent) => {
                            site.refuse_content(COMMENT_LOST_CONTENT, errors)
                        }
                    }
                }
                Ok(ParseOutcome::Rejected) => return ParseOutcome::Rejected,
                Err(fault) => {
                    crate::parser::typed_cst::report_cst_failure(
                        body.raw_node(),
                        body.source(),
                        fault,
                        errors,
                    );
                    return ParseOutcome::Rejected;
                }
            }
        }
        Err(failure) => return failure.into_outcome(&site, errors),
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
    let children = match crate::parser::typed_cst::report_admission(
        crate::parser::typed_cst::extract_envelope(typed),
        site.actual(),
        site.input(),
        errors,
    ) {
        Ok(children) => children,
        Err(_) => return ParseOutcome::Rejected,
    };
    content_header(
        &site,
        children.field_child_2().slot(),
        &children.children().unexpected,
        &ContentSlot {
            missing: "Missing @Number option",
            suggested_fix: Some("Use @Number:\t1|2|3|4|5|more|audience"),
        },
        errors,
        TextKind::Number,
    )
}

/// `@Recording Quality` -> `Header::RecordingQuality`. All values accepted;
/// validator flags unsupported.
pub(super) fn recording_quality<'tree>(
    typed: SourceBound<'tree, '_, RecordingQualityHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let children = match crate::parser::typed_cst::report_admission(
        crate::parser::typed_cst::extract_envelope(typed),
        site.actual(),
        site.input(),
        errors,
    ) {
        Ok(children) => children,
        Err(_) => return ParseOutcome::Rejected,
    };
    content_header(
        &site,
        children.field_child_2().slot(),
        &children.children().unexpected,
        &ContentSlot {
            missing: "Missing @Recording Quality option",
            suggested_fix: Some("Use @Recording Quality:\t1|2|3|4|5"),
        },
        errors,
        TextKind::RecordingQuality,
    )
}

/// `@Transcription` -> `Header::Transcription`. All values accepted; validator
/// flags unsupported.
pub(super) fn transcription<'tree>(
    typed: SourceBound<'tree, '_, TranscriptionHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let children = match crate::parser::typed_cst::report_admission(
        crate::parser::typed_cst::extract_envelope(typed),
        site.actual(),
        site.input(),
        errors,
    ) {
        Ok(children) => children,
        Err(_) => return ParseOutcome::Rejected,
    };
    content_header(
        &site,
        children.field_child_2().slot(),
        &children.children().unexpected,
        &ContentSlot {
            missing: "Missing @Transcription option",
            suggested_fix: Some("Use a valid @Transcription option value"),
        },
        errors,
        TextKind::Transcription,
    )
}

/// Read a participant-qualified header: the speaker first (a refusal
/// returns without reading the value, preserving diagnostic order), then the
/// grammar-optional value once ([`read_body`]), then the kind's own
/// constructor: its empty state naming the participant, or its header for
/// the participant and the value. Only the generated speaker kind can supply
/// participant text. Generic over each slot's recovery payloads, as the slot
/// readers are.
fn participant_header<'value, 'tree: 'value, 'source, 'w, V, MS, ES, AS, MV, EV>(
    site: &HeaderSite<'tree, 'source>,
    speaker: SourceField<'value, 'tree, 'source, AnyKindSlot<SpeakerNode<'tree>, MS, ES, AS>>,
    speaker_words: &'w ContentSlot<'w>,
    value: SourceField<'value, 'tree, 'source, Option<AnySelectedKindSlot<V, MV, EV>>>,
    value_words: &'w ContentSlot<'w>,
    errors: &impl ErrorSink,
    kind: QualifiedKind,
) -> Result<Header, ContentReadError<'source, 'w>>
where
    V: SourceBoundKind<'tree> + NamedKind,
    MS: SourceRecovery<'value, 'tree, 'source>,
    ES: SourceRecovery<'value, 'tree, 'source>,
    AS: crate::generated_traversal::Absence,
    MV: SourceRecovery<'value, 'tree, 'source>,
    EV: SourceRecovery<'value, 'tree, 'source>,
{
    let participant =
        model::SpeakerCode::new(read_source_content(site, speaker, speaker_words, errors)?);
    Ok(match read_body(site, value, value_words, errors)? {
        Body::Empty(separator) => kind.empty(participant, separator),
        Body::Written { text, .. } => kind.header(participant, text),
    })
}

/// `@Birth of` -> `Header::Birth`.
pub(super) fn birth_of<'tree>(
    typed: SourceBound<'tree, '_, BirthOfHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let children = typed.extract();
    let header = match participant_header(
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
        QualifiedKind::Birth,
    ) {
        Ok(header) => ParseOutcome::parsed(header),
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
    let children = typed.extract();
    let header = match participant_header(
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
        QualifiedKind::Birthplace,
    ) {
        Ok(header) => ParseOutcome::parsed(header),
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

/// `@L1 of` -> `Header::L1Of`. The value is an ISO 639-3 code (typed model
/// migration, 2026-07-16); its shape and registry membership are validation's.
pub(super) fn l1_of<'tree>(
    typed: SourceBound<'tree, '_, L1OfHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let children = typed.extract();
    let header = match participant_header(
        &site,
        children.field_child_2().slot(),
        &ContentSlot {
            missing: "Missing participant code in @L1 of header",
            suggested_fix: None,
        },
        children.field_child_4().slot(),
        &ContentSlot {
            missing: "Missing language value in @L1 of header",
            suggested_fix: None,
        },
        errors,
        QualifiedKind::L1Of,
    ) {
        Ok(header) => ParseOutcome::parsed(header),
        Err(failure) => failure.into_outcome(&site, errors),
    };
    surface_displaced(
        &children.children().unexpected,
        L1_OF_HEADER,
        typed.source(),
        errors,
    );
    header
}

/// `@Options` -> `Header::Options`, read by the model's one option-list
/// reader. A list with an empty position (`CA,,NoAlign`, a trailing comma, a
/// recovered option name) is recovery: E316 when the tree holds no recovery
/// node of its own (the whole-tree backstop reports one that does), and an
/// unknown header either way. (A body with an ERROR in or beside it never
/// reaches the reader; `read_body` refuses it. A MISSING option name, which
/// drops no text, does.)
pub(super) fn options<'tree>(
    typed: SourceBound<'tree, '_, OptionsHeaderNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    let site = HeaderSite::bound(typed);
    let Ok(children) = crate::parser::typed_cst::report_admission(
        crate::parser::typed_cst::extract_envelope(typed),
        site.actual(),
        site.input(),
        errors,
    ) else {
        return ParseOutcome::Rejected;
    };
    surface_displaced(
        &children.children().unexpected,
        OPTIONS_HEADER,
        site.input(),
        errors,
    );
    let words = ContentSlot {
        missing: "Missing @Options content",
        suggested_fix: None,
    };
    match read_body(&site, children.field_child_2().slot(), &words, errors) {
        Ok(Body::Empty(separator)) => {
            ParseOutcome::parsed(PlainKind::Structured(StructuredKind::Options).empty(separator))
        }
        Ok(Body::Written { body, text }) => match ChatOptionFlags::parse(text) {
            Ok(options) => ParseOutcome::parsed(Header::Options { options }),
            Err(malformed) => {
                let reason = malformed.to_string();
                if !body.into_inner().raw_node().has_error() {
                    errors.report(talkbank_model::ParseError::at_span(
                        talkbank_model::ErrorCode::UnparsableContent,
                        talkbank_model::Severity::Error,
                        crate::parser::node_span::span_of(site.actual()),
                        reason.clone(),
                    ));
                }
                ParseOutcome::parsed(site.unknown(reason, None))
            }
        },
        Err(failure) => failure.into_outcome(&site, errors),
    }
}
