//! Structural/header-level validation for CHAT file preambles.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Bg_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Eg_Header>

use crate::model::{Header, SpeakerCode};
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span};
use std::collections::{HashMap, HashSet};

/// Returns a gem label as `&str`, or `""` for unlabeled gems.
fn label_or_empty(label: Option<&str>) -> &str {
    // DEFAULT: Unlabeled gems are represented by an empty label string.
    label.unwrap_or_default()
}

/// Internal function: validate headers collection with ErrorSink
///
/// `source_len` is the end of the last line, where a missing required
/// header is reported: the end of the file is where the header is not.
/// Until 2026-09-09 it was an `Option` that the caller left `None` for a
/// file with no lines and this function read as offset 0; a file with no
/// lines ends at 0, so the caller says that and the fallback is gone.
pub(crate) fn check_headers(
    headers: &[(&Header, Span)],
    errors: &impl ErrorSink,
    source_len: usize,
) {
    let mut header_counts: HashMap<String, (usize, Span)> = HashMap::new();
    let mut declared_participants: HashSet<SpeakerCode> = HashSet::new();
    let mut id_speakers: Vec<(SpeakerCode, Span)> = Vec::new();

    for (header, span) in headers {
        let name_lower = header.name().to_lowercase();
        header_counts
            .entry(name_lower)
            .and_modify(|(count, _)| *count += 1)
            .or_insert((1, *span));

        if let Header::Participants { entries } = header {
            for entry in entries {
                declared_participants.insert(entry.speaker_code.clone());
            }
        }

        if let Header::ID(id_header) = header {
            id_speakers.push((id_header.speaker.clone(), *span));
        }
    }

    let single_only_headers = ["Types", "Media", "Videos", "UTF8", "Begin", "End"];
    for name in &single_only_headers {
        let name_lower = name.to_lowercase();
        if let Some(&(count, span)) = header_counts.get(&name_lower)
            && count > 1
        {
            let mut err = ParseError::new(
                ErrorCode::DuplicateHeader,
                Severity::Error,
                SourceLocation::at_offset(span.start as usize),
                ErrorContext::new(*name, 0..name.len(), *name),
                format!(
                    "Duplicate @{} header: found {} occurrences, but only one is allowed",
                    name, count
                ),
            )
            .with_suggestion(format!(
                "Remove the extra @{} headers so only one remains",
                name
            ));
            err.location.span = span;
            errors.report(err);
        }
    }

    // Missing-header errors point at the end of the file.
    let eof_offset = source_len;

    let required_headers = ["Begin", "Languages", "Participants"];
    for required in &required_headers {
        let required_lower = required.to_lowercase();
        if !header_counts.contains_key(&required_lower) {
            errors.report(
                ParseError::new(
                    ErrorCode::MissingRequiredHeader,
                    Severity::Error,
                    SourceLocation::at_offset(eof_offset),
                    ErrorContext::new("", 0..0, ""),
                    format!("Missing required @{} header in file preamble", required),
                )
                .with_suggestion(format!(
                    "Add an @{} line to the file header section (before any utterances)",
                    required
                )),
            );
        }
    }

    // E503: @UTF8 must be present (spec requires it as the first line)
    if !header_counts.contains_key("utf8") {
        errors.report(
            ParseError::new(
                ErrorCode::MissingUTF8Header,
                Severity::Error,
                SourceLocation::at_offset(eof_offset),
                ErrorContext::new("", 0..0, ""),
                "Missing @UTF8 header: every CHAT file must declare its encoding",
            )
            .with_suggestion("Add @UTF8 as the very first line of the file"),
        );
    }

    if !header_counts.contains_key("end") {
        errors.report(
            ParseError::new(
                ErrorCode::MissingEndHeader,
                Severity::Error,
                SourceLocation::at_offset(eof_offset),
                ErrorContext::new("", 0..0, ""),
                "Missing @End header at end of file",
            )
            .with_suggestion("Add @End as the last line of the file"),
        );
    }

    // Header ordering checks. NOTE: the immediate-follows family below
    // (E547/E548/E551) all share one shape, a single pass tracking `prev` plus a
    // per-rule allowed-predecessor set. They are kept as explicit sibling
    // functions for readability; if a 4th immediate-follows rule appears,
    // collapse them into one table-driven `check_immediate_follows` helper.
    // (E543 is a precedence rule, "X before @Participants", not an
    // immediate-follows rule, so it stays separate.)
    //
    // E543: Check header ordering, @Participants must precede @Options and @ID
    check_header_order(headers, errors);

    // E547: Constant participant headers (@Birth of / @Birthplace of / @L1 of)
    // must immediately follow the @ID block, before any changeable header.
    check_constant_participant_header_order(headers, errors);

    // E548: The @ID block must immediately follow @Participants / @Options,
    // with no changeable header (e.g. @Comment) intervening.
    check_id_header_order(headers, errors);

    // E551: The @Options header, when present, must immediately follow
    // @Participants (before the @ID block or any other header).
    check_options_header_order(headers, errors);

    // E549: No speaker code may be declared more than once in @Participants.
    check_duplicate_participants(headers, errors);

    // E549 (CLAN 13 again): the same speaker must not have more than one @ID
    // header. CLAN flags duplicate speaker declarations whether they arise from
    // a repeated @Participants entry or a repeated @ID line.
    check_duplicate_id_headers(&id_speakers, errors);

    for (speaker, span) in &id_speakers {
        if !speaker.as_str().is_empty() && !declared_participants.contains(speaker) {
            let speaker_str = speaker.as_str();
            let mut err = ParseError::new(
                ErrorCode::SpeakerNotDefined,
                Severity::Error,
                SourceLocation::at_offset(span.start as usize),
                ErrorContext::new(speaker_str, 0..speaker_str.len(), speaker_str),
                format!(
                    "Speaker '{}' referenced in @ID header but not declared in @Participants",
                    speaker_str
                ),
            )
            .with_suggestion(format!(
                "Add '{}' to the @Participants line, or remove this @ID header",
                speaker_str
            ));
            err.location.span = *span;
            errors.report(err);
        }
    }

    // E526, E527, E528: Validate @Bg/@Eg matching
    check_gem_balance(headers, errors);
}

/// Check that headers appear in canonical order.
///
/// The CHAT spec requires:
/// - `@Participants` must appear before `@Options`
/// - `@Participants` must appear before `@ID`
///
/// This corresponds to CLAN CHECK errors 61 and 125.
fn check_header_order(headers: &[(&Header, Span)], errors: &impl ErrorSink) {
    let mut saw_participants = false;

    for (header, span) in headers {
        match header {
            Header::Participants { .. } => {
                saw_participants = true;
            }
            Header::Options { .. } if !saw_participants => {
                let mut err = ParseError::new(
                    ErrorCode::HeaderOutOfOrder,
                    Severity::Error,
                    SourceLocation::at_offset(span.start as usize),
                    ErrorContext::new("@Options", 0.."@Options".len(), "@Options"),
                    "@Options must appear after @Participants",
                )
                .with_suggestion("Move @Options to after the @Participants header");
                err.location.span = *span;
                errors.report(err);
            }
            Header::ID(_) if !saw_participants => {
                let mut err = ParseError::new(
                    ErrorCode::HeaderOutOfOrder,
                    Severity::Error,
                    SourceLocation::at_offset(span.start as usize),
                    ErrorContext::new("@ID", 0.."@ID".len(), "@ID"),
                    "@ID must appear after @Participants",
                )
                .with_suggestion("Move @ID to after the @Participants header");
                err.location.span = *span;
                errors.report(err);
            }
            _ => {}
        }
    }
}

/// Check that constant participant-specific headers immediately follow the
/// `@ID` block.
///
/// The CHAT spec requires `@Birth of`, `@Birthplace of`, and `@L1 of` to appear
/// directly after the `@ID` headers, before any changeable header such as
/// `@Comment`, `@Date`, or `@Situation`. A changeable header between the `@ID`
/// block and a constant participant header is an ordering violation: the
/// constant header no longer "immediately follows" `@ID`.
///
/// This corresponds to CLAN CHECK error 127 ("Header must follow @ID: or
/// @Birth of or @Birthplace of or @L1 of header").
fn check_constant_participant_header_order(headers: &[(&Header, Span)], errors: &impl ErrorSink) {
    // The only headers that may legally precede a constant participant header:
    // an `@ID` header, or another constant participant header.
    fn is_allowed_predecessor(header: &Header) -> bool {
        matches!(
            header,
            Header::ID(_) | Header::Birth { .. } | Header::Birthplace { .. } | Header::L1Of { .. }
        )
    }

    let mut prev: Option<&Header> = None;
    for (header, span) in headers {
        // Identify the constant participant header (and its keyword for the
        // diagnostic); `None` for every other header.
        let keyword = match header {
            Header::Birth { .. } => Some("@Birth of"),
            Header::Birthplace { .. } => Some("@Birthplace of"),
            Header::L1Of { .. } => Some("@L1 of"),
            _ => None,
        };

        if let Some(keyword) = keyword
            && !prev.is_some_and(is_allowed_predecessor)
        {
            let mut err = ParseError::new(
                ErrorCode::ConstantHeaderOutOfOrder,
                Severity::Error,
                SourceLocation::at_offset(span.start as usize),
                ErrorContext::new(keyword, 0..keyword.len(), keyword),
                format!(
                    "{keyword} must immediately follow the @ID block, before any changeable header"
                ),
            )
            .with_suggestion(
                "Move this header to directly after the @ID headers, before any @Comment, @Date, or other changeable header",
            );
            err.location.span = *span;
            errors.report(err);
        }

        prev = Some(header);
    }
}

/// Check that the `@ID` block immediately follows `@Participants` / `@Options`.
///
/// Per the CHAT spec the `@ID` headers come directly after `@Participants` (and
/// the optional `@Options`), with no other header in between; subsequent `@ID`
/// headers follow one another. An `@ID` whose immediately-preceding header is
/// something else (e.g. `@Comment`), once `@Participants` has been seen, is an
/// ordering violation. The distinct case of an `@ID` appearing *before*
/// `@Participants` is handled by [`check_header_order`] (E543), so this check is
/// gated on `@Participants` already having been seen to avoid double-reporting.
///
/// This corresponds to CLAN CHECK error 126 ("@ID header must immediately
/// follow @Participants: or @Options header").
fn check_id_header_order(headers: &[(&Header, Span)], errors: &impl ErrorSink) {
    let mut saw_participants = false;
    let mut prev: Option<&Header> = None;
    for (header, span) in headers {
        if matches!(header, Header::Participants { .. }) {
            saw_participants = true;
        }
        if matches!(header, Header::ID(_)) {
            // Valid immediate predecessors of an @ID: @Participants, @Options,
            // or another @ID.
            let preceded_validly = prev.is_some_and(|p| {
                matches!(
                    p,
                    Header::Participants { .. } | Header::Options { .. } | Header::ID(_)
                )
            });
            if saw_participants && !preceded_validly {
                let mut err = ParseError::new(
                    ErrorCode::IdHeaderOutOfOrder,
                    Severity::Error,
                    SourceLocation::at_offset(span.start as usize),
                    ErrorContext::new("@ID", 0.."@ID".len(), "@ID"),
                    "@ID must immediately follow @Participants / @Options (or another @ID), with no changeable header in between",
                )
                .with_suggestion(
                    "Move this @ID up to directly follow @Participants / @Options, before any @Comment or other changeable header",
                );
                err.location.span = *span;
                errors.report(err);
            }
        }
        prev = Some(header);
    }
}

/// Check that no speaker has more than one `@ID` header.
///
/// CLAN CHECK error 13 ("Duplicate speaker declaration") fires both for a
/// speaker code repeated in `@Participants` (see
/// [`check_duplicate_participants`]) and for two `@ID` lines naming the same
/// speaker. Each speaker has exactly one `@ID`, so a repeat is a duplicate
/// declaration. Empty speaker codes are skipped (an absent code is a different
/// error, not a duplicate).
fn check_duplicate_id_headers(id_speakers: &[(SpeakerCode, Span)], errors: &impl ErrorSink) {
    let mut seen: HashSet<&str> = HashSet::new();
    for (speaker, span) in id_speakers {
        let code = speaker.as_str();
        if code.is_empty() {
            continue;
        }
        if !seen.insert(code) {
            let mut err = ParseError::new(
                ErrorCode::DuplicateSpeakerDeclaration,
                Severity::Error,
                SourceLocation::at_offset(span.start as usize),
                ErrorContext::new(code, 0..code.len(), code),
                format!("Speaker '{code}' has more than one @ID header"),
            )
            .with_suggestion("Declare each speaker with exactly one @ID header");
            err.location.span = *span;
            errors.report(err);
        }
    }
}

/// Check that the `@Options` header immediately follows `@Participants`.
///
/// Per the CHAT spec the optional `@Options` line, when present, comes directly
/// after `@Participants`, before the `@ID` block. An `@Options` whose
/// immediately-preceding header is something else (e.g. an `@ID` or `@Comment`),
/// once `@Participants` has been seen, is an ordering violation. The distinct
/// case of `@Options` appearing *before* `@Participants` is handled by
/// [`check_header_order`] (E543), so this check is gated on `@Participants`
/// already having been seen to avoid double-reporting.
///
/// This corresponds to CLAN CHECK error 125 ("@Options header must immediately
/// follow @Participants: header").
fn check_options_header_order(headers: &[(&Header, Span)], errors: &impl ErrorSink) {
    let mut saw_participants = false;
    let mut prev: Option<&Header> = None;
    for (header, span) in headers {
        if matches!(header, Header::Participants { .. }) {
            saw_participants = true;
        }
        if matches!(header, Header::Options { .. }) {
            // The only valid immediate predecessor of @Options is @Participants.
            let preceded_validly = matches!(prev, Some(Header::Participants { .. }));
            if saw_participants && !preceded_validly {
                let mut err = ParseError::new(
                    ErrorCode::OptionsHeaderOutOfOrder,
                    Severity::Error,
                    SourceLocation::at_offset(span.start as usize),
                    ErrorContext::new("@Options", 0.."@Options".len(), "@Options"),
                    "@Options must immediately follow @Participants, before the @ID block or any other header",
                )
                .with_suggestion(
                    "Move this @Options up to directly follow @Participants, before any @ID or @Comment header",
                );
                err.location.span = *span;
                errors.report(err);
            }
        }
        prev = Some(header);
    }
}

/// Check that no speaker code is declared more than once in `@Participants`
/// (E549).
///
/// Corresponds to CLAN CHECK error 13 ("Duplicate speaker declaration").
fn check_duplicate_participants(headers: &[(&Header, Span)], errors: &impl ErrorSink) {
    use std::collections::HashSet;
    for (header, span) in headers {
        if let Header::Participants { entries } = header {
            let mut seen: HashSet<&str> = HashSet::new();
            for entry in entries.as_slice() {
                let code = entry.speaker_code.as_str();
                if !seen.insert(code) {
                    let mut err = ParseError::new(
                        ErrorCode::DuplicateSpeakerDeclaration,
                        Severity::Error,
                        SourceLocation::at_offset(span.start as usize),
                        ErrorContext::new(code, 0..code.len(), code),
                        format!("Speaker '{code}' is declared more than once in @Participants"),
                    )
                    .with_suggestion("Declare each participant exactly once in @Participants");
                    err.location.span = *span;
                    errors.report(err);
                }
            }
        }
    }
}

/// Validate that @Bg (Begin Gem) and @Eg (End Gem) markers are properly matched.
///
/// Checks:
/// - E526: Every @Bg has a matching @Eg
/// - E527: Every @Eg has a matching @Bg
/// - E528: Labels match between paired @Bg/@Eg
/// - E529: Nested @Bg with same label (opening @Bg while already in that scope)
/// - E530: @G (lazy gem) inside @Bg/@Eg scope
fn check_gem_balance(headers: &[(&Header, Span)], errors: &impl ErrorSink) {
    use std::collections::HashMap;

    // Track open scopes by label (None for unlabeled gems)
    let mut open_scopes: HashMap<Option<String>, usize> = HashMap::new();

    for (header, span) in headers {
        match header {
            Header::BeginGem { label } => {
                let key = label.as_ref().map(|l| l.as_str().to_string());
                let current_count = open_scopes.get(&key).copied().unwrap_or(0);

                // E529: Nested @Bg with the same label is not allowed.
                // Different labels are permitted (stack-based LIFO scoping).
                if current_count > 0 {
                    let label_str = label_or_empty(key.as_deref());
                    let mut err = ParseError::new(
                        ErrorCode::NestedBeginGem,
                        Severity::Error,
                        SourceLocation::at_offset(span.start as usize),
                        ErrorContext::new("", 0..0, ""),
                        if label_str.is_empty() {
                            "Nested @Bg: cannot open a new @Bg while already inside a @Bg scope with the same label"
                                .to_string()
                        } else {
                            format!(
                                "Nested @Bg:{0}: cannot open a new @Bg:{0} while already inside a @Bg:{0} scope",
                                label_str
                            )
                        },
                    )
                    .with_suggestion(
                        "Close the current @Bg scope with @Eg before opening another @Bg with the same label"
                            .to_string(),
                    );
                    err.location.span = *span;
                    errors.report(err);
                }

                *open_scopes.entry(key).or_insert(0) += 1;
            }
            Header::LazyGem { label } => {
                // E530: Check if any @Bg scope is open
                let any_scope_open = open_scopes.values().any(|&count| count > 0);
                if any_scope_open {
                    let label_str = label_or_empty(label.as_ref().map(|l| l.as_str()));
                    let mut err = ParseError::new(
                        ErrorCode::LazyGemInsideScope,
                        Severity::Error,
                        SourceLocation::at_offset(span.start as usize),
                        ErrorContext::new("", 0..0, ""),
                        if label_str.is_empty() {
                            "@G (lazy gem) cannot appear inside @Bg/@Eg scope".to_string()
                        } else {
                            format!(
                                "@G:{} (lazy gem) cannot appear inside @Bg/@Eg scope",
                                label_str
                            )
                        },
                    )
                    .with_suggestion(
                        "Move @G outside of @Bg/@Eg scope, or use @Bg/@Eg markers instead"
                            .to_string(),
                    );
                    err.location.span = *span;
                    errors.report(err);
                }
            }
            Header::EndGem { label } => {
                let key = label.as_ref().map(|l| l.as_str().to_string());
                let has_any_open_scope = open_scopes.values().any(|&count| count > 0);
                let count = open_scopes.get_mut(&key);

                if let Some(count) = count {
                    if *count > 0 {
                        *count -= 1;
                    } else {
                        if has_any_open_scope {
                            let label_str = label_or_empty(key.as_deref());
                            let mut err = ParseError::new(
                                ErrorCode::GemLabelMismatch,
                                Severity::Error,
                                SourceLocation::at_offset(span.start as usize),
                                ErrorContext::new("", 0..0, ""),
                                if label_str.is_empty() {
                                    "Gem label mismatch between @Bg/@Eg markers".to_string()
                                } else {
                                    format!(
                                        "Gem label mismatch: @Eg:{} does not match active @Bg scope",
                                        label_str
                                    )
                                },
                            );
                            err.location.span = *span;
                            errors.report(err);
                        }
                        // End without matching begin
                        let label_str = label_or_empty(key.as_deref());
                        let mut err = ParseError::new(
                            ErrorCode::UnmatchedEndGem,
                            Severity::Error,
                            SourceLocation::at_offset(span.start as usize),
                            ErrorContext::new("", 0..0, ""),
                            if label_str.is_empty() {
                                "Unmatched @Eg (no matching @Bg)".to_string()
                            } else {
                                format!(
                                    "Unmatched @Eg:{} (no matching @Bg:{})",
                                    label_str, label_str
                                )
                            },
                        )
                        .with_suggestion(if label_str.is_empty() {
                            "Add a matching @Bg before this @Eg".to_string()
                        } else {
                            format!(
                                "Add a matching @Bg:{} before this @Eg:{}",
                                label_str, label_str
                            )
                        });
                        err.location.span = *span;
                        errors.report(err);
                    }
                } else {
                    if has_any_open_scope {
                        let label_str = label_or_empty(key.as_deref());
                        let mut err = ParseError::new(
                            ErrorCode::GemLabelMismatch,
                            Severity::Error,
                            SourceLocation::at_offset(span.start as usize),
                            ErrorContext::new("", 0..0, ""),
                            if label_str.is_empty() {
                                "Gem label mismatch between @Bg/@Eg markers".to_string()
                            } else {
                                format!(
                                    "Gem label mismatch: @Eg:{} does not match active @Bg scope",
                                    label_str
                                )
                            },
                        );
                        err.location.span = *span;
                        errors.report(err);
                    }
                    // End without any matching begin (different label)
                    let label_str = label_or_empty(key.as_deref());
                    let mut err = ParseError::new(
                        ErrorCode::UnmatchedEndGem,
                        Severity::Error,
                        SourceLocation::at_offset(span.start as usize),
                        ErrorContext::new("", 0..0, ""),
                        if label_str.is_empty() {
                            "Unmatched @Eg (no matching @Bg)".to_string()
                        } else {
                            format!(
                                "Unmatched @Eg:{} (no matching @Bg:{})",
                                label_str, label_str
                            )
                        },
                    )
                    .with_suggestion(if label_str.is_empty() {
                        "Add a matching @Bg before this @Eg".to_string()
                    } else {
                        format!(
                            "Add a matching @Bg:{} before this @Eg:{}",
                            label_str, label_str
                        )
                    });
                    err.location.span = *span;
                    errors.report(err);
                }
            }
            _ => {}
        }
    }

    // Check for unclosed scopes, no specific header to point at (scope was opened earlier)
    for (label_opt, count) in open_scopes {
        if count > 0 {
            let label_str = label_or_empty(label_opt.as_deref());
            errors.report(
                ParseError::new(
                    ErrorCode::UnmatchedBeginGem,
                    Severity::Error,
                    SourceLocation::at_offset(0),
                    ErrorContext::new("", 0..0, ""),
                    if label_str.is_empty() {
                        format!("Unmatched @Bg: {} @Bg without matching @Eg", count)
                    } else {
                        format!(
                            "Unmatched @Bg:{}: {} @Bg:{} without matching @Eg:{}",
                            label_str, count, label_str, label_str
                        )
                    },
                )
                .with_suggestion(if label_str.is_empty() {
                    format!("Add {} matching @Eg marker(s)", count)
                } else {
                    format!("Add {} matching @Eg:{} marker(s)", count, label_str)
                }),
            );
        }
    }
}

// The tests that stood here moved to
// `talkbank-parser-tests/tests/integration/header_structure_from_source.rs`
// on 2026-09-08.
//
// All twenty-four handed `check_header_order`, `check_gem_balance` and their
// siblings a `Vec<(&Header, Span)>` assembled in the test, every position
// `Span::DUMMY`. Both facts under test are properties of a header BLOCK, which
// a `.cha` file states directly, so the assembled sequence was a second way of
// writing what the format already writes, with nothing forcing the two to
// describe the same thing. The coverage it produced was fabrication-backed:
// these functions ran and no header line was ever read.
//
// A `#[cfg(test)] mod` here cannot parse a header block, because the
// lib-test target is a second instantiation of this crate and the parser's
// `Header` is not its `Header`; an integration test under `tests/` with a
// dev-dependency cycle could, and `talkbank-parser-tests` is the cheaper home
// because it already depends on every parser. Every assertion the twenty-four
// made travels: the exactly-one count, the message fragment each pinned (E543
// says `@Options` or `@ID` depending on which is out of order, and a test that
// checks only the code passes after the two messages are swapped), and the
// three-error residual stack of the label-mismatch case. One case is new: a
// balanced gem is now checked against E527 as well as E526.
