//! Content-header admission retains the exact lexer prefix and separator, and
//! lowers the line to the shared model once.
//!
//! The lexer's rule names the header ([`HeaderPrefixFamily`], whose known
//! headers map once to the model's kind through [`LexedHeader::route`]) and
//! carries its separator evidence: required for every rule that matched a
//! colon and tab, optional only for the gem markers. The content's text, the
//! exact source bytes after the separator, is classified once by the model's
//! one emptiness decision ([`HeaderContent::classify`]), with no allocation:
//! the content tokens tile the source contiguously (every lexer rule starts
//! where the previous one ended), so the text is the source slice they
//! cover. Malformed content (a token the lexer marked) is refused before
//! anything else, blank or not, exactly as tree-sitter refuses a body with a
//! recovery node in it; otherwise blank content is the empty state, and
//! content builds the header through the model's own constructors for the
//! kind.

use crate::ast::LocatedHeader;
use crate::token::{
    Declaration, HeaderPrefixFamily, HeaderPrefixToken, HeaderRoute, LexedHeader,
    SeparatedPrefixToken, Token,
};
use chumsky::Parser as _;
use talkbank_model::model::{
    BlankContent, ChatOptionFlags, CommentContent, GemKind, Header, HeaderContent, HeaderText,
    PlainKind, QualifiedKind, RecoveryText, SpeakerCode, StructuredKind, TextKind,
    WrittenSeparator,
};
use talkbank_model::{ErrorCode, ErrorSink};

/// Only the four content-header prefix variants can enter this line parser.
/// Each carries the lexer-issued separator; no unrelated token can supply one.
pub(super) enum ContentHeaderPrefix<'a> {
    Named(HeaderPrefixToken<'a>),
    BirthOf(SeparatedPrefixToken<'a>),
    BirthplaceOf(SeparatedPrefixToken<'a>),
    L1Of(SeparatedPrefixToken<'a>),
}

/// What a content header line is to the model, decided once from its prefix,
/// with the separator evidence each kind can have: a text, structured or
/// qualified header's rule always matched its colon and tab, so only a gem
/// can be without one.
enum LineKind<'a> {
    /// A header whose whole content is one text or scalar value.
    Text(TextKind, WrittenSeparator),
    /// A header whose content has its own grammar.
    Structured(StructuredKind, WrittenSeparator),
    /// A gem marker, with its colon and tab or bare.
    Gem(GemKind, Option<WrittenSeparator>),
    /// A participant-qualified header and the participant it names.
    Qualified(QualifiedKind, &'a str, WrittenSeparator),
    /// A declaration with its own admission state.
    Declaration(Declaration),
    /// A header name the model does not carry.
    Unsupported,
}

/// A content line, its source regions admitted once by the lexer owner.
struct Line<'r, 'a> {
    /// The content tokens after the separator, before the line ending.
    content: crate::parser::located::LocatedTokens<'r, 'a>,
    /// The content's exact source text (empty when there are no tokens).
    text: &'a str,
    /// The header from its prefix through its content.
    header_content: crate::parser::located::NonEmptyRegion<'r, 'a>,
    /// The whole header line, including its ending.
    header_line: crate::parser::located::NonEmptyRegion<'r, 'a>,
    /// The original source, for placing comment bullets.
    source: crate::source_text::SourceText<'a>,
}

impl Line<'_, '_> {
    /// Whether the lexer marked any content token malformed.
    fn malformed(&self) -> bool {
        self.content
            .tokens()
            .iter()
            .any(|token| token.error_context().is_some())
    }

    /// Report malformed content (E316) and keep the header's source as an
    /// unknown header with `reason`.
    fn refuse(&self, reason: &str, errors: &impl ErrorSink) -> Header {
        self.header_content
            .report_error(ErrorCode::UnparsableContent, reason, errors);
        self.unknown(Some(reason))
    }

    /// The header's source, kept as an unknown header.
    fn unknown(&self, reason: Option<&str>) -> Header {
        Header::Unknown {
            text: RecoveryText::observed(self.header_content.source_slice().text()),
            parse_reason: reason.map(str::to_owned),
            suggested_fix: None,
        }
    }
}

/// The reason recorded when content is malformed.
const MALFORMED: &str = talkbank_model::model::MALFORMED_HEADER_CONTENT;

impl<'a> ContentHeaderPrefix<'a> {
    fn separator(&self) -> talkbank_model::model::TierSeparator {
        match self {
            Self::Named(prefix) => prefix.separator(),
            Self::BirthOf(prefix) | Self::BirthplaceOf(prefix) | Self::L1Of(prefix) => {
                prefix.separator()
            }
        }
    }

    fn line_kind(&self) -> LineKind<'a> {
        let qualified = |kind, prefix: &SeparatedPrefixToken<'a>| {
            LineKind::Qualified(kind, prefix.text(), prefix.written_separator())
        };
        match self {
            Self::Named(prefix) => match prefix.family() {
                HeaderPrefixFamily::Separated(header, separator) => {
                    Self::separated_kind(header, separator)
                }
                HeaderPrefixFamily::Gem(gem, separator) => LineKind::Gem(gem, separator),
                HeaderPrefixFamily::Unknown => LineKind::Unsupported,
            },
            Self::BirthOf(prefix) => qualified(QualifiedKind::Birth, prefix),
            Self::BirthplaceOf(prefix) => qualified(QualifiedKind::Birthplace, prefix),
            Self::L1Of(prefix) => qualified(QualifiedKind::L1Of, prefix),
        }
    }

    /// The model meaning of a known header written with its separator.
    fn separated_kind(header: LexedHeader, separator: WrittenSeparator) -> LineKind<'a> {
        match header.route() {
            HeaderRoute::Text(kind) => LineKind::Text(kind, separator),
            HeaderRoute::Structured(kind) => LineKind::Structured(kind, separator),
            HeaderRoute::Declaration(declaration) => LineKind::Declaration(declaration),
            HeaderRoute::Unsupported => LineKind::Unsupported,
        }
    }

    pub(super) fn parse_line(
        self,
        cursor: &mut super::super::TokenCursor<'_, 'a>,
        errors: &impl ErrorSink,
    ) -> Result<LocatedHeader<'a>, super::super::located::ReportedRegionFailure> {
        let prefix = self;
        let lexed = cursor.owner();
        let header_start = cursor.position();
        cursor.advance();
        let separator = prefix.separator();
        let separator = if let Some(Token::HeaderSep(sep)) = cursor.peek().map(|next| next.token())
        {
            cursor.advance();
            sep.separator()
        } else {
            separator
        };
        let content_start = cursor.position();
        cursor.skip_to_newline();
        let content_end = cursor.position();
        let content = lexed.admit_tokens(content_start..content_end, errors)?;
        let text = lexed
            .optional_region(content_start..content_end, errors)?
            .map_or("", |region| region.source_slice().text());
        let header_content = lexed.admit_region(header_start..content_end, errors)?;
        let _ = cursor.take_newline();
        let header_line = lexed.admit_region(header_start..cursor.position(), errors)?;
        let line = Line {
            content,
            text,
            header_content,
            header_line,
            source: lexed.source_text(),
        };
        let header = prefix.lower(&line, errors);
        Ok(line.header_line.locate_header(header, separator))
    }

    /// The one lowering: the line's kind, then malformed content refused,
    /// then the one emptiness decision, then the kind's own constructor.
    ///
    /// Malformed content is refused before any reader sees it, for every
    /// kind that has content, structured ones included: a reader given the
    /// tokens around a malformed one could build a header from part of the
    /// line, and tree-sitter refuses a body with an ERROR node in or beside
    /// it the same way, so both keep the line as an unknown header with the
    /// same reason.
    fn lower(&self, line: &Line<'_, 'a>, errors: &impl ErrorSink) -> Header {
        let content = HeaderContent::classify(line.text);
        match self.line_kind() {
            LineKind::Declaration(declaration) => lower_declaration(declaration, line, errors),
            LineKind::Text(..)
            | LineKind::Structured(..)
            | LineKind::Gem(..)
            | LineKind::Qualified(..)
            | LineKind::Unsupported
                if line.malformed() =>
            {
                line.refuse(MALFORMED, errors)
            }
            LineKind::Unsupported => line.unknown(None),
            LineKind::Structured(kind, separator) => match content {
                HeaderContent::Blank => PlainKind::Structured(kind).empty(separator),
                HeaderContent::Text(text) => lower_structured(kind, text, line, errors),
            },
            LineKind::Text(kind, separator) => match content {
                HeaderContent::Blank => PlainKind::Text(kind).empty(separator),
                HeaderContent::Text(text) => kind.header(text),
            },
            LineKind::Gem(gem, separator) => match (content, separator) {
                (HeaderContent::Blank, Some(separator)) => PlainKind::Gem(gem).empty(separator),
                // No colon at all: the gem's legal bare form.
                (HeaderContent::Blank, None) => gem.bare(),
                (HeaderContent::Text(text), Some(_)) => gem.labelled(text),
                // A bare gem marker carries no content; content after one is
                // malformed.
                (HeaderContent::Text(_), None) => line.refuse(MALFORMED, errors),
            },
            LineKind::Qualified(kind, participant, separator) => match content {
                HeaderContent::Blank => kind.empty(SpeakerCode::new(participant), separator),
                HeaderContent::Text(text) => kind.header(SpeakerCode::new(participant), text),
            },
        }
    }
}

/// A structured header with content, read by its own grammar. No content
/// token is malformed: [`ContentHeaderPrefix::lower`] refused that line first.
fn lower_structured(
    kind: StructuredKind,
    text: HeaderText<'_>,
    line: &Line<'_, '_>,
    errors: &impl ErrorSink,
) -> Header {
    use super::super::headers;
    let tokens = line.content.tokens();
    match kind {
        // Token grammars read their tokens themselves: the parser's failure
        // is the recovery.
        StructuredKind::Types => headers::types_header_parser()
            .parse(tokens)
            .into_result()
            .map_or_else(
                |_| structured_failure(line, "Malformed @Types header", errors),
                Header::Types,
            ),
        StructuredKind::Media => headers::media_header_parser(line.source)
            .parse(tokens)
            .into_result()
            .map_or_else(
                |_| structured_failure(line, MALFORMED, errors),
                Header::Media,
            ),
        StructuredKind::Page => headers::page_header_parser()
            .parse(tokens)
            .into_result()
            .map_or_else(
                |_| structured_failure(line, "Malformed @Page number", errors),
                |page| Header::Page { page },
            ),
        StructuredKind::Comment => match CommentContent::try_from_bullets(
            crate::convert::tokens_to_bullet_content(tokens, line.source),
        ) {
            Ok(content) => Header::Comment { content },
            // Nonblank text that lowers to content writing nothing lost what
            // was written: malformed, never a blank comment.
            Err(BlankContent) => line.refuse(MALFORMED, errors),
        },
        StructuredKind::Options => match ChatOptionFlags::parse(text) {
            Ok(options) => Header::Options { options },
            Err(malformed) => line.refuse(&malformed.to_string(), errors),
        },
    }
}

/// A structured body grammar refused the content: E316, and the header kept
/// as an unknown header with the grammar's reason.
fn structured_failure(line: &Line<'_, '_>, reason: &str, errors: &impl ErrorSink) -> Header {
    line.header_content
        .report_error(ErrorCode::UnparsableContent, MALFORMED, errors);
    line.unknown(Some(reason))
}

/// The declarations keep their own admission states and codes, including
/// for an empty declaration.
fn lower_declaration(
    declaration: Declaration,
    line: &Line<'_, '_>,
    errors: &impl ErrorSink,
) -> Header {
    use super::super::headers;
    use talkbank_model::model;
    let tokens = line.content.tokens();
    let extent = line.header_line.extent();
    let line_text = line.header_line.source_slice().text();
    let report = || {
        line.header_content
            .report_error(ErrorCode::UnparsableContent, MALFORMED, errors);
    };
    match declaration {
        Declaration::Id => headers::id_header_parser()
            .parse(tokens)
            .into_result()
            .map_or_else(
                |_| {
                    report();
                    Header::UnparsedID(model::UnparsedIDHeader::new(extent, line_text))
                },
                Header::ID,
            ),
        Declaration::Languages => headers::languages_header_parser()
            .parse(tokens)
            .into_result()
            .map_or_else(
                |_| {
                    report();
                    Header::UnparsedLanguages(model::UnparsedLanguageHeader::new(
                        extent,
                        line_text,
                        model::LanguageHeaderFailure::List,
                    ))
                },
                |codes| Header::Languages { codes },
            ),
        Declaration::Participants => {
            let malformed = line.malformed();
            if malformed {
                report();
            }
            let findings = talkbank_model::ErrorCollector::new();
            let entries = headers::parse_participants_tokens(line.content.located(), &findings);
            let findings = findings.into_vec();
            let rejected = malformed
                || findings
                    .iter()
                    .any(|finding| finding.code != ErrorCode::EmptyParticipantRole);
            errors.report_all(
                findings
                    .into_iter()
                    .filter(|finding| !rejected || finding.code != ErrorCode::EmptyParticipantRole)
                    .collect(),
            );
            if rejected {
                Header::UnparsedParticipants(model::UnparsedParticipantHeader::new(
                    extent, line_text,
                ))
            } else {
                Header::Participants { entries }
            }
        }
    }
}
