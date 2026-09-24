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
use crate::model::{BracketedContent, BracketedItem, Utterance, UtteranceContent, Word};
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};

/// A non-sentinel closing-code position admitted from a model span.
/// This preserves the existing refusal to diagnose unlocated constructed nodes.
#[derive(Clone, Copy)]
struct CodeEnd(std::num::NonZeroU32);

impl CodeEnd {
    fn from_span(span: crate::Span) -> Option<Self> {
        std::num::NonZeroU32::new(span.end).map(Self)
    }

    fn check_following(self, word: &Word, errors: &impl ErrorSink) {
        let end = self.0.get();
        if word.span.start == end {
            let location = crate::Span::new(end, end);
            errors.report(
                ParseError::new(
                    ErrorCode::CodeGluedToFollowingContent,
                    Severity::Error,
                    SourceLocation::new(location),
                    ErrorContext::new("]", location, "]"),
                    "Bracketed code must be separated from the following word by a space",
                )
                .with_suggestion("Add a space after the closing bracket"),
            );
        }
    }
}

/// One item's spacing evidence and enclosed sequence, projected together from
/// that item. Private producers prevent pairing another item's boundary with
/// its children. A word may also end in an annotation; these are separate axes.
struct CodeSpacing<'a> {
    word: Option<&'a Word>,
    end: Option<CodeEnd>,
    enclosed: Option<&'a BracketedContent>,
}

trait CodeSpacingSource {
    fn code_spacing(&self) -> CodeSpacing<'_>;
}

// The two content enums share the same spacing policy. One exhaustive mapping
// generates both projections; adding a variant to either requires a decision.
macro_rules! impl_code_spacing {
    ($content:ty) => {
        impl CodeSpacingSource for $content {
            fn code_spacing(&self) -> CodeSpacing<'_> {
                let (word, end) = match self {
                    Self::Word(word) => (Some(word.as_ref()), None),
                    Self::AnnotatedWord(annotated) => {
                        (Some(&annotated.inner), CodeEnd::from_span(annotated.span))
                    }
                    // The source wrapper includes the replacement and any
                    // trailing scoped annotations, not just the spoken word.
                    Self::ReplacedWord(replaced) => {
                        (Some(&replaced.word), CodeEnd::from_span(replaced.span))
                    }
                    Self::Retrace(retrace) => (None, CodeEnd::from_span(retrace.span)),
                    Self::AnnotatedRetrace(annotated) => (None, CodeEnd::from_span(annotated.span)),
                    Self::AnnotatedGroup(annotated) => (None, CodeEnd::from_span(annotated.span)),
                    Self::AnnotatedEvent(annotated) => (None, CodeEnd::from_span(annotated.span)),
                    Self::AnnotatedAction(annotated) => (None, CodeEnd::from_span(annotated.span)),
                    Self::AnnotatedQuotation(annotated) => {
                        (None, CodeEnd::from_span(annotated.span))
                    }
                    Self::Action(_)
                    | Self::Event(_)
                    | Self::Pause(_)
                    | Self::Group(_)
                    | Self::PhoGroup(_)
                    | Self::SinGroup(_)
                    | Self::Quotation(_)
                    | Self::Freecode(_)
                    | Self::Separator(_)
                    | Self::OverlapPoint(_)
                    | Self::InternalBullet(_)
                    | Self::LongFeatureBegin(_)
                    | Self::LongFeatureEnd(_)
                    | Self::UnderlineBegin(_)
                    | Self::UnderlineEnd(_)
                    | Self::NonvocalBegin(_)
                    | Self::NonvocalEnd(_)
                    | Self::NonvocalSimple(_)
                    | Self::OtherSpokenEvent(_) => (None, None),
                };
                CodeSpacing {
                    word,
                    end,
                    enclosed: self.structure().enclosed(),
                }
            }
        }
    };
}

impl_code_spacing!(UtteranceContent);
impl_code_spacing!(BracketedItem);

/// Check neighbors within one owning sequence. Recursive calls start a fresh
/// predecessor state: an outer wrapper is never adjacent to its own first word.
fn check_code_sequence(items: &[impl CodeSpacingSource], errors: &impl ErrorSink) {
    let mut previous: Option<CodeEnd> = None;
    for item in items {
        let spacing = item.code_spacing();
        if let (Some(end), Some(word)) = (previous, spacing.word) {
            end.check_following(word, errors);
        }
        if let Some(enclosed) = spacing.enclosed {
            check_code_sequence(enclosed.content.as_slice(), errors);
        }
        previous = spacing.end;
    }
}

/// E757: a bracketed code's closing `]` must not run directly into the
/// next word (`hello [/]x`, `hello [!]x`; CLAN CHECK 19). Apply the same
/// sibling-adjacency rule at every depth without flattening container edges.
pub(crate) fn check_code_glued_to_following_content(
    utterance: &Utterance,
    errors: &impl ErrorSink,
) {
    check_code_sequence(utterance.main.content.content.as_slice(), errors);
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
