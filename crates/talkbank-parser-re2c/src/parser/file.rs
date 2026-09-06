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
use talkbank_model::{ErrorSink, ParseError, Span};

mod dependent;
use super::main_tier;
use dependent::parse_dependent_tiers;

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

/// Report E765 for a PAUSE immediately followed by content (`(.)and`).
///
/// The model rule `check_separator_glued_to_following_content` covers this
/// case in principle: its `free_standing_end` returns `Some(pause.span.end)`.
/// It cannot fire on this backend's output, because pause spans are still
/// `Span::DUMMY` and the rule skips `end == Span::DUMMY.end`.
///
/// SEPARATORS are deliberately NOT handled here any more. They carry real
/// spans since 2026-08-27, so the model rule reaches them, and this scan
/// reported E765 a SECOND time for every one it caught: `dog :and .` gave two.
/// A mirror is only correct while its model rule is genuinely unreachable, and
/// `re2c_reports_no_diagnostic_twice` is what notices when that stops being
/// true. Delete this the moment pauses carry spans.
fn report_pause_glued_to_following_content<'a>(tokens: &[Token<'a>], errors: &impl ErrorSink) {
    let is_pause = |t: &Token<'a>| {
        matches!(
            t,
            Token::PauseShort(_)
                | Token::PauseMedium(_)
                | Token::PauseLong(_)
                | Token::PauseTimed(_)
        )
    };
    for pair in tokens.windows(2) {
        if is_pause(&pair[0]) && (matches!(pair[1], Token::Word { .. }) || is_pause(&pair[1])) {
            errors.report(ParseError::new(
                talkbank_model::errors::codes::ErrorCode::SeparatorGluedToFollowingContent,
                talkbank_model::Severity::Error,
                talkbank_model::SourceLocation::new(Span::DUMMY),
                None,
                "Separator must be separated from the following content by a space".to_owned(),
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

/// Report missing separators at source-located annotation boundaries.
/// Replacements require whitespace before their opening bracket (E375/E316);
/// rich scoped annotations and retraces require it before a following word
/// (E757). Lexer locations stay paired with the admitted token categories.
fn report_annotation_spacing(lexed: &super::LexedSource<'_>, errors: &impl ErrorSink) {
    let range = 0..lexed.tokens().len();
    for ((left, _), (right, span)) in lexed
        .located(range.clone())
        .zip(lexed.located(range).skip(1))
    {
        // A replacement is a separate token after a word, including a word
        // assembled from sub-tokens. Recovery retains the AST, but the missing
        // separator violates word_with_optional_annotations (CHECK 161).
        if matches!(right, Token::Replacement(_))
            && (matches!(left, Token::Word { .. })
                || super::classify::is_word_token(TokenDiscriminants::from(left)))
        {
            errors.report(ParseError::new(
                talkbank_model::ErrorCode::ContentAnnotationParseError,
                talkbank_model::Severity::Error,
                talkbank_model::SourceLocation::from_offsets(span.end - 1, span.end),
                None,
                "Replacement annotation must be separated from its word by whitespace",
            ));
            // The canonical grammar also reports the opening bracket as
            // unparsable content. These endpoints come from the complete
            // replacement match, not a search through reconstructed text.
            errors.report(ParseError::new(
                talkbank_model::ErrorCode::UnparsableContent,
                talkbank_model::Severity::Error,
                talkbank_model::SourceLocation::from_offsets(span.start, span.start + 1),
                None,
                "Replacement begins without the required preceding whitespace",
            ));
        }
        let closes_a_code = matches!(left, Token::RightBracket(_))
            || matches!(
                super::classify::token_to_parsed_annotation(left.clone()),
                Some(ParsedAnnotation::Scoped(_) | ParsedAnnotation::Retrace(_))
            );
        if closes_a_code && matches!(right, Token::Word { .. }) {
            errors.report(ParseError::new(
                talkbank_model::ErrorCode::CodeGluedToFollowingContent,
                talkbank_model::Severity::Error,
                talkbank_model::SourceLocation::from_offsets(span.start, span.end),
                None,
                "Bracketed code must be separated from the following word by a space",
            ));
        }
    }
}

/// What one pre-parse pass over a main tier line's token slice (`*`
/// through newline) found that must be reported-and-stripped before the
/// combinator parse.
struct LineScan {
    /// Any `IllegalCurlyQuote` token on the line (E256; detected
    /// line-wide, including after the terminator).
    has_curly_quote: bool,
    /// Ascending indices of misplaced linker tokens (E766). A linker is
    /// legal only in the initial run after `*SPK:\t` (before the optional
    /// langcode and the content), so any linker after the first
    /// non-linker item is misplaced. Detection stops at the terminator: a
    /// linker AFTER the terminator does not parse as a content item on
    /// the tree-sitter side either, so it stays on the generic-failure
    /// path in both parsers (parity).
    misplaced_linkers: Vec<usize>,
}

/// Single pre-parse scan for [`LineScan`]: one pass finds both the curly
/// quotes and the misplaced linkers, so the valid-line fast path pays one
/// token walk instead of two. The `Vec` is heap-free until first push, so
/// clean lines allocate nothing.
fn scan_main_tier_line(main_tier_tokens: &[Token<'_>]) -> LineScan {
    use super::classify::{is_linker, is_terminator};

    /// Where the linker-phase half of the scan is within the line.
    enum LinePhase {
        /// Still in the initial linker run after `*SPK:\t`.
        InitialRun,
        /// After the first non-linker item; a linker here is misplaced.
        AfterContent,
        /// After the terminator; linkers here stay on the generic path.
        PastTerminator,
    }

    let mut phase = LinePhase::InitialRun;
    let mut scan = LineScan {
        has_curly_quote: false,
        misplaced_linkers: Vec::new(),
    };
    for (idx, tok) in main_tier_tokens.iter().enumerate() {
        let d = TokenDiscriminants::from(tok);
        match d {
            TokenDiscriminants::Star
            | TokenDiscriminants::Speaker
            | TokenDiscriminants::TierSep
            | TokenDiscriminants::Whitespace
            | TokenDiscriminants::Newline => {}
            TokenDiscriminants::IllegalCurlyQuote => {
                scan.has_curly_quote = true;
                // The quote is content for the linker phase too: `’ +"`
                // has content before the linker.
                if matches!(phase, LinePhase::InitialRun) {
                    phase = LinePhase::AfterContent;
                }
            }
            _ if is_linker(Some(d)) => {
                if matches!(phase, LinePhase::AfterContent) {
                    scan.misplaced_linkers.push(idx);
                }
            }
            _ if is_terminator(Some(d)) => phase = LinePhase::PastTerminator,
            _ => {
                if matches!(phase, LinePhase::InitialRun) {
                    phase = LinePhase::AfterContent;
                }
            }
        }
    }
    scan
}

/// Report E748 for every media-bullet timestamp written with a leading
/// zero before another digit (`012`); a bare `0` is legal. Mirrors the
/// tree-sitter parser's check in `media_bullet.rs` (CLAN CHECK 90,
/// spec `E748.md`). Token-level scan because
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

/// Parse a complete CHAT file from a temporary token slice, reporting
/// parse failures to the given error sink.
pub(crate) fn parse_file_with_errors<'a>(
    lexed: &super::LexedSource<'a>,
    errors: &impl ErrorSink,
) -> ChatFile<'a> {
    let tokens = lexed.tokens();
    let source = lexed.source();
    report_leading_zero_bullet_times(tokens, errors);
    report_space_inside_angle_group(tokens, errors);
    report_pause_glued_to_word(tokens, errors);
    report_pause_glued_to_following_content(tokens, errors);
    report_annotation_spacing(lexed, errors);
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
                lines.push(Line::Header {
                    header: HeaderParsed::Other {
                        prefix: tok,
                        content: vec![],
                    },
                    separator: talkbank_model::model::TierSeparator::CLEAN,
                });
            }

            // Headers with content
            TokenDiscriminants::HeaderPrefix
            | TokenDiscriminants::HeaderBirthOf
            | TokenDiscriminants::HeaderBirthplaceOf
            | TokenDiscriminants::HeaderL1Of => {
                let prefix = tokens[pos].clone();
                pos += 1;
                let separator = match &prefix {
                    Token::HeaderPrefix(prefix)
                    | Token::HeaderBirthOf(prefix)
                    | Token::HeaderBirthplaceOf(prefix)
                    | Token::HeaderL1Of(prefix) => prefix.separator(),
                    _ => talkbank_model::model::TierSeparator::CLEAN,
                };
                let separator = if let Some(Token::HeaderSep(sep)) = tokens.get(pos) {
                    pos += 1;
                    sep.separator()
                } else {
                    separator
                };
                let content_start = pos;
                while pos < tokens.len()
                    && TokenDiscriminants::from(&tokens[pos]) != TokenDiscriminants::Newline
                {
                    pos += 1;
                }
                let header = if matches!(&prefix, Token::HeaderPrefix(p) if p.text().contains("@Participants"))
                {
                    HeaderParsed::Participants(super::headers::parse_participants_tokens(
                        lexed.located(content_start..pos),
                        errors,
                    ))
                } else {
                    // Keep whitespace for consumers such as @Media, whose
                    // E767 diagnostic depends on spaces before the comma.
                    HeaderParsed::Other {
                        prefix,
                        content: tokens[content_start..pos].to_vec(),
                    }
                };
                if pos < tokens.len()
                    && TokenDiscriminants::from(&tokens[pos]) == TokenDiscriminants::Newline
                {
                    pos += 1;
                }
                lines.push(Line::Header { header, separator });
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
                // Misplaced linkers (E766, linker after content) get the same
                // treatment: report each by name and strip it, so the rest of
                // the line parses and no generic E321 co-fires, mirroring the
                // tree-sitter side, where the grammar parses the misplaced
                // linker as a content item and the lowering rejects just that
                // item. One `scan_main_tier_line` pass finds both, keeping the
                // valid-line fast path to a single no-allocation scan. Only
                // when something must be stripped do we make one more pass
                // that both reports and builds the filtered stream. Token
                // storage is independent of source lifetime, so this recovery
                // buffer is dropped after parsing while the AST borrows source.
                let scan = scan_main_tier_line(main_tier_tokens);
                let tier_input: std::borrow::Cow<'_, [Token<'a>]> = if scan.has_curly_quote
                    || !scan.misplaced_linkers.is_empty()
                {
                    // The indices come out of the scan in ascending order and
                    // `idx` ascends too, so a forward cursor replaces a
                    // per-token membership test.
                    let mut next_misplaced = scan.misplaced_linkers.iter().copied().peekable();
                    let mut filtered: Vec<Token<'a>> = Vec::with_capacity(main_tier_tokens.len());
                    for (idx, tok) in main_tier_tokens.iter().enumerate() {
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
                        } else if next_misplaced.next_if_eq(&idx).is_some() {
                            errors.report(
                                ParseError::new(
                                    talkbank_model::errors::codes::ErrorCode::LinkerNotUtteranceInitial,
                                    talkbank_model::Severity::Error,
                                    talkbank_model::SourceLocation::new(Span::DUMMY),
                                    None,
                                    "Linker must be utterance-initial; it links this utterance \
                                     to the previous one and cannot follow content"
                                        .to_owned(),
                                )
                                .with_suggestion(
                                    "Move the linker to the start of the utterance, or remove it \
                                     if no link is intended",
                                ),
                            );
                        } else {
                            filtered.push(tok.clone());
                        }
                    }
                    std::borrow::Cow::Owned(filtered)
                } else {
                    std::borrow::Cow::Borrowed(main_tier_tokens)
                };

                match main_tier::main_tier_parser()
                    .parse(tier_input.as_ref())
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
                        let dep_tiers = parse_dependent_tiers(lexed, &mut pos, errors);
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
                            && matches!(
                                TokenDiscriminants::from(&tokens[pos]),
                                TokenDiscriminants::TierPrefix
                                    | TokenDiscriminants::IncompleteTierPrefix
                            )
                        {
                            pos = skip_to_newline(tokens, pos);
                            if pos < tokens.len() {
                                pos += 1;
                            }
                        }
                    }
                }
            }

            // A line break not consumed by a header or utterance is a blank
            // line, unless malformed-line recovery left its terminator here.
            TokenDiscriminants::Newline => {
                lexed.report_blank_line(pos, errors);
                pos += 1;
            }

            // Skip remaining structural tokens
            TokenDiscriminants::Whitespace
            | TokenDiscriminants::Continuation
            | TokenDiscriminants::BOM => {
                pos += 1;
            }

            // Orphan tier prefix (no preceding main tier), report E319
            TokenDiscriminants::TierPrefix | TokenDiscriminants::IncompleteTierPrefix => {
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

            // The lexer already classified a whole unsupported line. Preserve
            // that fact instead of degrading it to a generic syntax failure.
            TokenDiscriminants::ErrorLine => {
                let (_, mut span) = lexed.token_at(pos);
                pos += 1;
                if matches!(tokens.get(pos), Some(Token::Newline(_))) {
                    span.end = lexed.token_at(pos).1.end;
                    pos += 1;
                }
                errors.report(ParseError::new(
                    talkbank_model::ErrorCode::UnexpectedLineType,
                    talkbank_model::Severity::Error,
                    talkbank_model::SourceLocation::from_offsets(span.start, span.end),
                    talkbank_model::ErrorContext::new(source, span.clone(), "unsupported_line"),
                    "Unsupported line skipped",
                ));
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
    // Asked of the annotation's own type rather than by listing seven token
    // variants here; `is_postfix` is the property, and `chat_text` reproduces
    // the marker as written even though the lexer tag-extracts most payloads.
    let offending = match items.first() {
        Some(ContentItem::OrphanAnnotation(annotation)) if annotation.is_postfix() => {
            Some(annotation.chat_text())
        }
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
    items.iter().any(|item| {
        matches!(item, ContentItem::Retrace(r) if r.synthesized_missing_annotation)
            || has_synthesized_missing_annotation(item.children())
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

/// Advance position to the Newline token (or end of tokens).
fn skip_to_newline(tokens: &[Token<'_>], mut pos: usize) -> usize {
    while pos < tokens.len()
        && TokenDiscriminants::from(&tokens[pos]) != TokenDiscriminants::Newline
    {
        pos += 1;
    }
    pos
}
