//! Source-spacing validation: E751 (pause glued to the preceding word)
//! and E757 (bracketed code glued to the following content).
//!
//! Sibling of the comma-spacing rules in `comma.rs` (E258/E259/E749):
//! CHAT items are space-delimited in the source, and these rules detect
//! glued items by SPAN ADJACENCY over the in-order content walk, which
//! works because the parser preserves byte spans on words and pauses.
//! Both parser backends preserve pause spans. Dummy (0,0) spans from
//! programmatically constructed items are skipped.
//!
//! E758 (leading/trailing space between a tab delimiter and tier
//! content) used to live here as a main-tier-only span reconstruction
//! (`first_element_start`); it is now read uniformly from every source
//! line's [`crate::model::TierSeparator`] (main tier, dependent tiers,
//! and headers alike), so that reconstruction was deleted.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Pauses>
// Design rule 3, enforced by the compiler rather than by prose: a `_` arm over
// a content enum means a future variant compiles clean and answers wrong.
// Added per file as each is cleaned; `content_catch_alls` lists the rest.
#![deny(clippy::wildcard_enum_match_arm)]

use crate::alignment::helpers::{ContentItem, walk_content};
use crate::model::{Utterance, UtteranceContent};
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};

// The source end byte of a content item, when the item is a word whose
// trailing edge can glue a following pause. Non-word items return
// `None`: CHECK 57 fires on the word-then-`(` shape specifically.

/// The source start byte of a top-level item whose leading edge can be
/// glued onto a preceding code's `]` (the word family). Other variants
/// return `None`: the grounded CHECK-19 shape is code-then-word; further
/// glue shapes get their own grounding before extension (see the spec).
fn word_family_start(item: &UtteranceContent) -> Option<u32> {
    match item {
        UtteranceContent::Word(word) => Some(word.span.start),
        UtteranceContent::AnnotatedWord(annotated) => Some(annotated.inner.span.start),
        UtteranceContent::ReplacedWord(replaced) => Some(replaced.word.span.start),
        // Deliberately not glue targets in the grounded shape: groups,
        // events, pauses (E751's territory), markers, and separators.
        UtteranceContent::Event(_)
        | UtteranceContent::AnnotatedEvent(_)
        | UtteranceContent::Pause(_)
        | UtteranceContent::Group(_)
        | UtteranceContent::AnnotatedGroup(_)
        | UtteranceContent::Retrace(_)
        | UtteranceContent::AnnotatedRetrace(_)
        | UtteranceContent::PhoGroup(_)
        | UtteranceContent::SinGroup(_)
        | UtteranceContent::Quotation(_)
        | UtteranceContent::AnnotatedQuotation(_)
        | UtteranceContent::Action(_)
        | UtteranceContent::AnnotatedAction(_)
        | UtteranceContent::Freecode(_)
        | UtteranceContent::Separator(_)
        | UtteranceContent::OverlapPoint(_)
        | UtteranceContent::InternalBullet(_)
        | UtteranceContent::LongFeatureBegin(_)
        | UtteranceContent::LongFeatureEnd(_)
        | UtteranceContent::UnderlineBegin(_)
        | UtteranceContent::UnderlineEnd(_)
        | UtteranceContent::NonvocalBegin(_)
        | UtteranceContent::NonvocalEnd(_)
        | UtteranceContent::NonvocalSimple(_)
        | UtteranceContent::OtherSpokenEvent(_) => None,
    }
}

/// The source end byte of an item that ENDS with a bracketed code, so
/// that glued following material is a separate item rather than part of
/// this one. Two shapes qualify:
///
/// - a retrace, whose span covers its content plus the `[...]` marker;
/// - any annotated item CARRYING scoped annotations (`hello [!]`,
///   `bobo [= toy]`), whose wrapper span covers the annotations.
///
/// This used to exclude "an annotated item with no scoped annotations",
/// guarding a case that no longer exists: `AnnotatedContentAnnotations` is
/// non-empty by construction since 2026-08-26, so an annotated item always
/// carries at least one annotation and its wrapper span always covers them.
/// The guard was removed rather than left as a check the type had made
/// unfalsifiable.
fn glued_code_end(item: &UtteranceContent) -> Option<u32> {
    /// The wrapper span, which always covers the annotations.
    fn annotated_end<T>(annotated: &crate::model::Annotated<T>) -> Option<u32> {
        Some(annotated.span.end)
    }
    match item {
        UtteranceContent::Retrace(retrace) => Some(retrace.span.end),
        // Ends with the `]` of its last annotation, like every other
        // annotated variant, so it shares their helper.
        UtteranceContent::AnnotatedRetrace(annotated) => annotated_end(annotated),
        UtteranceContent::AnnotatedWord(annotated) => annotated_end(annotated),
        UtteranceContent::AnnotatedGroup(annotated) => annotated_end(annotated),
        UtteranceContent::AnnotatedEvent(annotated) => annotated_end(annotated),
        UtteranceContent::AnnotatedAction(annotated) => annotated_end(annotated),
        UtteranceContent::AnnotatedQuotation(annotated) => annotated_end(annotated),
        // Not code-terminated: nothing here ends with a `]`. A bare action is
        // `0`, which does not, so it belongs here rather than beside its
        // annotated sibling above.
        UtteranceContent::Action(_)
        | UtteranceContent::Word(_)
        | UtteranceContent::ReplacedWord(_)
        | UtteranceContent::Event(_)
        | UtteranceContent::Pause(_)
        | UtteranceContent::Group(_)
        | UtteranceContent::PhoGroup(_)
        | UtteranceContent::SinGroup(_)
        | UtteranceContent::Quotation(_)
        | UtteranceContent::Freecode(_)
        | UtteranceContent::Separator(_)
        | UtteranceContent::OverlapPoint(_)
        | UtteranceContent::InternalBullet(_)
        | UtteranceContent::LongFeatureBegin(_)
        | UtteranceContent::LongFeatureEnd(_)
        | UtteranceContent::UnderlineBegin(_)
        | UtteranceContent::UnderlineEnd(_)
        | UtteranceContent::NonvocalBegin(_)
        | UtteranceContent::NonvocalEnd(_)
        | UtteranceContent::NonvocalSimple(_)
        | UtteranceContent::OtherSpokenEvent(_) => None,
    }
}

/// E757: a bracketed code's closing `]` must not run directly into the
/// next word (`hello [/]x`, `hello [!]x`; CLAN CHECK 19). Fires when a
/// code-terminated item's span ends exactly where the next top-level
/// word-family item starts.
pub(crate) fn check_code_glued_to_following_content(
    utterance: &Utterance,
    errors: &impl ErrorSink,
) {
    for pair in utterance.main.content.content.as_slice().windows(2) {
        let Some(code_end) = glued_code_end(&pair[0]) else {
            continue;
        };
        if code_end == crate::Span::DUMMY.end {
            continue;
        }
        let Some(next_start) = word_family_start(&pair[1]) else {
            continue;
        };
        if next_start == code_end {
            errors.report(
                ParseError::new(
                    ErrorCode::CodeGluedToFollowingContent,
                    Severity::Error,
                    SourceLocation::new(crate::Span::new(code_end, code_end)),
                    ErrorContext::new("]", crate::Span::new(code_end, code_end), "]"),
                    "Bracketed code must be separated from the following word by a space",
                )
                .with_suggestion("Add a space after the closing bracket"),
            );
        }
    }
}

/// The `&`-prefix categories: each introduces a word of its own, so each
/// needs a space before it. `Omission` (`0word`) and `CAOmission`
/// (`(word)`) are deliberately excluded: they are not `&` forms, they do
/// not split a glued token into two words, and the glued shape is already
/// rejected elsewhere (E220).
///
/// Deliberately NOT `WordCategory::material()`, which selects the same three
/// categories today. That asks whether the LETTERS are a spelling; this asks
/// about the SURFACE, because a `&` opens a new word and so needs a space before
/// it. A sound category written without a `&` prefix would belong to one and not
/// the other, so each site keeps answering its own question.
///
/// This was briefly derived as `to_chat_prefix().starts_with('&')`, which is
/// WORSE than the match it replaced: `to_chat_prefix` is a serialiser whose own
/// doc calls it "intentionally serialization-focused", it returns `""` for
/// `CAOmission` (an empty string doing duty as "has no prefix"), and a
/// validation rule has no business recovering a fact from a rendering of it.
/// The real fix is a typed `CategoryPrefix { None | Zero | Ampersand(..) }` that
/// both this and `to_chat_prefix` derive FROM; until that exists an exhaustive
/// match the compiler checks is the honest form. Recorded in the workspace's
/// deferred-type-findings note.
fn is_ampersand_prefixed(word: &crate::model::Word) -> bool {
    matches!(
        word.category,
        Some(
            crate::model::WordCategory::Filler
                | crate::model::WordCategory::Nonword
                | crate::model::WordCategory::PhonologicalFragment
        )
    )
}

/// E764: a `&`-prefixed form must not run directly into the preceding
/// word (`dog&-um`). Fires when such a word's span starts at the byte
/// where the previous in-order word's span ends. Mirror of
/// [`check_pause_glued_to_word`]: same walk, same adjacency test, same
/// dummy-span opt-out; only the glued item's identity differs.
pub(crate) fn check_prefixed_form_glued_to_preceding_word(
    utterance: &Utterance,
    errors: &impl ErrorSink,
) {
    let mut prev_word_end: Option<u32> = None;

    walk_content(
        utterance.main.content.content.as_slice(),
        None,
        &mut |item| {
            if let ContentItem::Word(word) = &item
                && word.span != crate::Span::DUMMY
                && is_ampersand_prefixed(word)
                && let Some(end) = prev_word_end
                && word.span.start == end
            {
                errors.report(
                    ParseError::new(
                        ErrorCode::PrefixedFormGluedToPrecedingWord,
                        Severity::Error,
                        SourceLocation::new(word.span),
                        ErrorContext::new("&", word.span, "&"),
                        "Prefixed form must be separated from the preceding word by a space",
                    )
                    .with_suggestion("Add a space between the word and the prefixed form"),
                );
            }
            prev_word_end = item.word_span().map(|span| span.end);
        },
    );
}

/// Whether nothing may be glued after this separator.
///
/// SCOPE, set by the corpus (2026-07-29 differential, adjudicated
/// UNINTENDED for the wider reading): only the plain punctuation
/// separators, `:` and `;`. Every CA mark is deliberately EXCLUDED,
/// because glue is part of what several of them MEAN and the wild data
/// uses it systematically:
///
/// - `≡` is latching, "no gap between turns", and is written glued on
///   both sides (`y≡I≡`); spacing it would misstate the phenomenon;
/// - the intonation arrows attach to the material they mark, including
///   directly before an overlap close (`⌊I don't know⇗⌋`);
/// - the remaining marks (`„ ‡ ∞ ≈ ≋`) are unadjudicated against real
///   data, so they stay out until they are.
///
/// A wider rule flagged 270 instances in a 2% corpus sample (~13,500
/// corpus-wide) of exactly these legitimate shapes. The comma is
/// excluded too: `,dog` is E749 and `,,` is E258.
fn separator_forbids_trailing_glue(separator: &crate::model::Separator) -> bool {
    matches!(
        separator,
        crate::model::Separator::Colon { .. } | crate::model::Separator::Semicolon { .. }
    )
}

/// Source-located roles for the separator rule. One classification owns both
/// sides of adjacency; excluded items break the chain rather than disappearing.
enum SpacingBoundary {
    FreeStanding(crate::Span),
    Word(crate::Span),
    Excluded,
}

impl SpacingBoundary {
    fn from_item(item: &ContentItem<'_>) -> Self {
        let boundary = match item {
            ContentItem::Word(word) => Self::Word(word.span),
            ContentItem::ReplacedWord(replaced) => Self::Word(replaced.word.span),
            ContentItem::Pause(pause) => Self::FreeStanding(pause.span),
            ContentItem::Separator(separator) => {
                if separator_forbids_trailing_glue(separator) {
                    Self::FreeStanding(separator.span())
                } else {
                    Self::Excluded
                }
            }
            ContentItem::Event(_)
            | ContentItem::Action(_)
            | ContentItem::OverlapPoint(_)
            | ContentItem::OtherSpokenEvent(_)
            | ContentItem::Freecode(_)
            | ContentItem::InternalBullet(_)
            | ContentItem::LongFeatureBegin(_)
            | ContentItem::LongFeatureEnd(_)
            | ContentItem::UnderlineBegin(_)
            | ContentItem::UnderlineEnd(_)
            | ContentItem::NonvocalBegin(_)
            | ContentItem::NonvocalEnd(_)
            | ContentItem::NonvocalSimple(_) => Self::Excluded,
        };
        match boundary {
            Self::FreeStanding(span) | Self::Word(span) => {
                if span.is_dummy() {
                    Self::Excluded
                } else {
                    boundary
                }
            }
            Self::Excluded => Self::Excluded,
        }
    }

    fn start(&self) -> Option<u32> {
        match self {
            Self::FreeStanding(span) | Self::Word(span) => Some(span.start),
            Self::Excluded => None,
        }
    }

    fn free_standing_end(&self) -> Option<u32> {
        match self {
            Self::FreeStanding(span) => Some(span.end),
            Self::Word(_) | Self::Excluded => None,
        }
    }
}

/// E765: a free-standing `:` or `;` separator, or a pause, must not run
/// directly into the item after it (`:and`, `;;`, `(.)dog`). Fires when
/// the following item starts at the byte where the free-standing item
/// ends.
///
/// Only this direction: trailing glue ONTO a word (`word↘`, `dog,`) is
/// documented CHAT convention and stays valid. Juxtaposition-matrix cell
/// 7, narrowed from its ruled scope by real-corpus evidence; see the spec
/// and [`separator_forbids_trailing_glue`] for what is excluded and why.
pub(crate) fn check_separator_glued_to_following_content(
    utterance: &Utterance,
    errors: &impl ErrorSink,
) {
    let mut previous_end = None;
    walk_content(
        utterance.main.content.content.as_slice(),
        None,
        &mut |item| {
            let boundary = SpacingBoundary::from_item(&item);
            if let (Some(end), Some(start)) = (previous_end, boundary.start())
                && start == end
            {
                let span = crate::Span::new(start, start);
                errors.report(
                    ParseError::new(
                        ErrorCode::SeparatorGluedToFollowingContent,
                        Severity::Error,
                        SourceLocation::new(span),
                        ErrorContext::new(" ", span, " "),
                        "Separator must be separated from the following content by a space",
                    )
                    .with_suggestion("Add a space after the separator"),
                );
            }
            previous_end = boundary.free_standing_end();
        },
    );
}

/// E751: a pause must not open directly at the end of a word
/// (`hello(.)`; CLAN CHECK 57). Fires when a pause's span starts at the
/// byte where the previous in-order word's span ends.
pub(crate) fn check_pause_glued_to_word(utterance: &Utterance, errors: &impl ErrorSink) {
    let mut prev_word_end: Option<u32> = None;

    walk_content(
        utterance.main.content.content.as_slice(),
        None,
        &mut |item| {
            if let ContentItem::Pause(pause) = item
                && pause.span != crate::Span::DUMMY
                && let Some(end) = prev_word_end
                && pause.span.start == end
            {
                errors.report(
                    ParseError::new(
                        ErrorCode::PauseGluedToWord,
                        Severity::Error,
                        SourceLocation::new(pause.span),
                        ErrorContext::new("(", pause.span, "("),
                        "Pause must be separated from the preceding word by a space",
                    )
                    .with_suggestion("Add a space between the word and the pause"),
                );
            }
            prev_word_end = item.word_span().map(|span| span.end);
        },
    );
}
