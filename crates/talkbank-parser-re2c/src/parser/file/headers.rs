//! Content-header admission retains the exact lexer prefix and separator.

use crate::ast::{HeaderParsed, LocatedHeader};
use crate::token::{HeaderKind, HeaderPrefixToken, PrefixToken, Token};
use chumsky::Parser as _;
use talkbank_model::ErrorSink;

/// Only the four content-header prefix variants can enter this line parser.
/// Each carries the lexer-issued separator; no unrelated token can supply one.
pub(super) enum ContentHeaderPrefix<'a> {
    Named(HeaderPrefixToken<'a>),
    BirthOf(PrefixToken<'a>),
    BirthplaceOf(PrefixToken<'a>),
    L1Of(PrefixToken<'a>),
}

impl<'a> ContentHeaderPrefix<'a> {
    fn separator(&self) -> talkbank_model::model::TierSeparator {
        match self {
            Self::Named(prefix) => prefix.separator(),
            Self::BirthOf(prefix) | Self::BirthplaceOf(prefix) | Self::L1Of(prefix) => {
                prefix.separator()
            }
        }
    }

    fn into_token(self) -> Token<'a> {
        match self {
            Self::Named(prefix) => Token::HeaderPrefix(prefix),
            Self::BirthOf(prefix) => Token::HeaderBirthOf(prefix),
            Self::BirthplaceOf(prefix) => Token::HeaderBirthplaceOf(prefix),
            Self::L1Of(prefix) => Token::HeaderL1Of(prefix),
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
        let content_tokens = lexed.admit_tokens(content_start..cursor.position(), errors)?;
        let content = content_tokens.tokens();
        let admission = HeaderAdmission::parse(&prefix, content, lexed.source_text());
        let header_content = lexed.admit_region(header_start..cursor.position(), errors)?;
        let _ = cursor.take_newline();
        let header_line = lexed.admit_region(header_start..cursor.position(), errors)?;
        let header = match admission {
            HeaderAdmission::Structured(Ok(header)) => HeaderParsed::Shared(*header),
            HeaderAdmission::Structured(Err(recovery)) => {
                header_content.report_error(
                    talkbank_model::ErrorCode::UnparsableContent,
                    "Malformed or unsupported header content",
                    errors,
                );
                HeaderParsed::Shared(recovery.recover(
                    header_line.extent(),
                    header_line.source_slice().text(),
                    header_content.source_slice().text(),
                ))
            }
            HeaderAdmission::Tokens => {
                let malformed = if matches!(&prefix, ContentHeaderPrefix::Named(p) if p.kind() == HeaderKind::LazyGem && p.text().ends_with(":\t"))
                {
                    content
                        .iter()
                        .all(|token| matches!(token, Token::Whitespace(_) | Token::Continuation(_)))
                } else {
                    content.iter().any(|token| token.error_context().is_some())
                };
                if malformed {
                    header_content.report_error(
                        talkbank_model::ErrorCode::UnparsableContent,
                        "Malformed or unsupported header content",
                        errors,
                    );
                }
                if matches!(&prefix, ContentHeaderPrefix::Named(p) if p.kind() == HeaderKind::Participants)
                {
                    let findings = talkbank_model::ErrorCollector::new();
                    let entries = super::super::headers::parse_participants_tokens(
                        content_tokens.into_located(),
                        &findings,
                    );
                    let findings = findings.into_vec();
                    let rejected = malformed
                        || findings.iter().any(|finding| {
                            finding.code != talkbank_model::ErrorCode::EmptyParticipantRole
                        });
                    errors.report_all(
                        findings
                            .into_iter()
                            .filter(|finding| {
                                !rejected
                                    || finding.code
                                        != talkbank_model::ErrorCode::EmptyParticipantRole
                            })
                            .collect(),
                    );
                    HeaderParsed::Shared(if rejected {
                        talkbank_model::model::Header::UnparsedParticipants(
                            talkbank_model::model::UnparsedParticipantHeader::new(
                                header_line.extent(),
                                header_line.source_slice().text(),
                            ),
                        )
                    } else {
                        talkbank_model::model::Header::Participants { entries }
                    })
                } else {
                    // Preserve the lexer representation for remaining header families.
                    let source = header_content.source_slice().text();
                    if malformed {
                        HeaderParsed::Shared(talkbank_model::model::Header::Unknown {
                            text: talkbank_model::model::WarningText::new(source),
                            parse_reason: Some(
                                "Malformed or unsupported header content".to_owned(),
                            ),
                            suggested_fix: None,
                        })
                    } else {
                        HeaderParsed::Other {
                            source,
                            prefix: prefix.into_token(),
                            content: content.to_vec(),
                        }
                    }
                }
            }
        };
        Ok(header_line.locate_header(header, separator))
    }
}

/// A structured header has exactly one parse result and one recovery policy.
/// Other families retain their lexer representation until conversion.
enum HeaderAdmission {
    Structured(Result<Box<talkbank_model::Header>, HeaderRecovery>),
    Tokens,
}

/// Recovery is selected by the same branch that selects the body grammar.
enum HeaderRecovery {
    Types,
    Id,
    Languages,
    Media,
    Page,
}

impl HeaderAdmission {
    fn parse<'a>(
        prefix: &ContentHeaderPrefix<'a>,
        content: &[Token<'a>],
        source: crate::source_text::SourceText<'a>,
    ) -> Self {
        use super::super::headers;
        use talkbank_model::Header;
        let prefix = match prefix {
            ContentHeaderPrefix::Named(prefix) => prefix,
            ContentHeaderPrefix::BirthOf(_)
            | ContentHeaderPrefix::BirthplaceOf(_)
            | ContentHeaderPrefix::L1Of(_) => return Self::Tokens,
        };
        let parsed = match prefix.kind() {
            HeaderKind::Types => headers::types_header_parser()
                .parse(content)
                .into_result()
                .map(Header::Types)
                .map_err(|_| HeaderRecovery::Types),
            HeaderKind::Id => headers::id_header_parser()
                .parse(content)
                .into_result()
                .map(Header::ID)
                .map_err(|_| HeaderRecovery::Id),
            HeaderKind::Languages => headers::languages_header_parser()
                .parse(content)
                .into_result()
                .map(|codes| Header::Languages { codes })
                .map_err(|_| HeaderRecovery::Languages),
            HeaderKind::Media => headers::media_header_parser(source)
                .parse(content)
                .into_result()
                .map(Header::Media)
                .map_err(|_| HeaderRecovery::Media),
            HeaderKind::Page => headers::page_header_parser()
                .parse(content)
                .into_result()
                .map(|page| Header::Page { page })
                .map_err(|_| HeaderRecovery::Page),
            HeaderKind::Comment
            | HeaderKind::Participants
            | HeaderKind::Date
            | HeaderKind::Options
            | HeaderKind::RecordingQuality
            | HeaderKind::Transcription
            | HeaderKind::Number
            | HeaderKind::Location
            | HeaderKind::Situation
            | HeaderKind::Activities
            | HeaderKind::RoomLayout
            | HeaderKind::TapeLocation
            | HeaderKind::TimeDuration
            | HeaderKind::TimeStart
            | HeaderKind::Transcriber
            | HeaderKind::Warning
            | HeaderKind::Videos
            | HeaderKind::Thumbnail
            | HeaderKind::T
            | HeaderKind::Bck
            | HeaderKind::Pid
            | HeaderKind::Font
            | HeaderKind::Window
            | HeaderKind::ColorWords
            | HeaderKind::BeginGem
            | HeaderKind::EndGem
            | HeaderKind::LazyGem
            | HeaderKind::Unknown => return Self::Tokens,
        };
        Self::Structured(parsed.map(Box::new))
    }
}

impl HeaderRecovery {
    fn recover(
        self,
        extent: talkbank_model::Span,
        line: &str,
        content: &str,
    ) -> talkbank_model::Header {
        use talkbank_model::{Header, model};
        match self {
            Self::Types => Self::unknown(content, "Malformed @Types header"),
            Self::Id => Header::UnparsedID(model::UnparsedIDHeader::new(extent, line)),
            Self::Languages => Header::UnparsedLanguages(model::UnparsedLanguageHeader::new(
                extent,
                line,
                model::LanguageHeaderFailure::List,
            )),
            Self::Media => Self::unknown(content, "Malformed or unsupported header content"),
            Self::Page => Self::unknown(content, "Malformed @Page number"),
        }
    }

    fn unknown(content: &str, reason: &str) -> talkbank_model::Header {
        use talkbank_model::{Header, model};
        Header::Unknown {
            text: model::WarningText::new(content),
            parse_reason: Some(reason.to_owned()),
            suggested_fix: None,
        }
    }
}
