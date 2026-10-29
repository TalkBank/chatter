//! Token classification functions, determine token categories for parser dispatch.
//!
//! These functions translate grammar.js rule membership (e.g., which tokens are
//! terminators, linkers, annotations) into Rust discriminant checks. They are
//! used by the parser to decide which production to enter.

use crate::ast::*;
use crate::token::{Token, TokenDiscriminants};

/// Convert an annotation token to a typed `ParsedAnnotation`, or `None`.
///
/// THE list of which token is an annotation. It used to end in a `panic!`,
/// defended by a comment asserting that `is_annotation`'s separate
/// seventeen-discriminant list exactly covered these eighteen arms. Nothing
/// bound the two, so an annotation token added to the lexer and missed in one
/// of them either panicked the parser or admitted a token the converters
/// mishandle. `Option` says the same thing without a panic in a parser, and
/// consumers use its result rather than repeating it.
pub(crate) fn token_to_parsed_annotation<'a>(
    tok: Token<'a>,
    source: crate::source_text::SourceText<'a>,
) -> Option<ParsedAnnotation<'a>> {
    use talkbank_model::model::{
        CodeSwitchSpan, ContentAnnotation as A, ScopedAlternative, ScopedError, ScopedExplanation,
        ScopedOverlapBegin, ScopedOverlapEnd, ScopedParalinguistic, ScopedPercentComment,
    };
    let scoped = |annotation, lexeme| {
        Some(ParsedAnnotation::Scoped(ScopedAnnotationParsed::new(
            annotation,
            source.bind_slice(lexeme)?,
        )))
    };
    let retrace = |kind, lexeme| {
        Some(ParsedAnnotation::Retrace(
            kind,
            crate::ast::RetraceMarker::new(source.bind_slice(lexeme)?),
        ))
    };
    Some(match tok {
        Token::RetracePartial(s) => retrace(RetraceKindParsed::Partial, s)?,
        Token::RetraceComplete(s) => retrace(RetraceKindParsed::Complete, s)?,
        Token::RetraceMultiple(s) => retrace(RetraceKindParsed::Multiple, s)?,
        Token::RetraceReformulation(s) => retrace(RetraceKindParsed::Reformulation, s)?,
        Token::ScopedStressing(s) => scoped(A::Stressing, s)?,
        Token::ScopedContrastiveStressing(s) => scoped(A::ContrastiveStressing, s)?,
        Token::ScopedUncertain(s) => scoped(A::Uncertain, s)?,
        Token::ExcludeMarker(s) => scoped(A::Exclude, s)?,
        Token::CodeSwitchShortcut(s) => scoped(A::CodeSwitch(CodeSwitchSpan::Shortcut), s)?,
        Token::CodeSwitchExplicit(s) => {
            let Ok(code) = talkbank_model::LanguageCode::new(s.text()) else {
                return None;
            };
            scoped(A::CodeSwitch(CodeSwitchSpan::Explicit(code)), s.lexeme())?
        }
        Token::ErrorMarkerAnnotation(s) => scoped(
            A::Error(ScopedError {
                code: if s.text().is_empty() {
                    None
                } else {
                    Some(s.text().into())
                },
            }),
            s.lexeme(),
        )?,
        Token::OverlapPrecedes(s) => {
            let index = ScopedOverlapIndex::from_lexer(s)?;
            scoped(
                A::OverlapBegin(ScopedOverlapBegin {
                    index: index.index(),
                }),
                index.lexeme(),
            )?
        }
        Token::OverlapFollows(s) => {
            let index = ScopedOverlapIndex::from_lexer(s)?;
            scoped(
                A::OverlapEnd(ScopedOverlapEnd {
                    index: index.index(),
                }),
                index.lexeme(),
            )?
        }
        Token::ExplanationAnnotation(s) => scoped(
            A::Explanation(ScopedExplanation {
                text: s.text().into(),
            }),
            s.lexeme(),
        )?,
        Token::ParaAnnotation(s) => scoped(
            A::Paralinguistic(ScopedParalinguistic {
                text: s.text().into(),
            }),
            s.lexeme(),
        )?,
        Token::AltAnnotation(s) => scoped(
            A::Alternative(ScopedAlternative {
                text: s.text().into(),
            }),
            s.lexeme(),
        )?,
        Token::PercentAnnotation(s) => scoped(
            A::PercentComment(ScopedPercentComment {
                text: s.text().into(),
            }),
            s.lexeme(),
        )?,
        Token::Langcode(s) => ParsedAnnotation::Langcode(s),
        Token::Postcode(s) => ParsedAnnotation::Postcode(s),
        _ => return None,
    })
}

// ── Token classification (from grammar.js rule definitions) ─────

pub(crate) fn is_terminator(discriminant: TokenDiscriminants) -> bool {
    terminator_constructor(discriminant).is_some()
}

/// One lexical dispatch table produces shared terminators, requiring an actual
/// span. Classification inspects the constructor without minting a placeholder
/// model value or maintaining a parallel semantic enum.
pub(crate) fn terminator_constructor(
    discriminant: TokenDiscriminants,
) -> Option<fn(talkbank_model::Span) -> talkbank_model::Terminator> {
    use TokenDiscriminants as D;
    use talkbank_model::Terminator;
    Some(match discriminant {
        D::Period => |span| Terminator::Period {
            location: span.into(),
        },
        D::Question => |span| Terminator::Question {
            location: span.into(),
        },
        D::Exclamation => |span| Terminator::Exclamation {
            location: span.into(),
        },
        D::TrailingOff => |span| Terminator::TrailingOff {
            location: span.into(),
        },
        D::Interruption => |span| Terminator::Interruption {
            location: span.into(),
        },
        D::SelfInterruption => |span| Terminator::SelfInterruption {
            location: span.into(),
        },
        D::InterruptedQuestion => |span| Terminator::InterruptedQuestion {
            location: span.into(),
        },
        D::BrokenQuestion => |span| Terminator::BrokenQuestion {
            location: span.into(),
        },
        D::QuotedNewLine => |span| Terminator::QuotedNewLine {
            location: span.into(),
        },
        D::QuotedPeriodSimple => |span| Terminator::QuotedPeriodSimple {
            location: span.into(),
        },
        D::SelfInterruptedQuestion => |span| Terminator::SelfInterruptedQuestion {
            location: span.into(),
        },
        D::TrailingOffQuestion => |span| Terminator::TrailingOffQuestion {
            location: span.into(),
        },
        D::BreakForCoding => |span| Terminator::BreakForCoding {
            location: span.into(),
        },
        _ => return None,
    })
}

/// Resolve linker membership and meaning together at grammar admission.
pub(crate) fn linker_kind(discriminant: TokenDiscriminants) -> Option<talkbank_model::LinkerKind> {
    use talkbank_model::LinkerKind;
    Some(match discriminant {
        TokenDiscriminants::LinkerLazyOverlap => LinkerKind::LazyOverlapPrecedes,
        TokenDiscriminants::LinkerQuickUptake => LinkerKind::OtherCompletion,
        TokenDiscriminants::LinkerQuickUptakeOverlap => LinkerKind::QuickUptakeOverlap,
        TokenDiscriminants::LinkerQuotationFollows => LinkerKind::QuotationFollows,
        TokenDiscriminants::LinkerSelfCompletion => LinkerKind::SelfCompletion,
        TokenDiscriminants::CaNoBreakLinker => LinkerKind::NoBreakTcuContinuation,
        TokenDiscriminants::CaTechnicalBreakLinker => LinkerKind::TcuContinuation,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_text::SourceText;
    use talkbank_model::Span;

    #[test]
    fn retrace_admission_retains_coordinates_and_rejects_foreign_bytes() {
        fn token(kind: RetraceKindParsed, text: &str) -> Token<'_> {
            match kind {
                RetraceKindParsed::Partial => Token::RetracePartial(text),
                RetraceKindParsed::Complete => Token::RetraceComplete(text),
                RetraceKindParsed::Multiple => Token::RetraceMultiple(text),
                RetraceKindParsed::Reformulation => Token::RetraceReformulation(text),
            }
        }
        let cases = [
            ("[/]", RetraceKindParsed::Partial),
            ("[//]", RetraceKindParsed::Complete),
            ("[///]", RetraceKindParsed::Multiple),
            ("[/-]", RetraceKindParsed::Reformulation),
        ];
        for (text, kind) in cases {
            let input = format!("prefix {text}");
            let source = SourceText::new(&input);
            let lexeme = &input[7..];
            let parsed = token_to_parsed_annotation(token(kind, lexeme), source)
                .expect("source-owned retrace");
            let ParsedAnnotation::Retrace(_, marker) = &parsed else {
                panic!("expected retrace");
            };
            assert_eq!(marker.span(), Span::new(7, input.len()));
            assert_eq!(marker.lexeme(), text);
            assert_eq!(parsed.source_text(source), Some(lexeme));
            assert_eq!(parsed.chat_text(), text);
            let foreign = String::from(text);
            assert!(token_to_parsed_annotation(token(kind, &foreign), source).is_none());
            assert_eq!(parsed.span_in(SourceText::new(&foreign)), None);
            assert_eq!(parsed.source_text(SourceText::new(&foreign)), None);
            let at_zero =
                token_to_parsed_annotation(token(kind, &foreign), SourceText::new(&foreign))
                    .expect("independently admitted retrace");
            assert_eq!(parsed, at_zero, "equality remains independent of location");
            let encoded = serde_json::to_value(&parsed).expect("annotation serialization");
            assert_eq!(encoded["Retrace"][1], text);
        }
    }

    #[test]
    fn scoped_admission_retains_coordinates_and_rejects_foreign_bytes() {
        let input = String::from("prefix [!]");
        let source = SourceText::new(&input);
        let lexeme = &input[7..];
        let parsed = token_to_parsed_annotation(Token::ScopedStressing(lexeme), source)
            .expect("source-owned annotation");
        let scoped = parsed.scoped().expect("scoped annotation");
        assert_eq!(scoped.span(), Span::new(7, 10));
        assert_eq!(scoped.lexeme(), "[!]");
        assert_eq!(parsed.span_in(source), Some(scoped.span()));
        assert_eq!(parsed.source_text(source), Some(lexeme));

        let foreign = String::from(lexeme);
        assert!(token_to_parsed_annotation(Token::ScopedStressing(&foreign), source).is_none());
        assert_eq!(parsed.span_in(SourceText::new(&foreign)), None);
        assert_eq!(parsed.source_text(SourceText::new(&foreign)), None);

        let at_zero =
            token_to_parsed_annotation(Token::ScopedStressing(&foreign), SourceText::new(&foreign))
                .expect("independently admitted annotation");
        assert_eq!(parsed, at_zero, "equality remains independent of location");
        let encoded = serde_json::to_value(scoped).expect("annotation serialization");
        assert_eq!(encoded["lexeme"], "[!]");
        assert_eq!(encoded.as_object().expect("annotation object").len(), 2);
        assert_eq!(
            encoded["annotation"],
            serde_json::to_value(talkbank_model::ContentAnnotation::Stressing).unwrap()
        );
    }
}
