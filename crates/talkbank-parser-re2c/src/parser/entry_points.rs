//! Public entry point functions, each lexes input and delegates to chumsky parsers.
//!
//! These are the public API that `chat_parser_impl.rs` and tests call.
//! Each function: lex → leaked token slice → chumsky parser → AST.
//!
//! **Memory:** Entry points use `lex_to_tokens` which leaks the NUL-padded
//! source and token slice via `Box::leak`. This is acceptable for
//! small-batch use. For large corpus runs (>5k files), callers should
//! periodically fork a subprocess or accept the memory cost.
//!
//! `parse_chat_file_to_model` provides an owned-result entry point that
//! still leaks internally but is the intended API for batch processing.

use crate::ast::*;
use crate::token::Token;
use talkbank_model::{ErrorSink, ParseError, Severity, SourceLocation, Span};

use super::{dependent_tiers, file, headers, lex_to_tokens, main_tier};

// ═══════════════════════════════════════════════════════════════
// Owned-result entry point (for batch processing / ChatParser trait)
// ═══════════════════════════════════════════════════════════════

/// Parse a complete CHAT file to an owned model.
///
/// Lex → parse → convert. The intermediate AST borrows from leaked data;
/// the returned model is fully owned (all `String`s, no borrows).
pub fn parse_chat_file_to_model(
    input: &str,
    errors: &impl ErrorSink,
) -> talkbank_model::model::ChatFile {
    let ast = parse_chat_file_streaming(input, errors);
    crate::convert::chat_file_to_model(&ast, errors)
}

// ═══════════════════════════════════════════════════════════════
// AST-returning entry points (for tests and direct AST inspection)
// ═══════════════════════════════════════════════════════════════

/// Parse a main tier string starting with '*'.
pub fn parse_main_tier(input: &str) -> Option<MainTier<'_>> {
    parse_main_tier_with_source(input).map(|(tier, _source)| tier)
}

/// Parse a main tier and return the LEAKED source its slices borrow from.
///
/// The lexer NUL-pads and leaks a COPY of `input`, so the caller's own string
/// is a different allocation and `SourceText::new(input)` would place nothing.
/// A caller that wants source spans needs this one; `parse_main_tier` remains
/// for callers that do not.
pub fn parse_main_tier_with_source(input: &str) -> Option<(MainTier<'_>, &str)> {
    use chumsky::Parser as _;
    let (tokens, source) = crate::parser::lex_to_tokens_and_source(input, 0);
    main_tier::main_tier_parser()
        .parse(tokens)
        .into_result()
        .ok()
        .map(|tier| (tier, source))
}

/// Parse an @ID header content (after `@ID:\t`).
pub fn parse_id_header(input: &str) -> Option<IdHeaderParsed<'_>> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, crate::lexer::COND_ID_CONTENT);
    headers::id_header_parser().parse(tokens).into_result().ok()
}

/// Parse a @Languages header content (after `@Languages:\t`).
pub fn parse_languages_header(input: &str) -> LanguagesHeaderParsed<'_> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, crate::lexer::COND_LANGUAGES_CONTENT);
    headers::languages_header_parser()
        .parse(tokens)
        .into_result()
        .unwrap_or_else(|_| LanguagesHeaderParsed { codes: Vec::new() })
}

/// Parse a @Participants header content (after `@Participants:\t`).
pub fn parse_participants_header(input: &str) -> ParticipantsHeaderParsed<'_> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, crate::lexer::COND_PARTICIPANTS_CONTENT);
    headers::participants_header_parser()
        .parse(tokens)
        .into_result()
        .unwrap_or_else(|_| ParticipantsHeaderParsed {
            entries: Vec::new(),
        })
}

/// Parse a single word (content item) from main tier content.
pub fn parse_word(input: &str) -> Option<WordWithAnnotations<'_>> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, crate::lexer::COND_MAIN_CONTENT);
    let word_parser =
        chumsky::primitive::choice((main_tier::rich_word(), main_tier::subtoken_word()));
    let item = word_parser.parse(tokens).into_result().ok()?;
    match item {
        ContentItem::Word(w) => Some(w),
        _ => None,
    }
}

/// Parse a single MorWord from %mor content.
pub fn parse_mor_word(input: &str) -> Option<MorWordParsed<'_>> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, crate::lexer::COND_MOR_CONTENT);
    dependent_tiers::mor_word_parser()
        .parse(tokens)
        .into_result()
        .ok()
}

/// Parse a single GraRelation from %gra content.
pub fn parse_gra_relation(input: &str) -> Option<GraRelationParsed<'_>> {
    let tokens = lex_to_tokens(input, crate::lexer::COND_GRA_CONTENT);
    if let Some(Token::GraRelation {
        index,
        head,
        relation,
    }) = tokens.first().cloned()
    {
        Some(GraRelationParsed {
            index,
            head,
            relation,
        })
    } else {
        None
    }
}

/// Parse a %pho tier body (content after `%pho:\t`).
pub fn parse_pho_tier(input: &str) -> PhoTier<'_> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, crate::lexer::COND_PHO_CONTENT);
    dependent_tiers::pho_tier_parser()
        .parse(tokens)
        .into_result()
        .unwrap_or_else(|_| PhoTier {
            items: Vec::new(),
            terminator: None,
        })
}

/// Parse a text tier body (content after `%com:\t`, `%act:\t`, etc.).
pub fn parse_text_tier(input: &str) -> TextTierParsed<'_> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, crate::lexer::COND_TIER_CONTENT);
    dependent_tiers::text_tier_parser()
        .parse(tokens)
        .into_result()
        .unwrap_or_else(|_| TextTierParsed {
            segments: Vec::new(),
        })
}

/// Parse a complete CHAT file (AST, borrows from leaked data).
pub fn parse_chat_file(input: &str) -> ChatFile<'_> {
    let (tokens, source) = super::lex_to_tokens_and_source(input, 0);
    file::parse_file(tokens, source)
}

/// Parse a complete CHAT file with streaming error reporting (AST, borrows).
pub fn parse_chat_file_streaming<'a>(input: &'a str, errors: &impl ErrorSink) -> ChatFile<'a> {
    report_header_colon_without_tab(input, errors);
    let (tokens, source) = super::lex_to_tokens_and_source(input, 0);
    file::parse_file_with_errors(tokens, source, errors)
}

/// Report E303 at the source boundary before lexing malformed headers.
///
/// The re2c lexer dispatches valid headers from their complete `@Name:\t`
/// prefix. A colon followed by anything else cannot enter a header-content
/// condition, so preserving the invalid separator as ordinary tokens loses
/// the fact that this was a header separator. Inspecting the physical header
/// line here keeps that provenance available to the diagnostic sink.
fn report_header_colon_without_tab(input: &str, errors: &impl ErrorSink) {
    let mut line_start = 0usize;
    for physical_line in input.split_inclusive('\n') {
        let current_line_start = line_start;
        line_start += physical_line.len();
        let without_lf = physical_line.strip_suffix('\n').unwrap_or(physical_line);
        let line = without_lf.strip_suffix('\r').unwrap_or(without_lf);
        let Some(header) = line.strip_prefix('@') else {
            continue;
        };
        let Some(colon_in_header) = header.find(':') else {
            continue;
        };
        let name = &header[..colon_in_header];
        if name.is_empty()
            || !name
                .chars()
                .all(|character| character.is_ascii_alphabetic() || character == ' ')
        {
            continue;
        }

        let after_colon = &header[colon_in_header + 1..];
        if name == "Page"
            || (["Languages", "Date", "Media"].contains(&name)
                && after_colon
                    .bytes()
                    .all(|byte| byte == b'\t' || byte == b' '))
        {
            continue;
        }
        if after_colon.starts_with('\t') {
            continue;
        }
        let found = match after_colon.chars().next() {
            None => "Nothing".to_owned(),
            Some(' ') => "A space".to_owned(),
            Some(character) => format!("'{character}'"),
        };
        let span_start = current_line_start.min(u32::MAX as usize) as u32;
        let span_end = (current_line_start + line.len()).min(u32::MAX as usize) as u32;
        errors.report(
            ParseError::new(
                talkbank_model::errors::codes::ErrorCode::SyntaxError,
                Severity::Error,
                SourceLocation::new(Span::new(span_start, span_end)),
                None,
                format!("{found} after the header colon; CHAT requires a single TAB"),
            )
            .with_suggestion("Put exactly one TAB between the header's ':' and its value"),
        );
    }
}

/// Parse a %mor tier body.
pub fn parse_mor_tier(input: &str) -> MorTier<'_> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, crate::lexer::COND_MOR_CONTENT);
    dependent_tiers::mor_tier_parser()
        .parse(tokens)
        .into_result()
        .unwrap_or_else(|_| MorTier {
            items: Vec::new(),
            terminator: None,
        })
}

/// Parse a %gra tier body.
pub fn parse_gra_tier(input: &str) -> GraTier<'_> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, crate::lexer::COND_GRA_CONTENT);
    dependent_tiers::gra_tier_parser()
        .parse(tokens)
        .into_result()
        .unwrap_or_else(|_| GraTier {
            relations: Vec::new(),
        })
}

#[cfg(test)]
mod tests {
    use super::parse_chat_file_streaming;
    use talkbank_model::ErrorCollector;

    fn diagnostic_codes(input: &str) -> Vec<String> {
        let errors = ErrorCollector::new();
        let _file = parse_chat_file_streaming(input, &errors);
        errors
            .to_vec()
            .into_iter()
            .map(|error| error.code.to_string())
            .collect()
    }

    #[test]
    fn malformed_header_separators_report_e303() {
        for header in ["@Situation: at home", "@Comment:note", "@Comment:"] {
            let input = format!("@UTF8\n@Begin\n{header}\n*CHI:\thello .\n@End\n");
            let codes = diagnostic_codes(&input);
            assert_eq!(
                codes.iter().filter(|code| code.as_str() == "E303").count(),
                1,
                "{header}: {codes:?}"
            );
        }
    }

    #[test]
    fn valid_or_non_header_colons_do_not_report_e303() {
        for header in [
            "@Situation:\tat home",
            "@Begin",
            "@ID|x:",
            "@Languages:",
            "@Date: ",
            "@Media:\t",
            "@Page: 1",
        ] {
            let codes = diagnostic_codes(header);
            assert!(
                codes.iter().all(|code| code != "E303"),
                "{header}: {codes:?}"
            );
        }
    }
}
