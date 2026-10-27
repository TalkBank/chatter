//! Public entry points lex caller-owned source into temporary token storage.
//!
//! Chumsky parsers borrow tokens only during parsing. Parser-specific source
//! carriers borrow the original input; text tiers return shared BulletContent
//! directly. Token and recovery buffers are dropped when parsing returns.
//!
//! `parse_chat_file_to_model` also converts the AST into a fully owned model.

use crate::ast::*;
use talkbank_model::{DiagnosticLocation, ErrorSink, ParseError, ParseOutcome, Severity, Span};

use super::{dependent_tiers, file, headers, lex_to_tokens, main_tier};

/// Caller input admitted against the shared control policy before lexing.
/// This proves only the shared control-character policy, not fragment grammar.
struct ControlCheckedFragment<'source> {
    source: &'source str,
}

impl<'source> ControlCheckedFragment<'source> {
    fn admit(source: &'source str) -> Option<Self> {
        let findings = talkbank_model::ErrorCollector::new();
        talkbank_model::validation::report_control_characters(source, &findings);
        if findings.has_errors() {
            None
        } else {
            Some(Self { source })
        }
    }

    fn lex(self, condition: crate::lexer::StartCondition) -> super::LexedSource<'source> {
        super::LexedSource::new(self.source, condition)
    }
}

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
}

impl<'a> ParsedMainTierLine<'a> {
    /// The original source owned by the located tier, never a replacement input.
    pub fn source(&self) -> crate::source_text::SourceText<'a> {
        self.main_tier.source()
    }
}

/// Parse a main tier and return it located, with its source.
///
/// `None` when the grammar refuses the line. The line machine then places
/// the line it accepted; that cannot fail for a line the grammar accepted,
/// since both read the same unfiltered token stream here.
pub fn parse_main_tier_with_source(input: &str) -> Option<ParsedMainTierLine<'_>> {
    use chumsky::Parser as _;
    let lexed = ControlCheckedFragment::admit(input)?.lex(crate::lexer::COND_INITIAL);
    // This AST-only convenience API has no diagnostic sink. Both an invalid
    // lexer range and grammar refusal remain rejection, never fabricated text.
    let region = match lexed.whole_region() {
        Ok(region) => region?,
        Err(_) => return None,
    };
    let source = region.source_text();
    let tier = main_tier::main_tier_parser(source)
        .parse(region.tokens())
        .into_result()
        .ok()?;
    let locations = region.main_tier_locations()?;
    Some(ParsedMainTierLine {
        main_tier: crate::ast::LocatedMainTier::new(tier, locations, region.source_slice()),
    })
}

/// Parse an @ID header content (after `@ID:\t`).
pub fn parse_id_header(input: &str) -> Option<talkbank_model::model::IDHeader> {
    use chumsky::Parser as _;
    let lexed = ControlCheckedFragment::admit(input)?.lex(crate::lexer::COND_ID_CONTENT);
    headers::id_header_parser()
        .parse(lexed.tokens())
        .into_result()
        .ok()
}

/// Parse a complete @Languages body into shared codes, or explicitly reject it.
pub fn parse_languages_header(input: &str) -> ParseOutcome<talkbank_model::LanguageCodes> {
    parse_languages_header_with_errors(input, &talkbank_model::NullErrorSink)
}

/// Parse a language-list body with diagnostics in caller-input coordinates.
/// Syntax failure never becomes an empty successful language list.
pub fn parse_languages_header_with_errors(
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<talkbank_model::LanguageCodes> {
    use chumsky::Parser as _;
    let findings = talkbank_model::ErrorCollector::new();
    talkbank_model::validation::report_control_characters(input, &findings);
    let tokens = lex_to_tokens(input, crate::lexer::COND_LANGUAGES_CONTENT);
    let parsed = headers::languages_header_parser()
        .parse(tokens.as_slice())
        .into_result();
    let outcome = match parsed {
        Ok(codes) if !findings.has_errors() => ParseOutcome::Parsed(codes),
        Ok(_) => ParseOutcome::Rejected,
        Err(_) => {
            findings.report(ParseError::at_span(
                talkbank_model::ErrorCode::UnparsableContent,
                Severity::Error,
                Span::new(0, input.len()),
                "Expected a complete nonempty comma-separated language list",
            ));
            ParseOutcome::Rejected
        }
    };
    errors.report_all(findings.into_vec());
    outcome
}

/// Parse a @Participants header content (after `@Participants:\t`).
pub fn parse_participants_header(
    input: &str,
    errors: &impl ErrorSink,
) -> talkbank_model::model::ParticipantEntries {
    talkbank_model::validation::report_control_characters(input, errors);
    let lexed = super::LexedSource::new(input, crate::lexer::COND_PARTICIPANTS_CONTENT);
    headers::parse_participants_tokens(lexed.located(), errors)
}

/// Parse a single word (content item) from main tier content.
pub fn parse_word(input: &str) -> Option<WordWithAnnotations<'_>> {
    use chumsky::Parser as _;
    let lexed = ControlCheckedFragment::admit(input)?.lex(crate::lexer::COND_MAIN_CONTENT);
    let source = lexed.source_text();
    let word_parser = main_tier::rich_word(source);
    let item = word_parser.parse(lexed.tokens()).into_result().ok()?;
    match item {
        ContentItem::Word(w) => Some(w),
        _ => None,
    }
}

/// Parse the lexical word required by `ChatParser`, excluding attached syntax.
/// The rich-word API above retains annotations in its native carrier instead.
pub(crate) fn parse_standalone_word(input: &str) -> Option<talkbank_model::Word> {
    use chumsky::Parser as _;
    let lexed = ControlCheckedFragment::admit(input)?.lex(crate::lexer::COND_MAIN_CONTENT);
    main_tier::standalone_word(lexed.source_text())
        .parse(lexed.tokens())
        .into_result()
        .ok()
}

/// Parse a shared MOR word, rejecting lexical or grammar failures explicitly.
pub fn parse_mor_word(input: &str) -> ParseOutcome<talkbank_model::MorWord> {
    parse_mor_word_with_errors(input, &talkbank_model::NullErrorSink)
}

/// Parse a shared MOR word with diagnostics in caller-input coordinates.
pub fn parse_mor_word_with_errors(
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<talkbank_model::MorWord> {
    use chumsky::Parser as _;
    let findings = talkbank_model::ErrorCollector::new();
    talkbank_model::validation::report_control_characters(input, &findings);
    let tokens = lex_to_tokens(input, crate::lexer::COND_MOR_CONTENT);
    let parsed = dependent_tiers::mor_word_parser()
        .parse(tokens.as_slice())
        .into_result();
    admit_mor_fragment(input, parsed, findings, errors)
}

/// Parse exactly one shared grammatical relation.
pub fn parse_gra_relation(input: &str) -> ParseOutcome<talkbank_model::GrammaticalRelation> {
    parse_gra_relation_with_errors(input, &talkbank_model::NullErrorSink)
}

/// Parse one shared relation, retaining diagnostics in caller-input coordinates.
pub fn parse_gra_relation_with_errors(
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<talkbank_model::GrammaticalRelation> {
    let findings = talkbank_model::ErrorCollector::new();
    let parsed = parse_gra_tier_with_errors(input, &findings);
    let outcome = match parsed {
        ParseOutcome::Parsed(tier) if tier.relations().len() == 1 => {
            ParseOutcome::Parsed(tier.relations()[0].clone())
        }
        ParseOutcome::Parsed(_) | ParseOutcome::Rejected => {
            if !findings.has_errors() {
                findings.report(ParseError::at_span(
                    talkbank_model::ErrorCode::ParseFailed,
                    Severity::Error,
                    Span::new(0, input.len()),
                    "Expected exactly one GRA relation",
                ));
            }
            ParseOutcome::Rejected
        }
    };
    errors.report_all(findings.into_vec());
    outcome
}

/// Parse a `%pho` body directly into the shared model, retaining rejection.
/// An empty body can be represented; this outcome does not certify validity.
pub fn parse_pho_tier(input: &str) -> ParseOutcome<talkbank_model::PhoTier> {
    use chumsky::Parser as _;
    let Some(source) = ControlCheckedFragment::admit(input) else {
        return ParseOutcome::rejected();
    };
    let lexed = source.lex(crate::lexer::COND_PHO_CONTENT);
    let extent = lexed.source_text().whole();
    match dependent_tiers::pho_tier_parser(extent, talkbank_model::PhoTierType::Pho)
        .parse(lexed.tokens())
        .into_result()
    {
        Ok(tier) => ParseOutcome::parsed(tier),
        Err(_) => ParseOutcome::rejected(),
    }
}

/// Parse a `%sin` body directly into the shared model, retaining rejection.
pub fn parse_sin_tier(input: &str) -> ParseOutcome<talkbank_model::SinTier> {
    use chumsky::Parser as _;
    let Some(source) = ControlCheckedFragment::admit(input) else {
        return ParseOutcome::rejected();
    };
    let lexed = source.lex(crate::lexer::COND_SIN_CONTENT);
    let extent = lexed.source_text().whole();
    match dependent_tiers::sin_tier_parser(extent)
        .parse(lexed.tokens())
        .into_result()
    {
        Ok(tier) => ParseOutcome::parsed(tier),
        Err(_) => ParseOutcome::rejected(),
    }
}

/// Parse a text tier body (content after `%com:\t`, `%act:\t`, etc.).
pub fn parse_text_tier(input: &str) -> ParseOutcome<talkbank_model::BulletContent> {
    parse_text_tier_with_errors(input, &talkbank_model::NullErrorSink)
}

/// Parse text-tier content with inline-bullet diagnostics in source coordinates.
/// Grammar refusal is `Rejected`, never a successful empty-content fallback.
pub fn parse_text_tier_with_errors(
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<talkbank_model::BulletContent> {
    parse_text_tier_in_condition(input, crate::lexer::COND_TIER_CONTENT, errors)
}

/// Comment fragments share the file comment grammar, including pictures.
pub(crate) fn parse_com_tier_with_errors(
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<talkbank_model::BulletContent> {
    parse_text_tier_in_condition(input, crate::lexer::COND_COM_CONTENT, errors)
}

fn parse_text_tier_in_condition(
    input: &str,
    condition: crate::lexer::StartCondition,
    errors: &impl ErrorSink,
) -> ParseOutcome<talkbank_model::BulletContent> {
    use chumsky::Parser as _;
    let findings = talkbank_model::ErrorCollector::new();
    talkbank_model::validation::report_control_characters(input, &findings);
    let lexed = super::LexedSource::new(input, condition);
    let tokens = dependent_tiers::DiagnosedTextTierTokens::admit(lexed.whole_tokens(), &findings);
    let tokens = match tokens {
        ParseOutcome::Parsed(tokens) => tokens,
        ParseOutcome::Rejected => {
            return admit_fragment_diagnostics(ParseOutcome::Rejected, findings, errors);
        }
    };
    let parsed = match dependent_tiers::text_tier_parser()
        .parse(tokens.tokens())
        .into_result()
    {
        Ok(body) => ParseOutcome::parsed(body),
        Err(_) => {
            findings.report(ParseError::new(
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
            ParseOutcome::rejected()
        }
    };
    // Filtered lexical/bullet errors cannot become successful shortened content.
    admit_fragment_diagnostics(parsed, findings, errors).map(|tokens| {
        crate::convert::tokens_to_bullet_content(tokens, crate::source_text::SourceText::new(input))
    })
}

fn admit_fragment_diagnostics<T>(
    parsed: ParseOutcome<T>,
    findings: talkbank_model::ErrorCollector,
    errors: &impl ErrorSink,
) -> ParseOutcome<T> {
    match talkbank_model::CompletedDiagnostics::admit(findings.into_vec()) {
        Ok(completed) => {
            let outcome = if completed
                .diagnostics()
                .iter()
                .any(|error| error.severity == Severity::Error)
            {
                ParseOutcome::rejected()
            } else {
                parsed
            };
            errors.report_all(completed.into_diagnostics());
            outcome
        }
        Err(failure) => {
            errors.report_all(failure.into_diagnostics());
            ParseOutcome::rejected()
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
    let mut cursor = lexed.cursor();
    let mut tiers = file::parse_dependent_tiers(&mut cursor, errors).into_iter();
    match (tiers.next(), tiers.next()) {
        (Some(tier), None) if cursor.is_finished() => Some(tier),
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
    super::LexedSource::new(input, crate::lexer::COND_INITIAL)
}

/// Report E303 at the source boundary before lexing malformed headers.
///
/// The re2c lexer dispatches valid headers from their complete `@Name:\t`
/// prefix. A colon followed by anything else cannot enter a header-content
/// condition, so preserving the invalid separator as ordinary tokens loses
/// the fact that this was a header separator. Inspecting the physical header
/// line here keeps that provenance available to the diagnostic sink.
fn report_header_colon_without_tab(input: &str, errors: &impl ErrorSink) {
    for physical_line in crate::source_text::SourceText::new(input).physical_lines() {
        let line = physical_line.text();
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
        errors.report(
            ParseError::new(
                talkbank_model::errors::codes::ErrorCode::SyntaxError,
                Severity::Error,
                DiagnosticLocation::new(physical_line.span()),
                None,
                format!("{found} after the header colon; CHAT requires a single TAB"),
            )
            .with_suggestion("Put exactly one TAB between the header's ':' and its value"),
        );
    }
}

/// Parse a shared MOR tier. Failure never becomes an empty successful tier.
pub fn parse_mor_tier(input: &str) -> ParseOutcome<talkbank_model::MorTier> {
    parse_mor_tier_with_errors(input, &talkbank_model::NullErrorSink)
}

/// Parse a shared MOR tier with diagnostics in caller-input coordinates.
pub fn parse_mor_tier_with_errors(
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<talkbank_model::MorTier> {
    use chumsky::Parser as _;
    let findings = talkbank_model::ErrorCollector::new();
    talkbank_model::validation::report_control_characters(input, &findings);
    let lexed = super::LexedSource::new(input, crate::lexer::COND_MOR_CONTENT);
    let source = lexed.source_text();
    let parsed = dependent_tiers::mor_tier_parser(source, source.whole().into())
        .parse(lexed.tokens())
        .into_result();
    admit_mor_fragment(input, parsed, findings, errors)
}

fn admit_mor_fragment<T, E>(
    input: &str,
    parsed: Result<T, E>,
    findings: talkbank_model::ErrorCollector,
    errors: &impl ErrorSink,
) -> ParseOutcome<T> {
    let parsed = match parsed {
        Ok(value) => ParseOutcome::parsed(value),
        Err(_) => {
            findings.report(ParseError::new(
                talkbank_model::ErrorCode::UnparsableContent,
                Severity::Error,
                DiagnosticLocation::from_offsets(0, input.len()),
                talkbank_model::ErrorContext::new(input, Span::new(0, input.len()), "MOR fragment"),
                "Native MOR grammar cannot consume the complete caller input",
            ));
            ParseOutcome::rejected()
        }
    };
    admit_fragment_diagnostics(parsed, findings, errors)
}

/// Parse a %gra tier body, or decline it.
///
/// Rejected relations reject the entire tier; no partial graph is returned.
pub fn parse_gra_tier(input: &str) -> ParseOutcome<talkbank_model::GraTier> {
    parse_gra_tier_with_errors(input, &talkbank_model::NullErrorSink)
}

/// Parse a shared GRA tier, reporting numeric errors before rejecting construction.
pub fn parse_gra_tier_with_errors(
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<talkbank_model::GraTier> {
    use chumsky::Parser as _;
    let lexical = talkbank_model::ErrorCollector::new();
    talkbank_model::validation::report_control_characters(input, &lexical);
    if lexical.has_errors() {
        errors.report_all(lexical.into_vec());
        return ParseOutcome::Rejected;
    }
    let source = crate::source_text::SourceText::new(input);
    let tokens = lex_to_tokens(input, crate::lexer::COND_GRA_CONTENT);
    let parsed = dependent_tiers::gra_tier_parser(source, source.whole(), errors)
        .parse(tokens.as_slice())
        .into_result();
    match parsed {
        Ok(outcome) => outcome,
        Err(_) => {
            errors.report(ParseError::at_span(
                talkbank_model::ErrorCode::ParseFailed,
                Severity::Error,
                source.whole(),
                "Failed to parse nonempty GRA tier",
            ));
            ParseOutcome::Rejected
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_chat_file_streaming;
    use talkbank_model::ErrorCollector;

    #[test]
    fn fragment_helpers_refuse_embedded_end_of_input() {
        let reference = include_str!("../../../../corpus/reference/tiers/pho.cha");
        let signs = include_str!("../../../../corpus/reference/tiers/sin.cha");
        let body = |source: &'static str, prefix: &str| {
            source
                .lines()
                .find_map(|line| line.strip_prefix(prefix))
                .unwrap()
        };
        let mut accepted = Vec::new();
        for (name, input, parse) in [
            (
                "main tier",
                reference
                    .lines()
                    .find(|line| line.starts_with('*'))
                    .unwrap(),
                (|s| super::parse_main_tier(s).is_some()) as fn(&str) -> bool,
            ),
            (
                "located main tier",
                reference
                    .lines()
                    .find(|line| line.starts_with('*'))
                    .unwrap(),
                (|s| super::parse_main_tier_with_source(s).is_some()) as fn(&str) -> bool,
            ),
            (
                "pho",
                body(reference, "%pho:\t"),
                (|s| {
                    matches!(
                        super::parse_pho_tier(s),
                        talkbank_model::ParseOutcome::Parsed(_)
                    )
                }) as fn(&str) -> bool,
            ),
            (
                "sin",
                body(signs, "%sin:\t"),
                (|s| {
                    matches!(
                        super::parse_sin_tier(s),
                        talkbank_model::ParseOutcome::Parsed(_)
                    )
                }) as fn(&str) -> bool,
            ),
            (
                "id",
                body(reference, "@ID:\t"),
                (|s| super::parse_id_header(s).is_some()) as fn(&str) -> bool,
            ),
            (
                "word",
                body(reference, "*CHI:\t")
                    .split_whitespace()
                    .next()
                    .unwrap(),
                (|s| super::parse_word(s).is_some()) as fn(&str) -> bool,
            ),
        ] {
            assert!(parse(input), "legal reference {name}: {input:?}");
            for malformed in [format!("{input}\0"), format!("{input}\0{input}")] {
                if parse(&malformed) {
                    accepted.push((name, malformed));
                }
            }
        }
        assert!(
            accepted.is_empty(),
            "silently accepted hidden input: {accepted:?}"
        );
    }

    #[test]
    fn fragment_control_admission_uses_shared_chat_policy() {
        for input in [
            "",
            "\t\r\n",
            "\u{15}0_10\u{15}",
            "\u{2}\u{1}word\u{2}\u{2}",
            "café",
        ] {
            assert!(
                super::ControlCheckedFragment::admit(input).is_some(),
                "{input:?}"
            );
        }
        for input in ["\0", "\u{1}", "\u{2}", "\u{2}\u{3}", "\u{7}", "\u{7f}"] {
            assert!(
                super::ControlCheckedFragment::admit(input).is_none(),
                "{input:?}"
            );
        }
    }

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
