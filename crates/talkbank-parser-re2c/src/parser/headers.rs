//! Chumsky parser combinators for header parsing.
//!
//! Header parsers: @ID, @Languages, @Participants.

use chumsky::prelude::*;

use crate::token::Token;

use super::dependent_tiers::{opt_newline, ws};

/// Chumsky input type.
use super::Tokens;

// ═══════════════════════════════════════════════════════════
// @ID header, extract 10 pipe-delimited fields from IdFields token
// ═══════════════════════════════════════════════════════════

/// Parse an `@ID` body directly into the shared model, without dropping fields.
pub fn id_header_parser<'tokens, 'a: 'tokens>()
-> impl Parser<'tokens, Tokens<'tokens, 'a>, talkbank_model::model::IDHeader> + Clone {
    use talkbank_model::model::{
        IDHeader, LanguageCode, LanguageCodes, ParticipantRole, Sex, SpeakerCode,
    };
    select! {
        Token::IdFields { language, corpus, speaker, age, sex, group, ses, role, education, custom }
            => (language, corpus, speaker, age, sex, group, ses, role, education, custom),
    }
    .try_map(
        |(language, corpus, speaker, age, sex, group, ses, role, education, custom), _| {
            // An observed empty field remains empty for E510. Empty elements of a
            // nonempty list instead reject; filtering them would fabricate a list.
            let languages = if language.trim().is_empty() {
                Vec::new()
            } else {
                language
                    .split(',')
                    .map(|code| LanguageCode::new(code.trim()))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| chumsky::error::EmptyErr::default())?
            };
            let mut id = IDHeader::from_languages(
                LanguageCodes::new(languages),
                SpeakerCode::new(speaker),
                ParticipantRole::new(role),
            );
            if !corpus.is_empty() {
                id = id.with_corpus(corpus);
            }
            if !age.is_empty() {
                id = id.with_age(age);
            }
            if !group.is_empty() {
                id = id.with_group(group);
            }
            if !ses.is_empty() {
                id = id.with_ses(ses);
            }
            if !education.is_empty() {
                id = id.with_education(education);
            }
            if !custom.is_empty() {
                id = id.with_custom_field(custom);
            }
            if !sex.is_empty() {
                id = id.with_sex(Sex::from_text(sex));
            }
            Ok(id)
        },
    )
    .then_ignore(ws())
    .then_ignore(opt_newline())
}

// ═══════════════════════════════════════════════════════════
// @Languages header, comma-separated language codes
// ═══════════════════════════════════════════════════════════

/// Parse a `@Languages` header content.
pub fn languages_header_parser<'tokens, 'a: 'tokens>()
-> impl Parser<'tokens, Tokens<'tokens, 'a>, talkbank_model::LanguageCodes> + Clone {
    let code = select! { Token::LanguageCode(s) => s }.try_map(|text, _| {
        talkbank_model::LanguageCode::new(text).map_err(|_| chumsky::error::EmptyErr::default())
    });
    let comma = select! { Token::Comma(_) => () };

    ws().ignore_then(
        code.separated_by(ws().then(comma).then(ws()))
            .at_least(1)
            .collect::<Vec<_>>(),
    )
    .then_ignore(ws())
    .then_ignore(opt_newline())
    .map(talkbank_model::LanguageCodes::new)
}

/// Admit exactly one Types triple; the enclosing parse requires complete input.
pub(super) fn types_header_parser<'tokens, 'a: 'tokens>()
-> impl Parser<'tokens, Tokens<'tokens, 'a>, talkbank_model::model::TypesHeader> + Clone {
    select! {
        Token::TypesFields { design, activity, group } => (design, activity, group),
    }
    .try_map(|(design, activity, group), _| {
        talkbank_model::model::TypesHeader::parse(design, activity, group)
            .map_err(|_| chumsky::error::EmptyErr::default())
    })
}

/// Parse complete media fields directly into the shared model.
pub(super) fn media_header_parser<'tokens, 'a: 'tokens>(
    source: crate::source_text::SourceText<'a>,
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, talkbank_model::model::MediaHeader> + Clone {
    use talkbank_model::model::{MediaFilename, MediaHeader, MediaStatus, MediaType};
    let field = select! { Token::MediaWord(text) => text, Token::MediaFilename(text) => text };
    let comma = select! { Token::Comma(_) => () };
    ws().ignore_then(field)
        .then(ws().to_slice())
        .then_ignore(comma)
        .then_ignore(ws())
        .then(field)
        .then(
            ws().ignore_then(comma)
                .ignore_then(ws())
                .ignore_then(field)
                .or_not(),
        )
        .then_ignore(ws())
        .try_map(move |(((filename, gap), media_type), status), _| {
            let filename =
                MediaFilename::parse(filename).map_err(|_| chumsky::error::EmptyErr::default())?;
            let mut media = MediaHeader::new(filename, MediaType::from_text(media_type));
            // E767 concerns the actual whitespace token immediately before the
            // first comma. Continuation syntax is not substituted for that gap.
            if let Some(Token::Whitespace(text)) = gap.last() {
                let span = source
                    .span_of(text)
                    .ok_or_else(chumsky::error::EmptyErr::default)?;
                media = media.with_whitespace_before_comma(talkbank_model::Location::Located(span));
            }
            if let Some(status) = status {
                media = media.with_status(MediaStatus::from_text(status));
            }
            Ok(media)
        })
}

// ═══════════════════════════════════════════════════════════
// @Participants header, comma-separated entries (SPK Name Role)
// ═══════════════════════════════════════════════════════════

/// A pending entry always has a first word; a separator belongs to the
/// completed entry before it. EOF can therefore distinguish an empty list
/// from a dangling separator without rescanning source text.
enum ParticipantListState<'a> {
    Start,
    Entry {
        first: &'a str,
        rest: Vec<&'a str>,
        first_span: crate::lexer::LexerSpan,
    },
    AfterComma(crate::lexer::LexerSpan),
}

fn report_participant_syntax(
    span: crate::lexer::LexerSpan,
    code: talkbank_model::ErrorCode,
    message: &str,
    errors: &impl talkbank_model::ErrorSink,
) {
    errors.report(talkbank_model::ParseError::new(
        code,
        talkbank_model::Severity::Error,
        talkbank_model::DiagnosticLocation::from_offsets(span.start, span.end),
        None,
        message,
    ));
}

/// One participant-list parser for file and fragment entry points. Token
/// locations originate in their owning lexer run and survive token recovery.
pub(crate) fn parse_participants_tokens<'tokens, 'source: 'tokens>(
    tokens: impl Iterator<Item = (&'tokens Token<'source>, crate::lexer::LexerSpan)>,
    errors: &impl talkbank_model::ErrorSink,
) -> talkbank_model::model::ParticipantEntries {
    use ParticipantListState::{AfterComma, Entry, Start};
    use talkbank_model::ErrorCode;

    let mut state = Start;
    let mut entries = Vec::new();
    for (token, span) in tokens {
        match token {
            Token::ParticipantWord(word) => {
                state = match state {
                    Entry {
                        first,
                        mut rest,
                        first_span,
                    } => {
                        rest.push(*word);
                        Entry {
                            first,
                            rest,
                            first_span,
                        }
                    }
                    Start | AfterComma(_) => Entry {
                        first: word,
                        rest: Vec::new(),
                        first_span: span,
                    },
                };
            }
            Token::Comma(_) => {
                state = match state {
                    Entry {
                        first,
                        rest,
                        first_span,
                    } => {
                        entries.push(finish_participant(first, rest, first_span, errors));
                        AfterComma(span)
                    }
                    Start => {
                        report_participant_syntax(
                            span,
                            ErrorCode::UnparsableContent,
                            "Expected a participant before the comma",
                            errors,
                        );
                        Start
                    }
                    AfterComma(previous) => {
                        report_participant_syntax(
                            span.clone(),
                            ErrorCode::UnparsableContent,
                            "Expected a participant between commas",
                            errors,
                        );
                        AfterComma(previous.start..span.end)
                    }
                };
            }
            // Continuations are lexer-owned separators, not participant words
            // or recovery faults. They preserve the pending entry/comma state.
            Token::Whitespace(_) | Token::Newline(_) | Token::Continuation(_) => {}
            _ => report_participant_syntax(
                span,
                ErrorCode::UnparsableContent,
                "Unexpected token in @Participants",
                errors,
            ),
        }
    }
    match state {
        Entry {
            first,
            rest,
            first_span,
        } => entries.push(finish_participant(first, rest, first_span, errors)),
        AfterComma(span) => {
            // Match the canonical parser's structural-recovery diagnostic and
            // the specific CHAT/CLAN trailing-separator rule.
            report_participant_syntax(
                span.clone(),
                ErrorCode::UnparsableContent,
                "Expected a participant after the comma",
                errors,
            );
            report_participant_syntax(
                span,
                ErrorCode::TrailingCommaInParticipants,
                "Commas at the end of the @Participants tier are not allowed",
                errors,
            );
        }
        Start => {}
    }
    talkbank_model::model::ParticipantEntries::new(entries)
}

/// Complete the observed declaration once; no empty-list or fabricated-role arm.
fn finish_participant(
    first: &str,
    rest: Vec<&str>,
    span: crate::lexer::LexerSpan,
    errors: &impl talkbank_model::ErrorSink,
) -> talkbank_model::model::ParticipantEntry {
    use talkbank_model::model::{ParticipantEntry, ParticipantName, ParticipantRole, SpeakerCode};
    match rest.split_last() {
        Some((role, names)) => ParticipantEntry::new(
            SpeakerCode::new(first),
            if names.is_empty() {
                None
            } else {
                Some(ParticipantName::new(names.join(" ")))
            },
            ParticipantRole::new(*role),
        ),
        None => {
            report_participant_syntax(
                span,
                talkbank_model::ErrorCode::EmptyParticipantRole,
                "Participant declaration has no role",
                errors,
            );
            ParticipantEntry::missing_role(SpeakerCode::new(first))
        }
    }
}

/// A page body is one admitted numeric token, not arbitrary concatenated text.
pub(crate) fn page_header_parser<'tokens, 'a: 'tokens>()
-> impl Parser<'tokens, Tokens<'tokens, 'a>, talkbank_model::model::PageNumber> + Clone {
    select! { Token::HeaderContent(text) => text }.try_map(|text, _| {
        talkbank_model::model::PageNumber::new(text)
            .map_err(|_| chumsky::error::EmptyErr::default())
    })
}
