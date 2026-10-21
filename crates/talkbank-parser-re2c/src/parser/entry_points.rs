//! Public entry points lex caller-owned source into temporary token storage.
//!
//! Chumsky parsers borrow tokens only during parsing. Returned AST slices borrow
//! the original input, assembled words included. All token and recovery
//! buffers are dropped when parsing returns.
//!
//! `parse_chat_file_to_model` also converts the AST into a fully owned model.

use crate::ast::*;
use talkbank_model::{DiagnosticLocation, ErrorSink, ParseError, Severity, Span};

use super::{dependent_tiers, file, headers, lex_to_tokens, main_tier};

// ═══════════════════════════════════════════════════════════════
// Owned-result entry point (for batch processing / ChatParser trait)
// ═══════════════════════════════════════════════════════════════

/// Parse a complete CHAT file to an owned model.
///
/// Lex → parse → convert. The intermediate AST borrows from the caller's input;
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
    parse_main_tier_with_source(input).map(|line| line.main_tier.tier().clone())
}

/// One main tier parsed from a caller's line, with what model conversion
/// needs beside the AST: the located tier and the source its slices point
/// into, already admitted as the text the converter locates against.
pub struct ParsedMainTierLine<'a> {
    /// The parsed tier with the line machine's locations for it.
    pub main_tier: crate::ast::LocatedMainTier<'a>,
    /// The caller's input, which the AST slices point into.
    pub source: crate::source_text::SourceText<'a>,
}

/// Parse a main tier and return it located, with its source.
///
/// `None` when the grammar refuses the line. The line machine then places
/// the line it accepted; that cannot fail for a line the grammar accepted,
/// since both read the same unfiltered token stream here.
pub fn parse_main_tier_with_source(input: &str) -> Option<ParsedMainTierLine<'_>> {
    use chumsky::Parser as _;
    let lexed = super::LexedSource::new(input, 0);
    let tokens = lexed.tokens();
    let source = lexed.source_text();
    let tier = main_tier::main_tier_parser(source)
        .parse(tokens)
        .into_result()
        .ok()?;
    let locations = lexed.main_tier_locations(0..tokens.len())?;
    Some(ParsedMainTierLine {
        main_tier: crate::ast::LocatedMainTier::new(tier, locations),
        source,
    })
}

/// Parse an @ID header content (after `@ID:\t`).
pub fn parse_id_header(input: &str) -> Option<IdHeaderParsed<'_>> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, crate::lexer::COND_ID_CONTENT);
    headers::id_header_parser()
        .parse(tokens.as_slice())
        .into_result()
        .ok()
}

/// Parse a @Languages header content (after `@Languages:\t`).
pub fn parse_languages_header(input: &str) -> LanguagesHeaderParsed<'_> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, crate::lexer::COND_LANGUAGES_CONTENT);
    headers::languages_header_parser()
        .parse(tokens.as_slice())
        .into_result()
        .unwrap_or_else(|_| LanguagesHeaderParsed { codes: Vec::new() })
}

/// Parse a @Participants header content (after `@Participants:\t`).
pub fn parse_participants_header<'a>(
    input: &'a str,
    errors: &impl ErrorSink,
) -> ParticipantsHeaderParsed<'a> {
    talkbank_model::validation::report_control_characters(input, errors);
    let lexed = super::LexedSource::new(input, crate::lexer::COND_PARTICIPANTS_CONTENT);
    headers::parse_participants_tokens(lexed.located(0..lexed.tokens().len()), errors)
}

/// Parse a single word (content item) from main tier content.
pub fn parse_word(input: &str) -> Option<WordWithAnnotations<'_>> {
    use chumsky::Parser as _;
    let lexed = super::LexedSource::new(input, crate::lexer::COND_MAIN_CONTENT);
    let source = lexed.source_text();
    let word_parser = chumsky::primitive::choice((
        main_tier::rich_word(source),
        main_tier::subtoken_word(source),
    ));
    let item = word_parser.parse(lexed.tokens()).into_result().ok()?;
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
        .parse(tokens.as_slice())
        .into_result()
        .ok()
}

/// Parse a single GraRelation from %gra content.
pub fn parse_gra_relation(input: &str) -> Option<GraRelationParsed<'_>> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, crate::lexer::COND_GRA_CONTENT);
    let tier = dependent_tiers::gra_tier_parser()
        .parse(tokens.as_slice())
        .into_result()
        .ok()?;
    let mut relations = tier.relations.into_iter();
    match (relations.next(), relations.next()) {
        (Some(relation), None) => Some(relation),
        _ => None,
    }
}

/// Parse a %pho tier body (content after `%pho:\t`).
pub fn parse_pho_tier(input: &str) -> PhoTier<'_> {
    parse_pho_tier_checked(input).unwrap_or_else(|| PhoTier {
        items: Vec::new(),
        terminator: None,
    })
}

/// Preserve native rejection for sink-bearing phonology fragment adapters.
pub(crate) fn parse_pho_tier_checked(input: &str) -> Option<PhoTier<'_>> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, crate::lexer::COND_PHO_CONTENT);
    dependent_tiers::pho_tier_parser()
        .parse(tokens.as_slice())
        .into_result()
        .ok()
}

/// Parse a text tier body (content after `%com:\t`, `%act:\t`, etc.).
pub fn parse_text_tier(input: &str) -> TextTierParsed<'_> {
    parse_text_tier_with_errors(input, &talkbank_model::NullErrorSink)
}

/// Parse text-tier content with inline-bullet diagnostics in source coordinates.
pub fn parse_text_tier_with_errors<'a>(
    input: &'a str,
    errors: &impl ErrorSink,
) -> TextTierParsed<'a> {
    parse_text_tier_in_condition(input, crate::lexer::COND_TIER_CONTENT, errors)
}

/// Comment fragments share the file comment grammar, including pictures.
pub(crate) fn parse_com_tier_with_errors<'a>(
    input: &'a str,
    errors: &impl ErrorSink,
) -> TextTierParsed<'a> {
    parse_text_tier_in_condition(input, crate::lexer::COND_COM_CONTENT, errors)
}

fn parse_text_tier_in_condition<'a>(
    input: &'a str,
    condition: usize,
    errors: &impl ErrorSink,
) -> TextTierParsed<'a> {
    use chumsky::Parser as _;
    talkbank_model::validation::report_control_characters(input, errors);
    let lexed = super::LexedSource::new(input, condition);
    let tokens =
        dependent_tiers::AdmittedTextTierTokens::admit(&lexed, 0..lexed.tokens().len(), errors);
    match dependent_tiers::text_tier_parser()
        .parse(tokens.tokens())
        .into_result()
    {
        Ok(tier) => tier,
        Err(_) => {
            errors.report(ParseError::new(
                talkbank_model::ErrorCode::UnparsableContent,
                Severity::Error,
                DiagnosticLocation::from_offsets(0, input.len()),
                talkbank_model::ErrorContext::new(
                    input,
                    talkbank_model::Span::new(0, input.len()),
                    "text-tier body",
                ),
                "Native text-tier grammar cannot consume the complete caller input",
            ));
            TextTierParsed {
                segments: Vec::new(),
            }
        }
    }
}

/// Admit one original dependent-tier line through the existing native dispatch,
/// without constructing a synthetic main tier or document around it.
pub(crate) fn parse_dependent_fragment<'a>(
    input: &'a str,
    errors: &impl ErrorSink,
) -> Option<DependentTierEntryParsed<'a>> {
    let lexed = lex_file_source(input, errors);
    let mut pos = 0;
    let mut tiers = file::parse_dependent_tiers(&lexed, &mut pos, errors).into_iter();
    match (tiers.next(), tiers.next()) {
        (Some(tier), None) if pos == lexed.tokens().len() => Some(tier),
        _ => None,
    }
}

/// Parse a complete CHAT file (AST, borrows from the input).
pub fn parse_chat_file(input: &str) -> ChatFile<'_> {
    parse_chat_file_streaming(input, &talkbank_model::NullErrorSink)
}

/// Parse a complete CHAT file with streaming error reporting (AST, borrows).
pub fn parse_chat_file_streaming<'a>(input: &'a str, errors: &impl ErrorSink) -> ChatFile<'a> {
    let lexed = lex_file_source(input, errors);
    file::parse_file_with_errors(&lexed, errors)
}

/// File and header-fragment admission share source-level diagnostics and one lexer run.
pub(super) fn lex_file_source<'a>(
    input: &'a str,
    errors: &impl ErrorSink,
) -> super::LexedSource<'a> {
    talkbank_model::validation::report_control_characters(input, errors);
    report_header_colon_without_tab(input, errors);
    super::LexedSource::new(input, 0)
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
        let span_start = current_line_start;
        let span_end = current_line_start + line.len();
        errors.report(
            ParseError::new(
                talkbank_model::errors::codes::ErrorCode::SyntaxError,
                Severity::Error,
                DiagnosticLocation::new(Span::new(span_start, span_end)),
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
        .parse(tokens.as_slice())
        .into_result()
        .unwrap_or_else(|_| MorTier {
            items: Vec::new(),
            terminator: None,
        })
}

/// Parse a %gra tier body, or decline it.
///
/// `None` when the body will not parse. It used to be
/// `.unwrap_or_else(|_| GraTier { relations: Vec::new() })`, so a `%gra` line
/// chumsky rejected outright came back as a tier of NO relations, with no
/// diagnostic, and the fragment entry point handed that to a caller as a
/// successful parse. That is the fabricated-value shape this crate's own
/// `%gra` lowering was cleaned of the day before, one function upstream of it,
/// and it made the lowering's completeness verdict a lie: zero relations
/// lowered from zero declared reads as WHOLE.
pub fn parse_gra_tier(input: &str) -> Option<GraTier<'_>> {
    use chumsky::Parser as _;
    let tokens = lex_to_tokens(input, crate::lexer::COND_GRA_CONTENT);
    dependent_tiers::gra_tier_parser()
        .parse(tokens.as_slice())
        .into_result()
        .ok()
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
