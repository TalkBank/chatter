// This file used to carry `#![allow(clippy::unreachable)]` and a paragraph
// explaining why three `_ => unreachable!()` arms were safe: each was guarded by
// an outer char-classification match that enumerated the same characters. That
// is a relationship between two lists maintained by convention, and the comment
// was the receipt for it.
//
// All three mappings are now total functions returning `Option`, and the match
// arms BIND that `Option` in an `if let` guard, so the classification happens
// once and the "in a CA arm with no kind" state has no arm to live in. There is
// nothing left to allow, and nothing left to explain away.

//! Word body parser, scans a `&str` body for internal structure.
//!
//! The lexer determines word boundaries and extracts prefix/suffixes.
//! The body contains: text segments, shortenings, lengthening,
//! compound markers, stress, overlap points, syllable pause,
//! clitic boundary, CA elements/delimiters, underline markers.
//!
//! This is char-level scanning, not token-level, chumsky does not
//! apply here.

use crate::ast::*;

/// Parse a word body string into structured `WordBodyItem` list.
/// The body is the interior of a word (no prefix, no suffixes).
pub fn parse_word_body(body: &str) -> Vec<WordBodyItem<'_>> {
    let mut items = Vec::new();
    let mut chars = body.char_indices().peekable();

    while let Some(&(i, ch)) = chars.peek() {
        match ch {
            // Shortening: (text)
            '(' => {
                chars.next();
                let content_start = chars.peek().map_or(body.len(), |&(j, _)| j);
                // Scan to closing )
                while let Some(&(_, c)) = chars.peek() {
                    if c == ')' {
                        break;
                    }
                    chars.next();
                }
                let content_end = chars.peek().map_or(body.len(), |&(j, _)| j);
                if chars.peek().is_some() {
                    chars.next(); // consume ')'
                }
                items.push(WordBodyItem::Shortening(&body[content_start..content_end]));
            }
            // Lengthening: one or more colons
            ':' => {
                let mut count: u8 = 0;
                while let Some(&(_, ':')) = chars.peek() {
                    chars.next();
                    count += 1;
                }
                items.push(WordBodyItem::Lengthening(count));
            }
            // Compound marker
            '+' => {
                chars.next();
                items.push(WordBodyItem::CompoundMarker);
            }
            // Stress markers
            '\u{02C8}' => {
                chars.next();
                items.push(WordBodyItem::Stress(StressKind::Primary));
            }
            '\u{02CC}' => {
                chars.next();
                items.push(WordBodyItem::Stress(StressKind::Secondary));
            }
            // Syllable pause
            '^' => {
                chars.next();
                items.push(WordBodyItem::SyllablePause);
            }
            // Clitic boundary
            '~' => {
                chars.next();
                items.push(WordBodyItem::CliticBoundary);
            }
            // Overlap points: ⌈ ⌉ ⌊ ⌋ with optional digit.
            _ if let Some(kind) = char_to_overlap_kind(ch) => {
                chars.next();
                // Include the overlap char + optional digit in the slice
                let end = chars.peek().map_or(body.len(), |&(j, _)| j);
                let overlap_text = &body[i..end];
                // Check for trailing digit
                if let Some(&(_, d)) = chars.peek()
                    && d.is_ascii_digit()
                    && d != '0'
                {
                    chars.next();
                    let end2 = chars.peek().map_or(body.len(), |&(j, _)| j);
                    items.push(WordBodyItem::OverlapPoint(kind, &body[i..end2]));
                    continue;
                }
                items.push(WordBodyItem::OverlapPoint(kind, overlap_text));
            }
            // Underline markers
            '\u{0002}' => {
                chars.next();
                if let Some(&(_, next_ch)) = chars.peek() {
                    match next_ch {
                        '\u{0001}' => {
                            chars.next();
                            items.push(WordBodyItem::UnderlineBegin);
                        }
                        '\u{0002}' => {
                            chars.next();
                            items.push(WordBodyItem::UnderlineEnd);
                        }
                        // A lone `\u{0002}` is not an underline marker. Keep it
                        // as text rather than consuming it silently: this match
                        // is where "skip for now" cost 768 spurious E357s, and
                        // dropping the odd byte is the same shape one arm over.
                        _ => items.push(WordBodyItem::Text(&body[i..i + ch.len_utf8()])),
                    }
                }
            }
            // CA elements
            _ if let Some(kind) = char_to_ca_element(ch) => {
                chars.next();
                items.push(WordBodyItem::CaElement(kind));
            }
            // CA delimiters
            _ if let Some(kind) = char_to_ca_delimiter(ch) => {
                chars.next();
                items.push(WordBodyItem::CaDelimiter(kind));
            }
            // Text segment: everything else until a special char
            _ => {
                chars.next();
                // Consume all text chars (including '0' in rest position)
                while let Some(&(_, c)) = chars.peek() {
                    if is_body_special_char(c) {
                        break;
                    }
                    chars.next();
                }
                let end = chars.peek().map_or(body.len(), |&(j, _)| j);
                items.push(WordBodyItem::Text(&body[i..end]));
            }
        }
    }
    items
}

/// Characters that break a text segment in word body parsing.
fn is_body_special_char(ch: char) -> bool {
    matches!(
        ch,
        '(' | ':' | '+' | '^' | '~' | '\u{02C8}' | '\u{02CC}' | '\u{0002}'
    ) || char_to_ca_element(ch).is_some()
        || char_to_ca_delimiter(ch).is_some()
        || char_to_overlap_kind(ch).is_some()
}

/// The overlap point a character denotes, or `None` when it denotes none.
fn char_to_overlap_kind(ch: char) -> Option<OverlapKind> {
    match ch {
        '\u{2308}' => Some(OverlapKind::TopBegin),
        '\u{2309}' => Some(OverlapKind::TopEnd),
        '\u{230A}' => Some(OverlapKind::BottomBegin),
        '\u{230B}' => Some(OverlapKind::BottomEnd),
        _ => None,
    }
}

/// The CA element a character denotes, or `None` when it denotes none.
fn char_to_ca_element(ch: char) -> Option<CaElementKind> {
    match ch {
        '\u{2260}' => Some(CaElementKind::BlockedSegments),
        '\u{223E}' => Some(CaElementKind::Constriction),
        '\u{2051}' => Some(CaElementKind::Hardening),
        '\u{2907}' => Some(CaElementKind::HurriedStart),
        '\u{2219}' => Some(CaElementKind::Inhalation),
        '\u{1F29}' => Some(CaElementKind::LaughInWord),
        '\u{2193}' => Some(CaElementKind::PitchDown),
        '\u{21BB}' => Some(CaElementKind::PitchReset),
        '\u{2191}' => Some(CaElementKind::PitchUp),
        '\u{2906}' => Some(CaElementKind::SuddenStop),
        _ => None,
    }
}

/// The CA delimiter a character denotes, or `None` when it denotes none.
fn char_to_ca_delimiter(ch: char) -> Option<CaDelimiterKind> {
    match ch {
        '\u{2047}' => Some(CaDelimiterKind::Unsure),
        '\u{00A7}' => Some(CaDelimiterKind::Precise),
        '\u{204E}' => Some(CaDelimiterKind::Creaky),
        '\u{00B0}' => Some(CaDelimiterKind::Softer),
        '\u{21AB}' => Some(CaDelimiterKind::SegmentRepetition),
        '\u{2206}' => Some(CaDelimiterKind::Faster),
        '\u{2207}' => Some(CaDelimiterKind::Slower),
        '\u{222C}' => Some(CaDelimiterKind::Whisper),
        '\u{222E}' => Some(CaDelimiterKind::Singing),
        '\u{2581}' => Some(CaDelimiterKind::LowPitch),
        '\u{2594}' => Some(CaDelimiterKind::HighPitch),
        '\u{25C9}' => Some(CaDelimiterKind::Louder),
        '\u{263A}' => Some(CaDelimiterKind::SmileVoice),
        '\u{264B}' => Some(CaDelimiterKind::BreathyVoice),
        '\u{03AB}' => Some(CaDelimiterKind::Yawn),
        _ => None,
    }
}
