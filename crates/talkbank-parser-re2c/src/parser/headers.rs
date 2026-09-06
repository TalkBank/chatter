//! Chumsky parser combinators for header parsing.
//!
//! Header parsers: @ID, @Languages, @Participants.

use chumsky::prelude::*;

use crate::ast::*;
use crate::token::Token;

use super::dependent_tiers::{opt_newline, ws};

/// Chumsky input type.
use super::Tokens;

// ═══════════════════════════════════════════════════════════
// @ID header, extract 10 pipe-delimited fields from IdFields token
// ═══════════════════════════════════════════════════════════

/// Parse an `@ID` header content.
pub fn id_header_parser<'tokens, 'a: 'tokens>()
-> impl Parser<'tokens, Tokens<'tokens, 'a>, IdHeaderParsed<'a>> + Clone {
    select! {
        Token::IdFields { language, corpus, speaker, age, sex, group, ses, role, education, custom }
            => IdHeaderParsed { language, corpus, speaker, age, sex, group, ses, role, education, custom_field: custom },
    }
    .then_ignore(ws())
    .then_ignore(opt_newline())
}

// ═══════════════════════════════════════════════════════════
// @Languages header, comma-separated language codes
// ═══════════════════════════════════════════════════════════

/// Parse a `@Languages` header content.
pub fn languages_header_parser<'tokens, 'a: 'tokens>()
-> impl Parser<'tokens, Tokens<'tokens, 'a>, LanguagesHeaderParsed<'a>> + Clone {
    let code = select! { Token::LanguageCode(s) => s };
    let comma = select! { Token::Comma(_) => () };

    ws().ignore_then(
        code.separated_by(ws().then(comma).then(ws()))
            .allow_trailing()
            .collect::<Vec<_>>(),
    )
    .then_ignore(ws())
    .then_ignore(opt_newline())
    .map(|codes| LanguagesHeaderParsed { codes })
}

// ═══════════════════════════════════════════════════════════
// @Participants header, comma-separated entries (SPK Name Role)
// ═══════════════════════════════════════════════════════════

/// A pending entry always has a first word; a separator belongs to the
/// completed entry before it. EOF can therefore distinguish an empty list
/// from a dangling separator without rescanning source text.
enum ParticipantListState<'a> {
    Start,
    Entry { first: &'a str, rest: Vec<&'a str> },
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
        talkbank_model::SourceLocation::from_offsets(span.start, span.end),
        None,
        message,
    ));
}

/// One participant-list parser for file and fragment entry points. Token
/// locations originate in their owning lexer run and survive token recovery.
pub(crate) fn parse_participants_tokens<'tokens, 'source: 'tokens>(
    tokens: impl Iterator<Item = (&'tokens Token<'source>, crate::lexer::LexerSpan)>,
    errors: &impl talkbank_model::ErrorSink,
) -> ParticipantsHeaderParsed<'source> {
    use ParticipantListState::{AfterComma, Entry, Start};
    use talkbank_model::ErrorCode;

    let mut state = Start;
    let mut entries = Vec::new();
    for (token, span) in tokens {
        match token {
            Token::ParticipantWord(word) => {
                state = match state {
                    Entry { first, mut rest } => {
                        rest.push(*word);
                        Entry { first, rest }
                    }
                    Start | AfterComma(_) => Entry {
                        first: word,
                        rest: Vec::new(),
                    },
                };
            }
            Token::Comma(_) => {
                state = match state {
                    Entry { first, rest } => {
                        entries.push(ParticipantEntryParsed {
                            words: std::iter::once(first).chain(rest).collect(),
                        });
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
            Token::Whitespace(_) | Token::Newline(_) => {}
            _ => report_participant_syntax(
                span,
                ErrorCode::UnparsableContent,
                "Unexpected token in @Participants",
                errors,
            ),
        }
    }
    match state {
        Entry { first, rest } => entries.push(ParticipantEntryParsed {
            words: std::iter::once(first).chain(rest).collect(),
        }),
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
    ParticipantsHeaderParsed { entries }
}
