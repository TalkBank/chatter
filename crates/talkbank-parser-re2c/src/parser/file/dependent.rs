//! Dependent-tier prefix admission and tier-specific recovery.

use crate::ast::*;
use crate::parser::{TokenCursor, dependent_tiers};
use crate::token::{DependentBodyKind, DependentPrefixToken, Token};
use chumsky::Parser as _;
use talkbank_model::{ErrorSink, ParseError};

/// Only the complete state can select a body grammar. An incomplete prefix
/// retains its original text until it is recorded as rejected source.
enum DependentPrefix<'a> {
    Complete(DependentPrefixToken<'a>),
    Incomplete(&'a str),
}

/// Parse dependent tiers following a main tier.
///
/// When a tier-specific chumsky parser fails, the error is reported
/// and raw content is retained for inspection. Rejected morphology has its
/// own recovery state, distinct from a valid generic text tier.
pub(in crate::parser) fn parse_dependent_tiers<'a>(
    cursor: &mut TokenCursor<'_, 'a>,
    errors: &impl ErrorSink,
) -> Vec<DependentTierEntryParsed<'a>> {
    let lexed = cursor.owner();
    let mut dep_tiers = Vec::new();

    while let Some(current) = cursor.peek() {
        let line_start = current.index();
        let prefix = current.token();
        let prefix_span = current.span();
        // Classify the token before consuming anything from the next line.
        #[allow(
            clippy::wildcard_enum_match_arm,
            reason = "a non-dependent prefix belongs to the next line and must remain unconsumed"
        )]
        let prefix = match prefix {
            Token::TierPrefix(prefix) => DependentPrefix::Complete(prefix.clone()),
            Token::IncompleteTierPrefix(text) => DependentPrefix::Incomplete(text),
            _ => break,
        };
        cursor.advance();

        let content_start = cursor.position();
        cursor.skip_to_newline();
        let content_end = cursor.position();
        let Ok(line_content) = lexed.admit_region(line_start..content_end, errors) else {
            break;
        };
        let _ = cursor.take_newline();
        // The tier's extent: its prefix through its line ending, as the
        // tree-sitter grammar and this backend's headers measure a line.
        let Ok(line) = lexed.admit_region(line_start..cursor.position(), errors) else {
            break;
        };
        let extent = line.extent();

        let prefix = match prefix {
            DependentPrefix::Complete(prefix) => prefix,
            DependentPrefix::Incomplete(text) => {
                line_content.report_error(
                    talkbank_model::ErrorCode::MalformedTierHeader,
                    "Dependent tier label must be followed by a colon and a tab",
                    errors,
                );
                let Ok(label) = TextTierLabel::from_prefix_text(text) else {
                    line_content.report_error(
                        talkbank_model::ErrorCode::InternalError,
                        "native incomplete dependent prefix has no label",
                        errors,
                    );
                    continue;
                };
                dep_tiers.push(DependentTierEntryParsed::from_lexed(
                    DependentTierParsed::RejectedText { label },
                    talkbank_model::model::TierSeparator::CLEAN,
                    None,
                    line.source_slice(),
                ));
                continue;
            }
        };
        let separator = prefix.separator();
        let prefix_text = prefix.text();
        let content_range = match line.dependent_content_span() {
            Ok(body) => body,
            Err(fault) => {
                errors.report(ParseError::new(
                    talkbank_model::ErrorCode::InternalError,
                    talkbank_model::Severity::Error,
                    talkbank_model::DiagnosticLocation::from_offsets(
                        prefix_span.start,
                        prefix_span.end,
                    ),
                    None,
                    fault.to_string(),
                ));
                None
            }
        };
        let mut push = |tier| {
            dep_tiers.push(DependentTierEntryParsed::from_lexed(
                tier,
                separator,
                content_range,
                line.source_slice(),
            ))
        };

        let Ok(selected) = lexed.admit_tokens(content_start..content_end, errors) else {
            break;
        };
        let tier_tokens = selected.tokens();

        // Only complete text-backed tiers reach payload admission. Incomplete
        // prefixes above retain rejected source without interpreting its body.
        let admit_text_label = || {
            TextTierLabel::from_prefix_text(prefix_text).map_err(|error| {
                line_content.report_error(
                    talkbank_model::ErrorCode::InternalError,
                    &format!("native dependent prefix has no label: {error}"),
                    errors,
                );
            })
        };

        // The lexer selected the body grammar at exact-prefix admission.
        // A longer label cannot accidentally enter a shorter label's parser.
        match prefix.kind() {
            DependentBodyKind::Mor => {
                match dependent_tiers::mor_tier_parser(
                    lexed.source_text(),
                    talkbank_model::Location::Located(extent),
                )
                .parse(tier_tokens)
                .into_result()
                {
                    Ok(tier) => push(DependentTierParsed::Mor(tier)),
                    Err(_) => {
                        // Inspect original whitespace items, retaining each item's
                        // source span even when rich tokens expose only payloads.
                        let Ok(mut items) =
                            lexed.whitespace_items(content_start..content_end, errors)
                        else {
                            break;
                        };
                        if let Some(item) =
                            items.find(|item| item.text().starts_with('|') && item.text().len() > 1)
                        {
                            let text = item.text();
                            errors.report(
                                ParseError::new(
                                    talkbank_model::errors::codes::ErrorCode::MorItemEmptyPos,
                                    talkbank_model::Severity::Error,
                                    talkbank_model::DiagnosticLocation::new(item.span()),
                                    None,
                                    format!("MOR item '{text}' has an empty part-of-speech field"),
                                )
                                .with_suggestion(
                                    "Every %mor item is pos|stem with a non-empty part of speech \
                                 before the pipe (e.g., pro|we, v|go)",
                                ),
                            );
                        } else {
                            line_content.report_error(
                                talkbank_model::ErrorCode::UnparsableContent,
                                &format!("failed to parse {prefix_text} tier content"),
                                errors,
                            );
                        }
                        push(DependentTierParsed::RejectedMor(
                            talkbank_model::UnparsedTier::new(extent, line.source_slice().text()),
                        ));
                    }
                }
            }
            DependentBodyKind::Pho => {
                match dependent_tiers::pho_tier_parser(extent, talkbank_model::PhoTierType::Pho)
                    .parse(tier_tokens)
                    .into_result()
                {
                    Ok(tier) => push(DependentTierParsed::Pho(tier)),
                    Err(_) => {
                        line_content.report_error(
                            talkbank_model::ErrorCode::UnparsableContent,
                            &format!("failed to parse {prefix_text} tier content"),
                            errors,
                        );
                        push(DependentTierParsed::RejectedPho(
                            talkbank_model::UnparsedTier::new(extent, line.source_slice().text()),
                        ));
                    }
                }
            }
            DependentBodyKind::Mod => {
                match dependent_tiers::pho_tier_parser(extent, talkbank_model::PhoTierType::Mod)
                    .parse(tier_tokens)
                    .into_result()
                {
                    Ok(tier) => push(DependentTierParsed::Mod(tier)),
                    Err(_) => {
                        line_content.report_error(
                            talkbank_model::ErrorCode::UnparsableContent,
                            &format!("failed to parse {prefix_text} tier content"),
                            errors,
                        );
                        push(DependentTierParsed::RejectedMod(
                            talkbank_model::UnparsedTier::new(extent, line.source_slice().text()),
                        ));
                    }
                }
            }
            DependentBodyKind::Gra => {
                match dependent_tiers::gra_tier_parser(lexed.source_text(), extent, errors)
                    .parse(tier_tokens)
                    .into_result()
                {
                    Ok(talkbank_model::ParseOutcome::Parsed(tier)) => {
                        push(DependentTierParsed::Gra(tier))
                    }
                    Ok(talkbank_model::ParseOutcome::Rejected) => {
                        push(DependentTierParsed::RejectedGra(
                            talkbank_model::UnparsedTier::new(extent, line.source_slice().text()),
                        ));
                    }
                    Err(_) => {
                        line_content.report_error(
                            talkbank_model::ErrorCode::UnparsableContent,
                            &format!("failed to parse {prefix_text} tier content"),
                            errors,
                        );
                        push(DependentTierParsed::RejectedGra(
                            talkbank_model::UnparsedTier::new(extent, line.source_slice().text()),
                        ));
                    }
                }
            }
            DependentBodyKind::Sin => {
                match dependent_tiers::sin_tier_parser(extent)
                    .parse(tier_tokens)
                    .into_result()
                {
                    Ok(tier) => push(DependentTierParsed::Sin(tier)),
                    Err(_) => {
                        line_content.report_error(
                            talkbank_model::ErrorCode::UnparsableContent,
                            &format!("failed to parse {prefix_text} tier content"),
                            errors,
                        );
                        push(DependentTierParsed::RejectedSin(
                            talkbank_model::UnparsedTier::new(extent, line.source_slice().text()),
                        ));
                    }
                }
            }
            DependentBodyKind::Wor => {
                match dependent_tiers::wor_tier_parser(lexed.source_text(), extent)
                    .parse(tier_tokens)
                    .into_result()
                {
                    Ok(wor) => push(DependentTierParsed::Wor(wor)),
                    Err(_) => {
                        line_content.report_error(
                            talkbank_model::ErrorCode::UnparsableContent,
                            &format!("failed to parse {prefix_text} tier content"),
                            errors,
                        );
                        push(DependentTierParsed::RejectedWor(
                            talkbank_model::UnparsedTier::new(extent, line.source_slice().text()),
                        ));
                    }
                }
            }
            DependentBodyKind::Text => {
                let Ok(label) = admit_text_label() else {
                    continue;
                };
                let content = dependent_tiers::DiagnosedTextTierTokens::admit(selected, errors);
                push(match content {
                    talkbank_model::ParseOutcome::Parsed(content) => DependentTierParsed::Text {
                        label,
                        content: content.into_tokens(),
                    },
                    talkbank_model::ParseOutcome::Rejected => {
                        DependentTierParsed::RejectedText { label }
                    }
                });
            }
        }
    }

    dep_tiers
}
