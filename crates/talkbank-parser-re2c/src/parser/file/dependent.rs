//! Dependent-tier prefix admission and tier-specific recovery.

use super::{report_error, skip_to_newline};
use crate::ast::*;
use crate::parser::{LexedSource, dependent_tiers};
use crate::token::{Token, TokenDiscriminants};
use chumsky::Parser as _;
use talkbank_model::{ErrorSink, ParseError, Span};

/// Parse dependent tiers following a main tier.
///
/// When a tier-specific chumsky parser fails, the error is reported
/// and raw content is retained for inspection. Rejected morphology has its
/// own recovery state, distinct from a valid generic text tier.
pub(super) fn parse_dependent_tiers<'a>(
    lexed: &LexedSource<'a>,
    pos: &mut usize,
    errors: &impl ErrorSink,
) -> Vec<DependentTierParsed<'a>> {
    let tokens = lexed.tokens();
    let mut dep_tiers = Vec::new();

    while *pos < tokens.len()
        && matches!(
            TokenDiscriminants::from(&tokens[*pos]),
            TokenDiscriminants::TierPrefix | TokenDiscriminants::IncompleteTierPrefix
        )
    {
        let (prefix, prefix_span) = lexed.token_at(*pos);
        let prefix = prefix.clone();
        let prefix_text = prefix.text();
        *pos += 1;

        let content_start = *pos;
        *pos = skip_to_newline(tokens, *pos);
        let content_end = *pos;
        if *pos < tokens.len() {
            *pos += 1; // consume newline
        }

        let tier_tokens = &tokens[content_start..content_end];

        // The lexer already determined whether the separator matched. Missing
        // content and a missing separator are independent facts.
        if matches!(prefix, Token::IncompleteTierPrefix(_)) {
            let (_, last_span) = lexed.token_at(content_end - 1);
            errors.report(ParseError::new(
                talkbank_model::ErrorCode::MalformedTierHeader,
                talkbank_model::Severity::Error,
                talkbank_model::SourceLocation::from_offsets(prefix_span.start, last_span.end),
                None,
                "Dependent tier label must be followed by a colon and a tab",
            ));
            dep_tiers.push(DependentTierParsed::Text {
                prefix,
                content: tier_tokens.to_vec(),
            });
            continue;
        }

        // Try the tier-specific parser. On failure, report error and
        // fall back to generic text tier.
        if prefix_text.starts_with("%mor") || prefix_text.starts_with("%trn") {
            match dependent_tiers::mor_tier_parser()
                .parse(tier_tokens)
                .into_result()
            {
                Ok(tier) => dep_tiers.push(DependentTierParsed::Mor(tier)),
                Err(_) => {
                    // E760: a mor item whose part-of-speech field is empty
                    // (an item beginning with the `|` separator, `|we`).
                    // More specific than the generic unparsable fallback;
                    // mirrors the tree-sitter dependent-tier error analysis
                    // (modern reading of CLAN CHECK error 11). On a mor
                    // lex/parse failure the token stream degrades toward
                    // character-level tokens, so the tier text is
                    // reconstructed by concatenation (tokens carry their
                    // exact source slices, including whitespace) and the
                    // item rule is applied to the whitespace-split items,
                    // identically to the tree-sitter side.
                    let tier_text: String = tier_tokens.iter().map(Token::text).collect();
                    // Tier text reconstruction starts at the tier's content
                    // boundary, so the first whitespace item is a genuine
                    // item (no split-tail hazard as in the tree-sitter
                    // fragment case); items whose leading pipe follows a
                    // non-space character inside the SAME whitespace token
                    // (two-pipe/compound malformations) do not match the
                    // starts_with test at all.
                    if let Some(item) = tier_text
                        .split_whitespace()
                        .find(|text| text.starts_with('|') && text.len() > 1)
                    {
                        errors.report(
                            ParseError::new(
                                talkbank_model::errors::codes::ErrorCode::MorItemEmptyPos,
                                talkbank_model::Severity::Error,
                                talkbank_model::SourceLocation::new(Span::DUMMY),
                                None,
                                format!("MOR item '{item}' has an empty part-of-speech field"),
                            )
                            .with_suggestion(
                                "Every %mor item is pos|stem with a non-empty part of speech \
                                 before the pipe (e.g., pro|we, v|go)",
                            ),
                        );
                    } else {
                        report_error(
                            errors,
                            talkbank_model::errors::codes::ErrorCode::UnparsableContent,
                            talkbank_model::Severity::Error,
                            tier_tokens,
                            &format!("failed to parse {prefix_text} tier content"),
                        );
                    }
                    if prefix_text == "%mor:\t" {
                        let (_, last_span) = lexed.token_at(content_end - 1);
                        dep_tiers.push(DependentTierParsed::RejectedMor(RejectedMorTier::report(
                            prefix,
                            tier_tokens,
                            Span::from_usize(prefix_span.start, last_span.end),
                            errors,
                        )));
                    } else {
                        dep_tiers.push(fallback_text_tier(prefix, tier_tokens));
                    }
                }
            }
        } else if prefix_text.starts_with("%pho") {
            match dependent_tiers::pho_tier_parser()
                .parse(tier_tokens)
                .into_result()
            {
                Ok(tier) => dep_tiers.push(DependentTierParsed::Pho(tier)),
                Err(_) => {
                    report_error(
                        errors,
                        talkbank_model::errors::codes::ErrorCode::UnparsableContent,
                        talkbank_model::Severity::Error,
                        tier_tokens,
                        &format!("failed to parse {prefix_text} tier content"),
                    );
                    dep_tiers.push(fallback_text_tier(prefix, tier_tokens));
                }
            }
        } else if prefix_text.starts_with("%mod") {
            match dependent_tiers::pho_tier_parser()
                .parse(tier_tokens)
                .into_result()
            {
                Ok(tier) => dep_tiers.push(DependentTierParsed::Mod(tier)),
                Err(_) => {
                    report_error(
                        errors,
                        talkbank_model::errors::codes::ErrorCode::UnparsableContent,
                        talkbank_model::Severity::Error,
                        tier_tokens,
                        &format!("failed to parse {prefix_text} tier content"),
                    );
                    dep_tiers.push(fallback_text_tier(prefix, tier_tokens));
                }
            }
        } else if prefix_text.starts_with("%gra") {
            match dependent_tiers::gra_tier_parser()
                .parse(tier_tokens)
                .into_result()
            {
                Ok(tier) => dep_tiers.push(DependentTierParsed::Gra(tier)),
                Err(_) => {
                    report_error(
                        errors,
                        talkbank_model::errors::codes::ErrorCode::UnparsableContent,
                        talkbank_model::Severity::Error,
                        tier_tokens,
                        &format!("failed to parse {prefix_text} tier content"),
                    );
                    dep_tiers.push(fallback_text_tier(prefix, tier_tokens));
                }
            }
        } else if prefix_text.starts_with("%sin") {
            match dependent_tiers::sin_tier_parser()
                .parse(tier_tokens)
                .into_result()
            {
                Ok(tier) => dep_tiers.push(DependentTierParsed::Sin(tier)),
                Err(_) => {
                    report_error(
                        errors,
                        talkbank_model::errors::codes::ErrorCode::UnparsableContent,
                        talkbank_model::Severity::Error,
                        tier_tokens,
                        &format!("failed to parse {prefix_text} tier content"),
                    );
                    dep_tiers.push(fallback_text_tier(prefix, tier_tokens));
                }
            }
        } else if prefix_text.starts_with("%wor") {
            match dependent_tiers::wor_tier_parser()
                .parse(tier_tokens)
                .into_result()
            {
                Ok(wor) => dep_tiers.push(DependentTierParsed::Wor(wor)),
                Err(_) => {
                    report_error(
                        errors,
                        talkbank_model::errors::codes::ErrorCode::UnparsableContent,
                        talkbank_model::Severity::Error,
                        tier_tokens,
                        &format!("failed to parse {prefix_text} tier content"),
                    );
                    dep_tiers.push(fallback_text_tier(prefix, tier_tokens));
                }
            }
        } else {
            // Generic text tier, always succeeds
            let content: Vec<Token<'a>> = tier_tokens.to_vec();
            dep_tiers.push(DependentTierParsed::Text { prefix, content });
        }
    }

    dep_tiers
}

/// Create a fallback text tier from raw tokens when a tier-specific
/// parser fails. This preserves the content for downstream inspection
/// rather than silently dropping it.
fn fallback_text_tier<'a>(prefix: Token<'a>, tokens: &[Token<'a>]) -> DependentTierParsed<'a> {
    let content: Vec<Token<'a>> = tokens.to_vec();
    DependentTierParsed::Text { prefix, content }
}
