//! Dependent-tier prefix admission and tier-specific recovery.

use super::{report_error, skip_to_newline};
use crate::ast::*;
use crate::parser::{LexedSource, dependent_tiers};
use crate::token::{DependentBodyKind, Token};
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
) -> Vec<DependentTierEntryParsed<'a>> {
    let tokens = lexed.tokens();
    let mut dep_tiers = Vec::new();

    while *pos < tokens.len() {
        let (prefix, prefix_span) = lexed.token_at(*pos);
        // Classify the token before consuming anything from the next line.
        let (kind, separator) = match prefix {
            Token::TierPrefix(prefix) => (Some(prefix.kind()), prefix.separator()),
            Token::IncompleteTierPrefix(_) => (None, talkbank_model::model::TierSeparator::CLEAN),
            _ => break,
        };
        let prefix = prefix.clone();
        let prefix_text = prefix.text();
        let mut push = |tier| dep_tiers.push(DependentTierEntryParsed { tier, separator });
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
        let Some(kind) = kind else {
            let (_, last_span) = lexed.token_at(content_end - 1);
            errors.report(ParseError::new(
                talkbank_model::ErrorCode::MalformedTierHeader,
                talkbank_model::Severity::Error,
                talkbank_model::SourceLocation::from_offsets(prefix_span.start, last_span.end),
                None,
                "Dependent tier label must be followed by a colon and a tab",
            ));
            push(DependentTierParsed::Text {
                prefix,
                content: tier_tokens.to_vec(),
            });
            continue;
        };

        // The lexer selected the body grammar at exact-prefix admission.
        // A longer label cannot accidentally enter a shorter label's parser.
        match kind {
            DependentBodyKind::Mor | DependentBodyKind::Trn => {
                match dependent_tiers::mor_tier_parser()
                    .parse(tier_tokens)
                    .into_result()
                {
                    Ok(tier) => push(DependentTierParsed::Mor(tier)),
                    Err(_) => {
                        // Inspect original whitespace items, retaining each item's
                        // source span even when rich tokens expose only payloads.
                        if let Some(item) = lexed
                            .whitespace_items(content_start..content_end)
                            .find(|item| item.text().starts_with('|') && item.text().len() > 1)
                        {
                            let text = item.text();
                            errors.report(
                                ParseError::new(
                                    talkbank_model::errors::codes::ErrorCode::MorItemEmptyPos,
                                    talkbank_model::Severity::Error,
                                    talkbank_model::SourceLocation::new(item.span()),
                                    None,
                                    format!("MOR item '{text}' has an empty part-of-speech field"),
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
                        if kind == DependentBodyKind::Mor {
                            let (_, last_span) = lexed.token_at(content_end - 1);
                            push(DependentTierParsed::RejectedMor(RejectedMorTier::report(
                                prefix,
                                tier_tokens,
                                Span::from_usize(prefix_span.start, last_span.end),
                                errors,
                            )));
                        } else {
                            push(fallback_text_tier(prefix, tier_tokens));
                        }
                    }
                }
            }
            DependentBodyKind::Pho => {
                match dependent_tiers::pho_tier_parser()
                    .parse(tier_tokens)
                    .into_result()
                {
                    Ok(tier) => push(DependentTierParsed::Pho(tier)),
                    Err(_) => {
                        report_error(
                            errors,
                            talkbank_model::errors::codes::ErrorCode::UnparsableContent,
                            talkbank_model::Severity::Error,
                            tier_tokens,
                            &format!("failed to parse {prefix_text} tier content"),
                        );
                        push(fallback_text_tier(prefix, tier_tokens));
                    }
                }
            }
            DependentBodyKind::Mod => {
                match dependent_tiers::pho_tier_parser()
                    .parse(tier_tokens)
                    .into_result()
                {
                    Ok(tier) => push(DependentTierParsed::Mod(tier)),
                    Err(_) => {
                        report_error(
                            errors,
                            talkbank_model::errors::codes::ErrorCode::UnparsableContent,
                            talkbank_model::Severity::Error,
                            tier_tokens,
                            &format!("failed to parse {prefix_text} tier content"),
                        );
                        push(fallback_text_tier(prefix, tier_tokens));
                    }
                }
            }
            DependentBodyKind::Gra => {
                match dependent_tiers::gra_tier_parser()
                    .parse(tier_tokens)
                    .into_result()
                {
                    Ok(tier) => push(DependentTierParsed::Gra(tier)),
                    Err(_) => {
                        report_error(
                            errors,
                            talkbank_model::errors::codes::ErrorCode::UnparsableContent,
                            talkbank_model::Severity::Error,
                            tier_tokens,
                            &format!("failed to parse {prefix_text} tier content"),
                        );
                        push(fallback_text_tier(prefix, tier_tokens));
                    }
                }
            }
            DependentBodyKind::Sin => {
                match dependent_tiers::sin_tier_parser()
                    .parse(tier_tokens)
                    .into_result()
                {
                    Ok(tier) => push(DependentTierParsed::Sin(tier)),
                    Err(_) => {
                        report_error(
                            errors,
                            talkbank_model::errors::codes::ErrorCode::UnparsableContent,
                            talkbank_model::Severity::Error,
                            tier_tokens,
                            &format!("failed to parse {prefix_text} tier content"),
                        );
                        push(fallback_text_tier(prefix, tier_tokens));
                    }
                }
            }
            DependentBodyKind::Wor => {
                match dependent_tiers::wor_tier_parser()
                    .parse(tier_tokens)
                    .into_result()
                {
                    Ok(wor) => push(DependentTierParsed::Wor(wor)),
                    Err(_) => {
                        report_error(
                            errors,
                            talkbank_model::errors::codes::ErrorCode::UnparsableContent,
                            talkbank_model::Severity::Error,
                            tier_tokens,
                            &format!("failed to parse {prefix_text} tier content"),
                        );
                        push(fallback_text_tier(prefix, tier_tokens));
                    }
                }
            }
            DependentBodyKind::Text => {
                // Generic text tier, always succeeds
                let content: Vec<Token<'a>> = tier_tokens.to_vec();
                push(DependentTierParsed::Text { prefix, content });
            }
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
