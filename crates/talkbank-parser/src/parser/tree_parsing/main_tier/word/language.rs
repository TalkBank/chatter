//! Lexical admission for the deliberately opaque `word_lang_suffix` token.
//!
//! The grammar keeps this immediate token whole so `:` and `&` cannot be
//! consumed as unrelated word structure. Decode only the source-bound kind,
//! then convert every component atomically; registry validity is a later policy.

use crate::error::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};
use crate::generated_traversal::{AsRawNode, NamedKind, SourceBound, WordLangSuffixNode};
use crate::parser::node_span::span_of;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{LanguageCode, WordLanguageMarker};

/// The spelling decision and its producing token travel together. No caller
/// supplies an independently chosen source or diagnostic range.
struct LanguageSuffix<'tree, 'source> {
    token: SourceBound<'tree, 'source, WordLangSuffixNode<'tree>>,
    spelling: SuffixSpelling<'source>,
}

enum SuffixSpelling<'source> {
    Shortcut,
    Explicit(&'source str),
    Multiple(std::str::Split<'source, char>),
    Ambiguous(std::str::Split<'source, char>),
}

impl<'tree, 'source> LanguageSuffix<'tree, 'source> {
    fn admit(
        token: SourceBound<'tree, 'source, WordLangSuffixNode<'tree>>,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Self> {
        let spelling = match token.text() {
            "@s" => SuffixSpelling::Shortcut,
            text => {
                let Some(payload) = text.strip_prefix("@s:") else {
                    report_fault(token, "Expected @s or @s:codes", errors);
                    return ParseOutcome::Rejected;
                };
                // Preserve the existing &-first interpretation, including
                // any mixed-separator payload that registry validation rejects.
                // This adapter does not decide a new grammar/policy for mixing.
                match (payload.contains('&'), payload.contains('+')) {
                    (true, _) => SuffixSpelling::Ambiguous(payload.split('&')),
                    (false, true) => SuffixSpelling::Multiple(payload.split('+')),
                    (false, false) => SuffixSpelling::Explicit(payload),
                }
            }
        };
        ParseOutcome::Parsed(Self { token, spelling })
    }

    fn into_marker(self, errors: &impl ErrorSink) -> ParseOutcome<WordLanguageMarker> {
        let marker = match self.spelling {
            SuffixSpelling::Shortcut => return ParseOutcome::Parsed(WordLanguageMarker::Shortcut),
            SuffixSpelling::Explicit(code) => {
                LanguageCode::new(code).map(WordLanguageMarker::Explicit)
            }
            SuffixSpelling::Multiple(codes) => codes
                .map(LanguageCode::new)
                .collect::<Result<Vec<_>, _>>()
                .map(WordLanguageMarker::Multiple),
            SuffixSpelling::Ambiguous(codes) => codes
                .map(LanguageCode::new)
                .collect::<Result<Vec<_>, _>>()
                .map(WordLanguageMarker::Ambiguous),
        };
        match marker {
            Ok(marker) => ParseOutcome::Parsed(marker),
            Err(error) => {
                report_fault(
                    self.token,
                    &format!("Invalid @s language code: {error}"),
                    errors,
                );
                ParseOutcome::Rejected
            }
        }
    }
}

fn report_fault<'tree>(
    token: SourceBound<'tree, '_, WordLangSuffixNode<'tree>>,
    message: &str,
    errors: &impl ErrorSink,
) {
    let node = token.raw_node();
    errors.report(ParseError::new(
        ErrorCode::TreeParsingError,
        Severity::Error,
        DiagnosticLocation::new(span_of(node)),
        ErrorContext::new(
            token.source(),
            span_of(node),
            WordLangSuffixNode::KIND.name(),
        ),
        message,
    ));
}

pub(super) fn parse_language_suffix<'tree>(
    token: SourceBound<'tree, '_, WordLangSuffixNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<WordLanguageMarker> {
    match LanguageSuffix::admit(token, errors) {
        ParseOutcome::Parsed(suffix) => suffix.into_marker(errors),
        ParseOutcome::Rejected => ParseOutcome::Rejected,
    }
}
