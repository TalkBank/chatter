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
//! reports the failure to the `ErrorSink` and retains the corresponding
//! rejected-tier or header recovery state.

#![deny(clippy::wildcard_enum_match_arm)]

use chumsky::Parser as _;

use crate::ast::*;
use crate::token::{Token, TokenDiscriminants};
use talkbank_model::{ErrorSink, ParseError};

mod dependent;
mod headers;
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
    for ((left, left_span), (right, right_span)) in lexed.located().zip(lexed.located().skip(1)) {
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
    for ((left, _), (right, span)) in lexed.located().zip(lexed.located().skip(1)) {
        // A replacement is a separate token after a word or action.
        // Recovery retains the AST, but the missing
        // separator violates word_with_optional_annotations (CHECK 161).
        if (matches!(right, Token::ReplacementBegin(_))
            || super::classify::token_to_parsed_annotation(right.clone(), lexed.source_text())
                .is_some())
            && (matches!(left, Token::Word { .. })
                || matches!(left, Token::GreaterThan(_) | Token::RightDoubleQuote(_))
                || matches!(left, Token::Zero(_)))
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
    use super::classify::{is_terminator, linker_kind};

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
            _ if linker_kind(d).is_some() => {
                if matches!(phase, LinePhase::AfterContent) {
                    scan.misplaced_linkers.push(idx);
                }
            }
            _ if is_terminator(d) => phase = LinePhase::PastTerminator,
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
    for (tok, span) in lexed.located() {
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
    for (tok, at) in lexed.located() {
        let Token::MediaBullet { bullet } = tok else {
            continue;
        };
        for (component, which) in bullet.overflows() {
            // The same lexer run owns this token and its original extent.
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
    let source = lexed.source();
    report_leading_zero_bullet_times(lexed, errors);
    report_unrepresentable_bullet_times(lexed, errors);
    report_space_inside_angle_group(lexed, errors);
    report_annotation_spacing(lexed, errors);
    let mut cursor = lexed.cursor();
    let mut lines = Vec::new();
    // This is only an observed document boundary, not a synthesized @Begin
    // requirement: header and utterance fragments use the same dispatcher.
    let mut ended = false;

    while let Some(current) = cursor.peek() {
        if ended && !matches!(*current.token(), Token::Newline(_) | Token::BOM(_)) {
            let end = cursor.line_end_position();
            let Ok(region) = lexed.admit_region(current.index()..end, errors) else {
                break;
            };
            region.report_error(
                talkbank_model::ErrorCode::UnexpectedSyntax,
                "Content follows the explicit @End boundary",
                errors,
            );
        }
        #[allow(
            clippy::wildcard_enum_match_arm,
            reason = "file dispatch reports unhandled lexical tokens; content traversal remains exhaustive"
        )]
        match current.token() {
            // No-content headers
            Token::HeaderUtf8(_)
            | Token::HeaderBegin(_)
            | Token::HeaderEnd(_)
            | Token::HeaderBlank(_)
            | Token::HeaderNewEpisode(_) => {
                let header_start = current.index();
                let tok = current.token().clone();
                if matches!(tok, Token::HeaderEnd(_)) {
                    ended = true;
                }
                cursor.advance();
                let _ = cursor.take_newline();
                let Ok(region) = lexed.admit_region(header_start..cursor.position(), errors) else {
                    break;
                };
                lines.push(Line::Header(Box::new(region.locate_header(
                    HeaderParsed::Other {
                        source: region.source_slice().text().trim_end_matches(['\r', '\n']),
                        prefix: tok,
                        content: vec![],
                    },
                    talkbank_model::model::TierSeparator::CLEAN,
                ))));
            }

            Token::HeaderPrefix(prefix) => {
                let Ok(header) = headers::ContentHeaderPrefix::Named(prefix.clone())
                    .parse_line(&mut cursor, errors)
                else {
                    break;
                };
                lines.push(Line::Header(Box::new(header)));
            }
            Token::HeaderBirthOf(prefix) => {
                let Ok(header) = headers::ContentHeaderPrefix::BirthOf(prefix.clone())
                    .parse_line(&mut cursor, errors)
                else {
                    break;
                };
                lines.push(Line::Header(Box::new(header)));
            }
            Token::HeaderBirthplaceOf(prefix) => {
                let Ok(header) = headers::ContentHeaderPrefix::BirthplaceOf(prefix.clone())
                    .parse_line(&mut cursor, errors)
                else {
                    break;
                };
                lines.push(Line::Header(Box::new(header)));
            }
            Token::HeaderL1Of(prefix) => {
                let Ok(header) = headers::ContentHeaderPrefix::L1Of(prefix.clone())
                    .parse_line(&mut cursor, errors)
                else {
                    break;
                };
                lines.push(Line::Header(Box::new(header)));
            }

            // Main tier
            Token::Star(_) => {
                let start = current.index();
                cursor.skip_to_newline();
                let _ = cursor.take_newline();

                let Ok(region) = lexed.admit_region(start..cursor.position(), errors) else {
                    break;
                };
                let main_tier_tokens = region.tokens();

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
                    for (idx, (tok, span)) in region.located().enumerate() {
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
                let located = main_tier::main_tier_parser(region.source_text())
                    .parse(tier_input.as_ref())
                    .into_result()
                    .ok()
                    .map(|tier| (tier, region.main_tier_locations()));
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
                            region.report_error(
                                talkbank_model::ErrorCode::AnnotationAtUtteranceStart,
                                "Scoped annotation has no preceding host",
                                errors,
                            );
                        }
                        // A retained bare angle group is recovery, not valid
                        // CHAT. Its native structure proves the missing annotation;
                        // the original lexer region locates the containing tier.
                        if has_unannotated_group(&main_tier.tier_body.contents) {
                            region.report_error(
                                talkbank_model::errors::codes::ErrorCode::MissingRequiredElement,
                                "angle-bracket group must be followed by an annotation ([ ])",
                                errors,
                            );
                        }
                        let dep_tiers = parse_dependent_tiers(&mut cursor, errors);
                        lines.push(Line::Utterance(Box::new(Utterance::from_parsed(
                            LocatedMainTier::new(main_tier, locations, region.source_slice()),
                            dep_tiers,
                        ))));
                    }
                    Some((_, None)) => {
                        region.report_error(
                            talkbank_model::errors::codes::ErrorCode::InternalError,
                            "main tier parsed from a line that does not begin `*SPK:`",
                            errors,
                        );
                        skip_orphan_dependent_tiers(&mut cursor);
                    }
                    None => {
                        // Report E321: unparsable utterance.
                        region.report_error(
                            talkbank_model::errors::codes::ErrorCode::UnparsableUtterance,
                            "utterance could not be parsed",
                            errors,
                        );
                        skip_orphan_dependent_tiers(&mut cursor);
                    }
                }
            }

            // A line break not consumed by a header or utterance is a blank
            // line, unless malformed-line recovery left its terminator here.
            Token::Newline(_) => {
                current.report_blank_line(errors);
                cursor.advance();
            }

            // Skip remaining structural tokens
            Token::Whitespace(_) | Token::Continuation(_) | Token::BOM(_) => {
                cursor.advance();
            }

            // Orphan tier prefix (no preceding main tier), report E319
            Token::TierPrefix(_) | Token::IncompleteTierPrefix(_) => {
                let line_start = current.index();
                cursor.skip_to_newline();
                let _ = cursor.take_newline();
                let Ok(region) = lexed.admit_region(line_start..cursor.position(), errors) else {
                    break;
                };
                region.report_error(
                    talkbank_model::errors::codes::ErrorCode::UnparsableLine,
                    "orphan dependent tier (no preceding main tier)",
                    errors,
                );
            }

            // The lexer already classified a whole unsupported line. Preserve
            // that fact instead of degrading it to a generic syntax failure.
            Token::ErrorLine(_) => {
                let mut span = current.span();
                cursor.advance();
                if let Some(newline) = cursor.take_newline() {
                    span.end = newline.end;
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
                let tok = current.token();
                let span = current.span();
                errors.report(ParseError::new(
                    talkbank_model::errors::codes::ErrorCode::UnexpectedSyntax,
                    talkbank_model::Severity::Error,
                    talkbank_model::DiagnosticLocation::from_offsets(span.start, span.end),
                    None,
                    format!("unhandled token in parse_chat_file: {:?}", tok.text()),
                ));
                cursor.advance();
            }
        }
    }

    ChatFile::from_parsed(lines, source)
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
    items
        .iter()
        .map(crate::ast::ContentRef::Item)
        .any(has_orphan_annotation_item)
}

fn has_orphan_annotation_item(item: crate::ast::ContentRef<'_, '_>) -> bool {
    matches!(
        item,
        crate::ast::ContentRef::Item(ContentItem::OrphanAnnotation(_))
    ) || item.children().any(has_orphan_annotation_item)
}

fn has_unannotated_group(items: &[ContentItem<'_>]) -> bool {
    items
        .iter()
        .map(crate::ast::ContentRef::Item)
        .any(has_unannotated_group_item)
}

fn has_unannotated_group_item(item: crate::ast::ContentRef<'_, '_>) -> bool {
    use crate::ast::{Chain, ContentRef};
    let unannotated = matches!(item, ContentRef::Item(ContentItem::Group(group)) if group.annotations.is_empty())
        || matches!(item, ContentRef::Chain(Chain::Group(_, _, annotations)) if annotations.is_empty());
    unannotated || item.children().any(has_unannotated_group_item)
}

/// Dependent tiers after a main tier that did not lower are orphaned without
/// it; consume them without misreporting each as an independent orphan.
fn skip_orphan_dependent_tiers(cursor: &mut super::TokenCursor<'_, '_>) {
    while cursor.peek().is_some_and(|next| {
        matches!(
            next.token(),
            Token::TierPrefix(_) | Token::IncompleteTierPrefix(_)
        )
    }) {
        cursor.skip_to_newline();
        let _ = cursor.take_newline();
    }
}
