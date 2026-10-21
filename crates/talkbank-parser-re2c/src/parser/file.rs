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

#![deny(clippy::wildcard_enum_match_arm)]

use chumsky::Parser as _;

use crate::ast::*;
use crate::token::{Token, TokenDiscriminants};
use talkbank_model::{ErrorSink, ParseError};

mod dependent;
use super::main_tier;
pub(super) use dependent::parse_dependent_tiers;

/// Report E750 for whitespace hugging an angle-group delimiter: a
/// `LessThan` token immediately followed by whitespace, or whitespace
/// immediately followed by `GreaterThan` (`< dog>` / `<dog >`; CLAN
/// CHECK 160). Mirrors the tree-sitter parser's check in the group
/// parser (`main_tier/content/group/parser.rs`); token-level here
/// because the lexer tokenizes whitespace explicitly, so the pattern
/// is directly visible in the stream.
fn report_space_inside_angle_group(lexed: &super::LexedSource<'_>, errors: &impl ErrorSink) {
    let range = 0..lexed.tokens().len();
    for ((left, left_span), (right, right_span)) in lexed
        .located(range.clone())
        .zip(lexed.located(range).skip(1))
    {
        let offending_space = match (left, right) {
            (Token::LessThan(_), Token::Whitespace(_)) => Some((right_span, "after '<'")),
            (Token::Whitespace(_), Token::GreaterThan(_)) => Some((left_span, "before '>'")),
            _ => None,
        };
        if let Some((span, position)) = offending_space {
            errors.report(ParseError::new(
                talkbank_model::errors::codes::ErrorCode::SpaceInsideAngleGroup,
                talkbank_model::Severity::Error,
                talkbank_model::DiagnosticLocation::from_offsets(span.start, span.end),
                None,
                format!("Space is not allowed {position} in an angle-bracket group"),
            ));
        }
    }
}

/// Report a missing separator before an annotation (E375): a replacement or
/// any annotation token written directly after a word or a closing `>`/`”`.
/// Lexer locations stay paired with the admitted token categories. The
/// opposite glue, a code running into the following word (E757), is the
/// model's rule, which fires on the located wrappers this backend builds;
/// this pass used to mirror it while those wrappers were unlocated.
fn report_annotation_spacing(lexed: &super::LexedSource<'_>, errors: &impl ErrorSink) {
    let range = 0..lexed.tokens().len();
    for ((left, _), (right, span)) in lexed
        .located(range.clone())
        .zip(lexed.located(range).skip(1))
    {
        // A replacement is a separate token after a word, including a word
        // assembled from sub-tokens. Recovery retains the AST, but the missing
        // separator violates word_with_optional_annotations (CHECK 161).
        if (matches!(right, Token::ReplacementBegin(_))
            || super::classify::token_to_parsed_annotation(right.clone()).is_some())
            && (matches!(left, Token::Word { .. })
                || matches!(left, Token::GreaterThan(_) | Token::RightDoubleQuote(_))
                || super::classify::is_word_token(TokenDiscriminants::from(left)))
        {
            errors.report(ParseError::new(
                talkbank_model::ErrorCode::ContentAnnotationParseError,
                talkbank_model::Severity::Error,
                talkbank_model::DiagnosticLocation::from_offsets(span.start, span.end),
                None,
                "Annotation must be separated from its preceding content by whitespace",
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
        #[allow(
            clippy::wildcard_enum_match_arm,
            reason = "every other lexical token ends the initial linker run"
        )]
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
fn report_leading_zero_bullet_times(lexed: &super::LexedSource<'_>, errors: &impl ErrorSink) {
    for (tok, span) in lexed.located(0..lexed.tokens().len()) {
        if let Token::MediaBullet { bullet } = tok {
            for (component, which) in bullet.spellings() {
                if component.len() > 1 && component.starts_with('0') {
                    errors.report(ParseError::new(
                        talkbank_model::errors::codes::ErrorCode::LeadingZeroBulletTime,
                        talkbank_model::Severity::Error,
                        talkbank_model::DiagnosticLocation::from_offsets(span.start, span.end),
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

/// Report the lexer's refused numeric components at their original locations.
/// File and WOR-fragment entry points share this owner; conversion consumes
/// only admitted values, without reparsing or fabricating a replacement pair.
pub(crate) fn report_unrepresentable_bullet_times(
    lexed: &super::LexedSource<'_>,
    errors: &impl ErrorSink,
) {
    for (index, tok) in lexed.tokens().iter().enumerate() {
        let Token::MediaBullet { bullet } = tok else {
            continue;
        };
        for (component, which) in bullet.overflows() {
            // The same lexer run owns this token and its original extent.
            let (_, at) = lexed.token_at(index);
            errors.report(ParseError::new(
                talkbank_model::errors::codes::ErrorCode::InvalidMediaBullet,
                talkbank_model::Severity::Error,
                talkbank_model::DiagnosticLocation::new(talkbank_model::Span::new(
                    at.start, at.end,
                )),
                None,
                format!(
                    "Invalid media bullet: the {which} time '{component}' is \
                         numeric but too large to read as milliseconds"
                ),
            ));
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
    report_leading_zero_bullet_times(lexed, errors);
    report_unrepresentable_bullet_times(lexed, errors);
    report_space_inside_angle_group(lexed, errors);
    report_annotation_spacing(lexed, errors);
    let mut pos = 0;
    let mut lines = Vec::new();
    // This is only an observed document boundary, not a synthesized @Begin
    // requirement: header and utterance fragments use the same dispatcher.
    let mut ended = false;

    while pos < tokens.len() {
        if ended && !matches!(tokens[pos], Token::Newline(_) | Token::BOM(_)) {
            let end = skip_to_newline(tokens, pos);
            lexed.report_error(
                pos..end,
                talkbank_model::ErrorCode::UnexpectedSyntax,
                "Content follows the explicit @End boundary",
                errors,
            );
        }
        let d = TokenDiscriminants::from(&tokens[pos]);
        #[allow(
            clippy::wildcard_enum_match_arm,
            reason = "file dispatch reports unhandled lexical tokens; content traversal remains exhaustive"
        )]
        match d {
            // No-content headers
            TokenDiscriminants::HeaderUtf8
            | TokenDiscriminants::HeaderBegin
            | TokenDiscriminants::HeaderEnd
            | TokenDiscriminants::HeaderBlank
            | TokenDiscriminants::HeaderNewEpisode => {
                let header_start = pos;
                let tok = tokens[pos].clone();
                if matches!(tok, Token::HeaderEnd(_)) {
                    ended = true;
                }
                pos += 1;
                if pos < tokens.len()
                    && TokenDiscriminants::from(&tokens[pos]) == TokenDiscriminants::Newline
                {
                    pos += 1;
                }
                lines.push(Line::Header {
                    header: Box::new(HeaderParsed::Other {
                        prefix: tok,
                        content: vec![],
                    }),
                    provenance: lexed.header_provenance(
                        header_start..pos,
                        talkbank_model::model::TierSeparator::CLEAN,
                    ),
                });
            }

            // Headers with content
            TokenDiscriminants::HeaderPrefix
            | TokenDiscriminants::HeaderBirthOf
            | TokenDiscriminants::HeaderBirthplaceOf
            | TokenDiscriminants::HeaderL1Of => {
                let header_start = pos;
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
                let content = &tokens[content_start..pos];
                let malformed = if matches!(&prefix, Token::HeaderPrefix(p) if p.text().starts_with("@Languages:"))
                {
                    super::headers::languages_header_parser()
                        .parse(content)
                        .into_result()
                        .is_err()
                } else if matches!(&prefix, Token::HeaderPrefix(p) if p.text().starts_with("@Media:"))
                {
                    super::headers::media_header_parser()
                        .parse(content)
                        .into_result()
                        .is_err()
                } else if matches!(&prefix, Token::HeaderPrefix(p) if p.text().starts_with("@G:")) {
                    content
                        .iter()
                        .all(|token| matches!(token, Token::Whitespace(_) | Token::Continuation(_)))
                } else {
                    content.iter().any(|token| token.error_context().is_some())
                };
                if malformed
                    || (content.is_empty()
                        && matches!(&prefix, Token::HeaderPrefix(p) if p.text().starts_with("@Page:")))
                {
                    lexed.report_error(
                        header_start..pos,
                        talkbank_model::ErrorCode::UnparsableContent,
                        "Malformed or unsupported header content",
                        errors,
                    );
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
                lines.push(Line::Header {
                    header: Box::new(header),
                    provenance: lexed.header_provenance(header_start..pos, separator),
                });
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
                // retaining the adjacent words. The
                // chumsky combinators have no `ErrorSink` access, so the
                // diagnostic is emitted here from its original lexer location.
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
                    for (idx, (tok, span)) in lexed.located(start..pos).enumerate() {
                        if let Token::IllegalCurlyQuote(s) = tok {
                            errors.report(
                                ParseError::new(
                                    talkbank_model::errors::codes::ErrorCode::IllegalCurlyQuote,
                                    talkbank_model::Severity::Error,
                                    talkbank_model::DiagnosticLocation::from_offsets(
                                        span.start, span.end,
                                    ),
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
                                    talkbank_model::DiagnosticLocation::from_offsets(span.start, span.end),
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

                // The grammar proves the line's shape over the (possibly
                // filtered) token stream; the line machine places the line
                // from its own spans. A head the grammar accepted but the
                // machine cannot find is an internal fault, reported as such
                // rather than lowered without a location.
                let located = main_tier::main_tier_parser(lexed.source_text())
                    .parse(tier_input.as_ref())
                    .into_result()
                    .ok()
                    .map(|tier| (tier, lexed.main_tier_locations(start..pos)));
                match located {
                    Some((main_tier, Some(locations))) => {
                        // Recovery is not validity: this front end parses a
                        // LEADING postfix annotation (retrace / overlap /
                        // replacement) as a standalone `Annotation` item, but
                        // those codes scope over PRECEDING material, so an
                        // utterance that BEGINS with one is malformed (CLAN
                        // CHECK 52, "Item '%s' must be preceded by text.").
                        // Report E759 here, mirroring the tree-sitter error
                        // analysis; the AST is kept (recovery policy).
                        if has_orphan_annotation(&main_tier.tier_body.contents) {
                            lexed.report_error(
                                start..pos,
                                talkbank_model::ErrorCode::AnnotationAtUtteranceStart,
                                "Scoped annotation has no preceding host",
                                errors,
                            );
                        }
                        // A retained bare angle group is recovery, not valid
                        // CHAT. Its native structure proves the missing annotation;
                        // the original lexer region locates the containing tier.
                        if has_unannotated_group(&main_tier.tier_body.contents) {
                            lexed.report_error(
                                start..pos,
                                talkbank_model::errors::codes::ErrorCode::MissingRequiredElement,
                                "angle-bracket group must be followed by an annotation ([ ])",
                                errors,
                            );
                        }
                        let dep_tiers = parse_dependent_tiers(lexed, &mut pos, errors);
                        lines.push(Line::Utterance(Box::new(Utterance {
                            main_tier: LocatedMainTier::new(main_tier, locations),
                            dependent_tiers: dep_tiers,
                        })));
                    }
                    Some((_, None)) => {
                        lexed.report_error(
                            start..pos,
                            talkbank_model::errors::codes::ErrorCode::InternalError,
                            "main tier parsed from a line that does not begin `*SPK:`",
                            errors,
                        );
                        skip_orphan_dependent_tiers(tokens, &mut pos);
                    }
                    None => {
                        // Report E321: unparsable utterance.
                        lexed.report_error(
                            start..pos,
                            talkbank_model::errors::codes::ErrorCode::UnparsableUtterance,
                            "utterance could not be parsed",
                            errors,
                        );
                        skip_orphan_dependent_tiers(tokens, &mut pos);
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
                lexed.report_error(
                    line_start..pos,
                    talkbank_model::errors::codes::ErrorCode::UnparsableLine,
                    "orphan dependent tier (no preceding main tier)",
                    errors,
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
                    talkbank_model::DiagnosticLocation::from_offsets(span.start, span.end),
                    talkbank_model::ErrorContext::new(
                        source,
                        talkbank_model::Span::new(span.start, span.end),
                        "unsupported_line",
                    ),
                    "Unsupported line skipped",
                ));
            }

            // Unknown tokens, report and skip
            _ => {
                let (tok, span) = lexed.token_at(pos);
                errors.report(ParseError::new(
                    talkbank_model::errors::codes::ErrorCode::UnexpectedSyntax,
                    talkbank_model::Severity::Error,
                    talkbank_model::DiagnosticLocation::from_offsets(span.start, span.end),
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
fn has_orphan_annotation(items: &[ContentItem<'_>]) -> bool {
    items.iter().any(|item| {
        matches!(item, ContentItem::OrphanAnnotation(_)) || has_orphan_annotation(item.children())
    })
}

fn has_unannotated_group(items: &[ContentItem<'_>]) -> bool {
    items.iter().any(|item| {
        matches!(item, ContentItem::Group(group) if group.annotations.is_empty())
            || has_unannotated_group(item.children())
    })
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

/// Dependent tiers after a main tier that did not lower are orphaned without
/// it; consume them so they are not reported as orphans in their own right.
fn skip_orphan_dependent_tiers(tokens: &[Token<'_>], pos: &mut usize) {
    while *pos < tokens.len()
        && matches!(
            TokenDiscriminants::from(&tokens[*pos]),
            TokenDiscriminants::TierPrefix | TokenDiscriminants::IncompleteTierPrefix
        )
    {
        *pos = skip_to_newline(tokens, *pos);
        if *pos < tokens.len() {
            *pos += 1;
        }
    }
}
