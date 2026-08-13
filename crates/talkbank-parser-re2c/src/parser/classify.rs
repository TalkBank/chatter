//! Token classification functions, determine token categories for parser dispatch.
//!
//! These functions translate grammar.js rule membership (e.g., which tokens are
//! terminators, linkers, annotations) into Rust discriminant checks. They are
//! used by the parser to decide which production to enter.

use crate::ast::*;
use crate::token::{Token, TokenDiscriminants};

/// Convert a `WordWithAnnotations` to the appropriate `ContentItem`.
///
/// The marker run is a LEFT-ASSOCIATIVE CHAIN: each marker scopes over
/// everything to its left. So `dog [* p:w] [/]` is a retrace of an
/// error-marked word, `dog [/] [* p:w]` is an error-marked retrace, and
/// `a [//] [/]` is a retrace of a retrace. Folding one wrapper per marker is
/// the meaning of the surface rather than an encoding of it.
///
/// This used to find the FIRST marker and split around it, which silently
/// dropped every marker after it: `a [//] [/] a` lowered as if the `[/]` were
/// not written. The tree-sitter side folds
/// (`talkbank-parser`'s `content/marker_chain.rs`), so the two backends
/// disagreed, and the parity oracle could not see it because it runs over the
/// reference corpus, which is valid CHAT by construction, and this shape is
/// invalid. Covered now by `equivalence_marker_chain`.
pub fn word_to_content_item<'a>(word: WordWithAnnotations<'a>) -> ContentItem<'a> {
    if !word.annotations.iter().any(|a| a.is_retrace()) {
        return ContentItem::Word(word);
    }
    let mut word = word;
    // grammar.js gives `word_with_optional_annotations` a replacement field
    // separate from `base_annotations`, so a replacement is part of the word
    // wherever it was written and never travels up the chain.
    //
    // `extract_if` rather than `partition`: it pulls the replacements OUT and
    // leaves the marker run in the buffer it already owns, so the common
    // `dog [/]` shape allocates nothing extra. `partition` allocated a fresh
    // Vec for the markers on every word-attached retrace, which is ~1.9M of
    // them corpus-wide, about 0.8% of the parse phase's total allocations.
    let mut markers = std::mem::take(&mut word.annotations);
    word.annotations = markers
        .extract_if(.., |a| matches!(a, ParsedAnnotation::Replacement(_)))
        .collect();

    Chain::Word(word).fold(markers).into_content_item()
}

/// What a marker fold has built so far.
///
/// One accumulator for all three seeds a marker run can start from (a word, an
/// event, a bracketed group), rather than folding over `ContentItem`, whose
/// other variants a marker run can never reach. That keeps `annotated_with`
/// total with no catch-all.
///
/// Shared by all three fold sites in this crate. They were three separate
/// algorithms: the word path folded, the event path partitioned markers out
/// and so lost the annotation/marker interleaving, and the group path still
/// split at the FIRST marker and dropped the rest, which is the exact bug the
/// word path had been fixed for. Covered by `equivalence_marker_chain`.
pub(crate) enum Chain<'a> {
    /// A word, with whatever annotations have attached so far.
    Word(WordWithAnnotations<'a>),
    /// An event token, with its annotations.
    Event(Token<'a>, Vec<ParsedAnnotation<'a>>),
    /// A bracketed group, with its annotations.
    Group(Vec<ContentItem<'a>>, Vec<ParsedAnnotation<'a>>),
    /// At least one retrace marker has wrapped the chain.
    Wrapped(Retrace<'a>),
}

impl<'a> Chain<'a> {
    /// Fold an ordered marker run onto this seed, one wrapper per marker.
    pub(crate) fn fold(self, markers: impl IntoIterator<Item = ParsedAnnotation<'a>>) -> Self {
        markers
            .into_iter()
            .fold(self, |chain, marker| match marker.retrace_kind() {
                Some(kind) => chain.retraced(kind),
                None => chain.annotated_with(marker),
            })
    }

    /// Attach a non-retrace annotation to whatever the chain currently is.
    fn annotated_with(self, annotation: ParsedAnnotation<'a>) -> Self {
        match self {
            Chain::Word(mut word) => {
                word.annotations.push(annotation);
                Chain::Word(word)
            }
            Chain::Event(event, mut annotations) => {
                annotations.push(annotation);
                Chain::Event(event, annotations)
            }
            Chain::Group(contents, mut annotations) => {
                annotations.push(annotation);
                Chain::Group(contents, annotations)
            }
            Chain::Wrapped(mut retrace) => {
                retrace.annotations.push(annotation);
                Chain::Wrapped(retrace)
            }
        }
    }

    /// Wrap the chain in a retrace.
    fn retraced(self, kind: RetraceKindParsed) -> Self {
        // A group with no annotations of its own hands its brackets to the
        // retrace, which is what `is_group` records. Nesting it instead would
        // serialize `<<a b>> [/]`, a bracket pair nobody wrote.
        //
        // SHARED SEMANTICS, written twice. `talkbank-parser`'s
        // `content::marker_chain::retrace` decides the same thing on the model
        // type, so no constructor can own it for both. Drift here is silent and
        // corrupts output, so the enforcement is the cross-parser test
        // `equivalence_marker_chain`; change one of these two and run it.
        if let Chain::Group(contents, annotations) = self {
            if annotations.is_empty() {
                return Chain::Wrapped(Retrace {
                    content: contents,
                    kind,
                    is_group: true,
                    synthesized_missing_annotation: false,
                    annotations: Vec::new(),
                });
            }
            return Chain::Wrapped(Retrace {
                content: vec![ContentItem::Group(Group {
                    contents,
                    annotations,
                })],
                kind,
                is_group: false,
                synthesized_missing_annotation: false,
                annotations: Vec::new(),
            });
        }
        Chain::Wrapped(Retrace {
            content: vec![self.into_content_item()],
            kind,
            is_group: false,
            synthesized_missing_annotation: false,
            annotations: Vec::new(),
        })
    }

    pub(crate) fn into_content_item(self) -> ContentItem<'a> {
        match self {
            Chain::Word(word) => ContentItem::Word(word),
            Chain::Event(event, annotations) => {
                if annotations.is_empty() {
                    ContentItem::Event(event.text())
                } else {
                    ContentItem::AnnotatedEvent {
                        event: event.text(),
                        annotations,
                    }
                }
            }
            Chain::Group(contents, annotations) => ContentItem::Group(Group {
                contents,
                annotations,
            }),
            Chain::Wrapped(retrace) => ContentItem::Retrace(retrace),
        }
    }
}

/// Convert an annotation token to a typed `ParsedAnnotation`, or `None`.
///
/// THE list of which token is an annotation. It used to end in a `panic!`,
/// defended by a comment asserting that `is_annotation`'s separate
/// seventeen-discriminant list exactly covered these eighteen arms. Nothing
/// bound the two, so an annotation token added to the lexer and missed in one
/// of them either panicked the parser or admitted a token the converters
/// mishandle. `Option` says the same thing without a panic in a parser, and
/// `is_annotation` is derived from it rather than repeating it.
pub fn token_to_parsed_annotation<'a>(tok: Token<'a>) -> Option<ParsedAnnotation<'a>> {
    Some(match tok {
        Token::RetracePartial(_) => ParsedAnnotation::Retrace(RetraceKindParsed::Partial),
        Token::RetraceComplete(_) => ParsedAnnotation::Retrace(RetraceKindParsed::Complete),
        Token::RetraceMultiple(_) => ParsedAnnotation::Retrace(RetraceKindParsed::Multiple),
        Token::RetraceReformulation(_) => {
            ParsedAnnotation::Retrace(RetraceKindParsed::Reformulation)
        }
        Token::ScopedStressing(_) => ParsedAnnotation::Stressing,
        Token::ScopedContrastiveStressing(_) => ParsedAnnotation::ContrastiveStressing,
        Token::ScopedUncertain(_) => ParsedAnnotation::Uncertain,
        Token::ExcludeMarker(_) => ParsedAnnotation::Exclude,
        Token::ErrorMarkerAnnotation(s) => ParsedAnnotation::Error(s),
        Token::OverlapPrecedes(s) => ParsedAnnotation::OverlapPrecedes(s),
        Token::OverlapFollows(s) => ParsedAnnotation::OverlapFollows(s),
        Token::ExplanationAnnotation(s) => ParsedAnnotation::Explanation(s),
        Token::ParaAnnotation(s) => ParsedAnnotation::Paralinguistic(s),
        Token::AltAnnotation(s) => ParsedAnnotation::Alternative(s),
        Token::PercentAnnotation(s) => ParsedAnnotation::PercentComment(s),
        Token::Replacement(s) => ParsedAnnotation::Replacement(s),
        Token::Langcode(s) => ParsedAnnotation::Langcode(s),
        Token::Postcode(s) => ParsedAnnotation::Postcode(s),
        _ => return None,
    })
}

// ── Token classification (from grammar.js rule definitions) ─────

pub fn is_terminator(d: Option<TokenDiscriminants>) -> bool {
    d.and_then(crate::ast::TerminatorKindParsed::from_discriminant)
        .is_some()
}

pub fn is_linker(d: Option<TokenDiscriminants>) -> bool {
    matches!(
        d,
        Some(
            TokenDiscriminants::LinkerLazyOverlap
                | TokenDiscriminants::LinkerQuickUptake
                | TokenDiscriminants::LinkerQuickUptakeOverlap
                | TokenDiscriminants::LinkerQuotationFollows
                | TokenDiscriminants::LinkerSelfCompletion
                | TokenDiscriminants::CaNoBreakLinker
                | TokenDiscriminants::CaTechnicalBreakLinker
        )
    )
}

pub fn is_pause(d: TokenDiscriminants) -> bool {
    matches!(
        d,
        TokenDiscriminants::PauseLong
            | TokenDiscriminants::PauseMedium
            | TokenDiscriminants::PauseShort
            | TokenDiscriminants::PauseTimed
    )
}

pub fn is_word_start(d: TokenDiscriminants) -> bool {
    matches!(
        d,
        TokenDiscriminants::WordSegment
            | TokenDiscriminants::Zero
            | TokenDiscriminants::PrefixFiller
            | TokenDiscriminants::PrefixNonword
            | TokenDiscriminants::PrefixFragment
            | TokenDiscriminants::Shortening
            | TokenDiscriminants::StressPrimary
            | TokenDiscriminants::StressSecondary
            | TokenDiscriminants::Ampersand
            // CA markers can start a word (standalone or preceding text)
            | TokenDiscriminants::CaBlockedSegments | TokenDiscriminants::CaConstriction
            | TokenDiscriminants::CaHardening | TokenDiscriminants::CaHurriedStart
            | TokenDiscriminants::CaInhalation | TokenDiscriminants::CaLaughInWord
            | TokenDiscriminants::CaPitchDown | TokenDiscriminants::CaPitchReset
            | TokenDiscriminants::CaPitchUp | TokenDiscriminants::CaSuddenStop
            | TokenDiscriminants::CaUnsure | TokenDiscriminants::CaPrecise
            | TokenDiscriminants::CaCreaky | TokenDiscriminants::CaSofter
            | TokenDiscriminants::CaSegmentRepetition | TokenDiscriminants::CaFaster
            | TokenDiscriminants::CaSlower | TokenDiscriminants::CaWhisper
            | TokenDiscriminants::CaSinging | TokenDiscriminants::CaLowPitch
            | TokenDiscriminants::CaHighPitch | TokenDiscriminants::CaLouder
            | TokenDiscriminants::CaSmileVoice | TokenDiscriminants::CaBreathyVoice
            | TokenDiscriminants::CaYawn
    )
}

pub fn is_word_token(d: TokenDiscriminants) -> bool {
    matches!(
        d,
        TokenDiscriminants::WordSegment
            | TokenDiscriminants::Zero
            | TokenDiscriminants::PrefixFiller
            | TokenDiscriminants::PrefixNonword
            | TokenDiscriminants::PrefixFragment
            | TokenDiscriminants::Shortening
            | TokenDiscriminants::Lengthening
            | TokenDiscriminants::StressPrimary | TokenDiscriminants::StressSecondary
            | TokenDiscriminants::CompoundMarker
            | TokenDiscriminants::OverlapTopBegin | TokenDiscriminants::OverlapTopEnd | TokenDiscriminants::OverlapBottomBegin | TokenDiscriminants::OverlapBottomEnd
            | TokenDiscriminants::SyllablePause
            | TokenDiscriminants::Tilde
            // Note: UnderlineBegin/End are NOT word tokens; they're content-level markers
            | TokenDiscriminants::CaBlockedSegments | TokenDiscriminants::CaConstriction
            | TokenDiscriminants::CaHardening | TokenDiscriminants::CaHurriedStart
            | TokenDiscriminants::CaInhalation | TokenDiscriminants::CaLaughInWord
            | TokenDiscriminants::CaPitchDown | TokenDiscriminants::CaPitchReset
            | TokenDiscriminants::CaPitchUp | TokenDiscriminants::CaSuddenStop
            | TokenDiscriminants::CaUnsure | TokenDiscriminants::CaPrecise
            | TokenDiscriminants::CaCreaky | TokenDiscriminants::CaSofter
            | TokenDiscriminants::CaSegmentRepetition | TokenDiscriminants::CaFaster
            | TokenDiscriminants::CaSlower | TokenDiscriminants::CaWhisper
            | TokenDiscriminants::CaSinging | TokenDiscriminants::CaLowPitch
            | TokenDiscriminants::CaHighPitch | TokenDiscriminants::CaLouder
            | TokenDiscriminants::CaSmileVoice | TokenDiscriminants::CaBreathyVoice
            | TokenDiscriminants::CaYawn
            | TokenDiscriminants::FormMarker
            | TokenDiscriminants::WordLangSuffix
            | TokenDiscriminants::PosTag
            | TokenDiscriminants::Ampersand
    )
}
