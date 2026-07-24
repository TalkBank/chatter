//! File-level parser, assembles headers, utterances, and dependent tiers.
//!
//! This module is imperative (sequential line dispatch) rather than
//! combinator-based, because the file structure is prefix-dispatched:
//! dependent tier type is determined by reading the `%mor:`, `%gra:` etc.
//! prefix text, which doesn't map cleanly to chumsky's token-variant matching.
//!
//! Sub-parsers for individual tiers are chumsky combinators from
//! `dependent_tiers` and `main_tier`.
//!
//! **Error reporting:** When a chumsky sub-parser fails, this module
//! reports the failure to the `ErrorSink` and produces best-effort
//! output (e.g., falling back to a generic text tier for unparseable
//! dependent tiers).

use chumsky::Parser as _;

use crate::ast::*;
use crate::token::{Token, TokenDiscriminants};
use talkbank_model::{ErrorSink, NullErrorSink, ParseError, Span};

use super::dependent_tiers;
use super::main_tier;

/// Parse a complete CHAT file with no error reporting.
pub fn parse_file<'a>(tokens: &'a [Token<'a>], source: &'a str) -> ChatFile<'a> {
    parse_file_with_errors(tokens, source, &NullErrorSink)
}

/// Report E749 for every comma token immediately followed by a word
/// token with no whitespace/newline between (`hey ,you`; CLAN CHECK
/// 92). Mirrors the model-validation rule `check_comma_glued_to_next`
/// (talkbank-model `validation/utterance/comma.rs`), which cannot fire
/// on this parser's output because its separators carry dummy spans;
/// the lexer tokenizes whitespace, so adjacency IS the absence of a
/// whitespace token between the comma and the next word. Narrowed to
/// word-next exactly like the model rule (group/overlap/CA tokens after
/// a comma are exempt in CHECK 92).
fn report_comma_glued_to_next_word<'a>(tokens: &[Token<'a>], errors: &impl ErrorSink) {
    for pair in tokens.windows(2) {
        if matches!(pair[0], Token::Comma(_)) && matches!(pair[1], Token::Word { .. }) {
            errors.report(ParseError::new(
                talkbank_model::errors::codes::ErrorCode::CommaGluedToNextWord,
                talkbank_model::Severity::Error,
                talkbank_model::SourceLocation::new(Span::DUMMY),
                None,
                "Comma must be followed by a space or end-of-line".to_owned(),
            ));
        }
    }
}

/// Report E750 for whitespace hugging an angle-group delimiter: a
/// `LessThan` token immediately followed by whitespace, or whitespace
/// immediately followed by `GreaterThan` (`< dog>` / `<dog >`; CLAN
/// CHECK 160). Mirrors the tree-sitter parser's check in the group
/// parser (`main_tier/content/group/parser.rs`); token-level here
/// because the lexer tokenizes whitespace explicitly, so the pattern
/// is directly visible in the stream.
fn report_space_inside_angle_group<'a>(tokens: &[Token<'a>], errors: &impl ErrorSink) {
    for pair in tokens.windows(2) {
        let position =
            if matches!(pair[0], Token::LessThan(_)) && matches!(pair[1], Token::Whitespace(_)) {
                Some("after '<'")
            } else if matches!(pair[0], Token::Whitespace(_))
                && matches!(pair[1], Token::GreaterThan(_))
            {
                Some("before '>'")
            } else {
                None
            };
        if let Some(position) = position {
            errors.report(ParseError::new(
                talkbank_model::errors::codes::ErrorCode::SpaceInsideAngleGroup,
                talkbank_model::Severity::Error,
                talkbank_model::SourceLocation::new(Span::DUMMY),
                None,
                format!("Space is not allowed {position} in an angle-bracket group"),
            ));
        }
    }
}

/// Report E751 for every pause token immediately following a word token
/// with no whitespace between (`hello(.)`; CLAN CHECK 57). Mirrors the
/// model-validation rule `check_pause_glued_to_word` (talkbank-model
/// `validation/utterance/spacing.rs`), which cannot fire on this
/// parser's output because its pauses carry dummy spans.
fn report_pause_glued_to_word<'a>(tokens: &[Token<'a>], errors: &impl ErrorSink) {
    for pair in tokens.windows(2) {
        let is_pause = matches!(
            pair[1],
            Token::PauseShort(_)
                | Token::PauseMedium(_)
                | Token::PauseLong(_)
                | Token::PauseTimed(_)
        );
        if matches!(pair[0], Token::Word { .. }) && is_pause {
            errors.report(ParseError::new(
                talkbank_model::errors::codes::ErrorCode::PauseGluedToWord,
                talkbank_model::Severity::Error,
                talkbank_model::SourceLocation::new(Span::DUMMY),
                None,
                "Pause must be separated from the preceding word by a space".to_owned(),
            ));
        }
    }
}

/// Report E757 for every word token immediately following a retrace
/// marker token with no whitespace between (`hello [/]x`; CLAN CHECK
/// 19). Mirrors the model-validation rule
/// `check_code_glued_to_following_content` (talkbank-model
/// `validation/utterance/spacing.rs`), which cannot fire on this
/// parser's output because its retraces carry dummy spans.
fn report_code_glued_to_following_content<'a>(tokens: &[Token<'a>], errors: &impl ErrorSink) {
    for pair in tokens.windows(2) {
        let is_retrace = matches!(
            pair[0],
            Token::RetraceComplete(_)
                | Token::RetracePartial(_)
                | Token::RetraceMultiple(_)
                | Token::RetraceReformulation(_)
        );
        if is_retrace && matches!(pair[1], Token::Word { .. }) {
            errors.report(ParseError::new(
                talkbank_model::errors::codes::ErrorCode::CodeGluedToFollowingContent,
                talkbank_model::Severity::Error,
                talkbank_model::SourceLocation::new(Span::DUMMY),
                None,
                "Bracketed code must be separated from the following word by a space".to_owned(),
            ));
        }
    }
}

/// Report E758 for a main tier whose content starts with a space after
/// the `:\t` separator, in a file WITHOUT `@Options: CA` (CLAN CHECK
/// 123). Mirrors the model-validation rule
/// `check_leading_space_on_main_tier` (talkbank-model
/// `validate/checks.rs`), which cannot fire on this parser's output
/// because its speaker spans are dummies. The CA probe scans header
/// content tokens for the CA option before the tier scan.
/// Scope note: this token scan keys on literal whitespace after the
/// `:\t` separator, so it can fire on a line whose first ITEM is
/// span-less (e.g. `*CHI:<tab><space>+" ...`), where the model-side
/// check opts out; the model side is the conservative one by design.
fn report_leading_space_on_main_tier<'a>(tokens: &[Token<'a>], errors: &impl ErrorSink) {
    let file_is_ca = tokens.windows(2).any(|pair| {
        matches!(pair[0], Token::HeaderPrefix(prefix) if prefix.starts_with("@Options"))
            && matches!(pair[1], Token::HeaderContent(content)
                if content.split(',').any(|option| option.trim() == "CA"))
    });
    if file_is_ca {
        return;
    }
    for window in tokens.windows(4) {
        if matches!(window[0], Token::Star(_))
            && matches!(window[1], Token::Speaker(_))
            && matches!(window[2], Token::TierSep(_))
            && matches!(window[3], Token::Whitespace(_))
        {
            errors.report(ParseError::new(
                talkbank_model::errors::codes::ErrorCode::LeadingSpaceOnMainTier,
                talkbank_model::Severity::Error,
                talkbank_model::SourceLocation::new(Span::DUMMY),
                None,
                "Extra whitespace between the tab and tier content in a non-CA file".to_owned(),
            ));
        }
    }
}

/// Report E748 for every media-bullet timestamp written with a leading
/// zero before another digit (`012`); a bare `0` is legal. Mirrors the
/// tree-sitter parser's check in `media_bullet.rs` (CLAN CHECK 90,
/// spec `E748_leading_zero_bullet_time.md`). Token-level scan because
/// the raw digit text exists only here: the model stores `u64`
/// milliseconds, so the representation is invisible downstream. The
/// bullet still parses; the diagnostic alone makes the file invalid.
fn report_leading_zero_bullet_times<'a>(tokens: &[Token<'a>], errors: &impl ErrorSink) {
    for tok in tokens {
        if let Token::MediaBullet {
            start_time,
            end_time,
            ..
        } = tok
        {
            for (component, which) in [(start_time, "start"), (end_time, "end")] {
                if component.len() > 1 && component.starts_with('0') {
                    errors.report(ParseError::new(
                        talkbank_model::errors::codes::ErrorCode::LeadingZeroBulletTime,
                        talkbank_model::Severity::Error,
                        talkbank_model::SourceLocation::new(Span::DUMMY),
                        None,
                        format!(
                            "Bullet {which} time '{component}' has a leading zero; \
                             bullet times are plain millisecond integers"
                        ),
                    ));
                }
            }
        }
    }
}

/// Parse a complete CHAT file from a leaked token slice, reporting
/// parse failures to the given error sink.
pub fn parse_file_with_errors<'a>(
    tokens: &'a [Token<'a>],
    source: &'a str,
    errors: &impl ErrorSink,
) -> ChatFile<'a> {
    report_leading_zero_bullet_times(tokens, errors);
    report_comma_glued_to_next_word(tokens, errors);
    report_space_inside_angle_group(tokens, errors);
    report_pause_glued_to_word(tokens, errors);
    report_code_glued_to_following_content(tokens, errors);
    report_leading_space_on_main_tier(tokens, errors);
    let mut pos = 0;
    let mut lines = Vec::new();

    while pos < tokens.len() {
        let d = TokenDiscriminants::from(&tokens[pos]);
        match d {
            // No-content headers
            TokenDiscriminants::HeaderUtf8
            | TokenDiscriminants::HeaderBegin
            | TokenDiscriminants::HeaderEnd
            | TokenDiscriminants::HeaderBlank
            | TokenDiscriminants::HeaderNewEpisode => {
                let tok = tokens[pos].clone();
                pos += 1;
                if pos < tokens.len()
                    && TokenDiscriminants::from(&tokens[pos]) == TokenDiscriminants::Newline
                {
                    pos += 1;
                }
                lines.push(Line::Header(HeaderParsed {
                    prefix: tok,
                    content: vec![],
                }));
            }

            // Headers with content
            TokenDiscriminants::HeaderPrefix
            | TokenDiscriminants::HeaderBirthOf
            | TokenDiscriminants::HeaderBirthplaceOf
            | TokenDiscriminants::HeaderL1Of => {
                let prefix = tokens[pos].clone();
                pos += 1;
                let mut content = Vec::new();
                while pos < tokens.len()
                    && TokenDiscriminants::from(&tokens[pos]) != TokenDiscriminants::Newline
                {
                    let tok = tokens[pos].clone();
                    pos += 1;
                    if !matches!(tok, Token::Whitespace(_)) {
                        content.push(tok);
                    }
                }
                if pos < tokens.len()
                    && TokenDiscriminants::from(&tokens[pos]) == TokenDiscriminants::Newline
                {
                    pos += 1;
                }
                lines.push(Line::Header(HeaderParsed { prefix, content }));
            }

            // Main tier
            TokenDiscriminants::Star => {
                let start = pos;
                pos = skip_to_newline(tokens, pos);
                if pos < tokens.len() {
                    pos += 1; // consume newline
                }

                let main_tier_tokens = &tokens[start..pos];

                // Curly single quotes (U+2018/U+2019) are illegal word
                // characters (E256; CLAN CHECK 138/139). The lexer recognizes
                // each as an `IllegalCurlyQuote` token. Emit E256 for each and
                // strip them before parsing, so the surrounding words survive,
                // matching the tree-sitter parser which drops the recognized
                // `illegal_curly_quote` node and keeps the adjacent words. The
                // chumsky combinators have no `ErrorSink` access, so the
                // diagnostic is emitted here, mirroring the MISSING-token
                // recovery policy (see this crate's CLAUDE.md).
                //
                // The `.any()` precheck keeps the valid-file fast path to a
                // single no-allocation scan. Only when a curly quote is present
                // do we make one more pass that both reports each E256 and
                // builds the filtered stream. chumsky ties the input-slice
                // lifetime to the parsed output, so the filtered stream must
                // outlive 'a; we leak it the way this crate already leaks its
                // token storage (see `lex_to_tokens`). The leak is bounded to
                // invalid input, never the valid-file fast path.
                let has_curly_quote = main_tier_tokens
                    .iter()
                    .any(|t| matches!(t, Token::IllegalCurlyQuote(_)));
                let tier_input: &'a [Token<'a>] = if has_curly_quote {
                    let mut filtered: Vec<Token<'a>> = Vec::with_capacity(main_tier_tokens.len());
                    for tok in main_tier_tokens {
                        if let Token::IllegalCurlyQuote(s) = tok {
                            errors.report(
                                ParseError::new(
                                    talkbank_model::errors::codes::ErrorCode::IllegalCurlyQuote,
                                    talkbank_model::Severity::Error,
                                    talkbank_model::SourceLocation::new(Span::DUMMY),
                                    None,
                                    format!(
                                        "Curly single quotation mark ({s}) is not a legal word \
                                         character; CHAT requires the ASCII apostrophe (')"
                                    ),
                                )
                                .with_suggestion(
                                    "Replace the curly single quote with the ASCII apostrophe (')",
                                ),
                            );
                        } else {
                            filtered.push(tok.clone());
                        }
                    }
                    Box::leak(filtered.into_boxed_slice())
                } else {
                    main_tier_tokens
                };

                match main_tier::main_tier_parser()
                    .parse(tier_input)
                    .into_result()
                {
                    Ok(main_tier) => {
                        // Recovery is not validity: this front end parses a
                        // LEADING postfix annotation (retrace / overlap /
                        // replacement) as a standalone `Annotation` item, but
                        // those codes scope over PRECEDING material, so an
                        // utterance that BEGINS with one is malformed (CLAN
                        // CHECK 52, "Item '%s' must be preceded by text.").
                        // Report E759 here, mirroring the tree-sitter error
                        // analysis; the AST is kept (recovery policy).
                        report_annotation_at_utterance_start(&main_tier.tier_body.contents, errors);
                        // Recovery is not validity: a `<...>` group with no
                        // following annotation only parses via a synthesized
                        // retrace (Retrace::synthesized_missing_annotation). CLAN
                        // rejects it ("< > should be followed by [ ]"), so surface
                        // the matching MISSING diagnostic (E342) here, where the
                        // ErrorSink is available, mirroring the tree-sitter
                        // backstop. The AST (and SemanticEq) is unchanged.
                        if has_synthesized_missing_annotation(&main_tier.tier_body.contents) {
                            report_error(
                                errors,
                                talkbank_model::errors::codes::ErrorCode::MissingRequiredElement,
                                talkbank_model::Severity::Error,
                                main_tier_tokens,
                                "angle-bracket group must be followed by an annotation ([ ])",
                            );
                        }
                        let dep_tiers = parse_dependent_tiers(tokens, &mut pos, errors);
                        lines.push(Line::Utterance(Box::new(Utterance {
                            main_tier,
                            dependent_tiers: dep_tiers,
                        })));
                    }
                    Err(_) => {
                        // Report E321: unparsable utterance.
                        report_error(
                            errors,
                            talkbank_model::errors::codes::ErrorCode::UnparsableUtterance,
                            talkbank_model::Severity::Error,
                            main_tier_tokens,
                            "utterance could not be parsed",
                        );
                        // Skip any dependent tiers that follow; they're orphaned
                        // without a valid main tier.
                        while pos < tokens.len()
                            && TokenDiscriminants::from(&tokens[pos])
                                == TokenDiscriminants::TierPrefix
                        {
                            pos = skip_to_newline(tokens, pos);
                            if pos < tokens.len() {
                                pos += 1;
                            }
                        }
                    }
                }
            }

            // Skip structural tokens
            TokenDiscriminants::Whitespace
            | TokenDiscriminants::Newline
            | TokenDiscriminants::Continuation
            | TokenDiscriminants::BOM => {
                pos += 1;
            }

            // Orphan tier prefix (no preceding main tier), report E319
            TokenDiscriminants::TierPrefix => {
                let line_start = pos;
                pos = skip_to_newline(tokens, pos);
                if pos < tokens.len() {
                    pos += 1;
                }
                report_error(
                    errors,
                    talkbank_model::errors::codes::ErrorCode::UnparsableLine,
                    talkbank_model::Severity::Error,
                    &tokens[line_start..pos],
                    "orphan dependent tier (no preceding main tier)",
                );
            }

            // Unknown tokens, report and skip
            _ => {
                let tok = &tokens[pos];
                errors.report(ParseError::new(
                    talkbank_model::errors::codes::ErrorCode::UnexpectedSyntax,
                    talkbank_model::Severity::Error,
                    talkbank_model::SourceLocation::new(Span::DUMMY),
                    None,
                    format!("unhandled token in parse_chat_file: {:?}", tok.text()),
                ));
                pos += 1;
            }
        }
    }

    ChatFile { lines, source }
}

/// Parse dependent tiers following a main tier.
///
/// When a tier-specific chumsky parser fails, the error is reported
/// and the tier falls back to a generic text tier (preserving the raw
/// content for downstream inspection).
fn parse_dependent_tiers<'a>(
    tokens: &'a [Token<'a>],
    pos: &mut usize,
    errors: &impl ErrorSink,
) -> Vec<DependentTierParsed<'a>> {
    let mut dep_tiers = Vec::new();

    while *pos < tokens.len()
        && TokenDiscriminants::from(&tokens[*pos]) == TokenDiscriminants::TierPrefix
    {
        let prefix = tokens[*pos].clone();
        let prefix_text = prefix.text();
        *pos += 1;

        let content_start = *pos;
        *pos = skip_to_newline(tokens, *pos);
        let content_end = *pos;
        if *pos < tokens.len() {
            *pos += 1; // consume newline
        }

        let tier_tokens = &tokens[content_start..content_end];

        // Malformed tier: no content after prefix (e.g., `%mor\n` without `:\t`).
        // Report E602 and produce a fallback text tier.
        if tier_tokens.is_empty() {
            errors.report(ParseError::new(
                talkbank_model::errors::codes::ErrorCode::MalformedTierHeader,
                talkbank_model::Severity::Error,
                talkbank_model::SourceLocation::new(Span::DUMMY),
                None,
                format!(
                    "malformed dependent tier: {} has no content (missing colon-tab?)",
                    prefix_text
                ),
            ));
            dep_tiers.push(DependentTierParsed::Text {
                prefix,
                content: vec![],
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
                    dep_tiers.push(fallback_text_tier(prefix, tier_tokens));
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
                Ok((items, terminator)) => {
                    dep_tiers.push(DependentTierParsed::Wor { items, terminator })
                }
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

/// Whether any content item is a synthesized recovery from a `<...>` group that
/// lacked a following annotation, recursing into nested groups, quotations, and
/// retraces. Used to surface the MISSING-annotation diagnostic (E342) that the
/// chumsky combinators cannot emit themselves (no ErrorSink access).
/// E759: the utterance's FIRST content item is a postfix annotation
/// (retrace, overlap marker, or replacement), which has no preceding
/// material to scope over. Trigger set mirrors CLAN CHECK error 52 and
/// the tree-sitter error analysis; ordinary leading items (words,
/// events, groups, precodes) never match. The `["]` quotation-marker
/// case of CHECK 52 has no token variant in this front end and is
/// covered by the tree-sitter side only.
fn report_annotation_at_utterance_start(items: &[ContentItem<'_>], errors: &impl ErrorSink) {
    let offending = match items.first() {
        Some(ContentItem::Annotation(token)) => match token {
            Token::RetraceComplete(s)
            | Token::RetracePartial(s)
            | Token::RetraceMultiple(s)
            | Token::RetraceReformulation(s)
            | Token::OverlapPrecedes(s)
            | Token::OverlapFollows(s)
            | Token::Replacement(s) => Some(*s),
            _ => None,
        },
        _ => None,
    };
    if let Some(code_text) = offending {
        errors.report(
            ParseError::new(
                talkbank_model::errors::codes::ErrorCode::AnnotationAtUtteranceStart,
                talkbank_model::Severity::Error,
                talkbank_model::SourceLocation::new(Span::DUMMY),
                None,
                format!("Annotation '{code_text}' at utterance start has no content to attach to"),
            )
            .with_suggestion(
                "Retraces, overlap markers, replacements, and quotation codes scope over the \
                 material BEFORE them; put the annotated content first, or remove the code",
            ),
        );
    }
}

fn has_synthesized_missing_annotation(items: &[ContentItem<'_>]) -> bool {
    items.iter().any(|item| match item {
        ContentItem::Retrace(r) => {
            r.synthesized_missing_annotation || has_synthesized_missing_annotation(&r.content)
        }
        ContentItem::Group(g) => has_synthesized_missing_annotation(&g.contents),
        ContentItem::Quotation(q) => has_synthesized_missing_annotation(&q.contents),
        _ => false,
    })
}

/// Report a parse error with a specific error code.
fn report_error(
    errors: &impl ErrorSink,
    code: talkbank_model::errors::codes::ErrorCode,
    severity: talkbank_model::Severity,
    tokens: &[Token<'_>],
    context: &str,
) {
    let preview: String = tokens
        .iter()
        .take(5)
        .map(|t| t.text())
        .collect::<Vec<_>>()
        .join(" ");
    errors.report(ParseError::new(
        code,
        severity,
        talkbank_model::SourceLocation::new(Span::DUMMY),
        None,
        format!("{context}: {preview}..."),
    ));
}

/// Create a fallback text tier from raw tokens when a tier-specific
/// parser fails. This preserves the content for downstream inspection
/// rather than silently dropping it.
fn fallback_text_tier<'a>(prefix: Token<'a>, tokens: &[Token<'a>]) -> DependentTierParsed<'a> {
    let content: Vec<Token<'a>> = tokens.to_vec();
    DependentTierParsed::Text { prefix, content }
}

/// Advance position to the Newline token (or end of tokens).
fn skip_to_newline(tokens: &[Token<'_>], mut pos: usize) -> usize {
    while pos < tokens.len()
        && TokenDiscriminants::from(&tokens[pos]) != TokenDiscriminants::Newline
    {
        pos += 1;
    }
    pos
}
