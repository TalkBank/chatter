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
use talkbank_model::generated::symbol_sets::is_word_segment_rest_char;
use talkbank_model::model::{
    CADelimiterType, CAElementType, EmptyText, WordShortening, WordStressMarkerType, WordText,
};

/// A word body cannot supply the lexical structure required by the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WordBodyError {
    /// A lexical segment has no content.
    #[error(transparent)]
    EmptySegment(#[from] EmptyText),
    /// An opening shortening delimiter has no observed closing delimiter.
    #[error("shortening has no closing parenthesis")]
    UnclosedShortening,
    /// An underline prefix is missing its begin/end discriminator.
    #[error("underline prefix is not followed by a begin or end marker")]
    MalformedUnderline,
}

/// A shortening admitted only after consuming its closing delimiter.
struct ClosedShortening(WordShortening);

impl ClosedShortening {
    /// The scanner has consumed the opening delimiter; retain its continuation.
    fn consume(
        body: &str,
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    ) -> Result<Self, WordBodyError> {
        let &(start, _) = chars.peek().ok_or(WordBodyError::UnclosedShortening)?;
        let (end, _) = chars
            .find(|&(_, ch)| ch == ')')
            .ok_or(WordBodyError::UnclosedShortening)?;
        Ok(Self(WordShortening::new(&body[start..end])?))
    }

    fn into_item<'a>(self) -> WordBodyItem<'a> {
        WordBodyItem::Shortening(self.0)
    }
}

/// Parse a word body string into structured `WordBodyItem` list.
/// The body is the interior of a word (no prefix, no suffixes).
/// Empty segments and unclosed shortenings reject instead of fabricating structure.
///
/// # Errors
/// Returns [`WordBodyError`] when a lexical segment is empty or a shortening
/// has no closing parenthesis, or an underline prefix is malformed. This
/// scanner does not certify whole-word validity.
pub fn parse_word_body(body: &str) -> Result<Vec<WordBodyItem<'_>>, WordBodyError> {
    let mut items = Vec::new();
    let mut chars = body.char_indices().peekable();

    while let Some(&(i, ch)) = chars.peek() {
        if let Some((run, remainder)) = LengtheningRun::take_prefix(&body[i..]) {
            let end = body.len() - remainder.len();
            // Advance the cursor over the admitted extent, without classifying
            // the colons again or pairing the run with independent text.
            while chars.next_if(|&(offset, _)| offset < end).is_some() {}
            items.push(WordBodyItem::Lengthening(run));
            continue;
        }
        if let Some((point, remainder)) = WordOverlapPoint::take_prefix(&body[i..]) {
            let end = body.len() - remainder.len();
            while chars.next_if(|&(offset, _)| offset < end).is_some() {}
            items.push(WordBodyItem::OverlapPoint(point));
            continue;
        }
        match ch {
            // Shortening: (text)
            '(' => {
                chars.next();
                items.push(ClosedShortening::consume(body, &mut chars)?.into_item());
            }
            // Compound marker
            '+' => {
                chars.next();
                items.push(WordBodyItem::CompoundMarker(&body[i..i + ch.len_utf8()]));
            }
            // Stress markers
            '\u{02C8}' => {
                chars.next();
                items.push(WordBodyItem::Stress(
                    WordStressMarkerType::Primary,
                    &body[i..i + ch.len_utf8()],
                ));
            }
            '\u{02CC}' => {
                chars.next();
                items.push(WordBodyItem::Stress(
                    WordStressMarkerType::Secondary,
                    &body[i..i + ch.len_utf8()],
                ));
            }
            // Syllable pause
            '^' => {
                chars.next();
                items.push(WordBodyItem::SyllablePause(&body[i..i + ch.len_utf8()]));
            }
            // Clitic boundary
            '~' => {
                chars.next();
                items.push(WordBodyItem::CliticBoundary(&body[i..i + ch.len_utf8()]));
            }
            // Underline markers
            '\u{0002}' => {
                chars.next();
                // Admit a complete structural marker. A failed marker is not
                // lexical text, even when this scanner is called directly.
                let item = match chars.next() {
                    Some((end, '\u{0001}')) => WordBodyItem::UnderlineBegin(&body[i..end + 1]),
                    Some((end, '\u{0002}')) => WordBodyItem::UnderlineEnd(&body[i..end + 1]),
                    Some(_) | None => return Err(WordBodyError::MalformedUnderline),
                };
                items.push(item);
            }
            // CA elements
            _ if let Some(kind) = CAElementType::from_char(ch) => {
                chars.next();
                items.push(WordBodyItem::CaElement(kind, &body[i..i + ch.len_utf8()]));
            }
            // CA delimiters
            _ if let Some(kind) = CADelimiterType::from_char(ch) => {
                chars.next();
                items.push(WordBodyItem::CaDelimiter(kind, &body[i..i + ch.len_utf8()]));
            }
            // Text segment: everything else until a special char
            _ => {
                chars.next();
                // Consume the registry-owned continuation class. In particular,
                // a backslash starts a new segment rather than extending this one.
                while let Some(&(_, c)) = chars.peek() {
                    if !is_word_segment_rest_char(c) {
                        break;
                    }
                    chars.next();
                }
                let end = chars.peek().map_or(body.len(), |&(j, _)| j);
                items.push(WordBodyItem::Text(WordText::new(&body[i..end])?));
            }
        }
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortening_requires_observed_closing_delimiter() {
        let reference =
            include_str!("../../../../corpus/reference/content/shortenings-in-words.cha");
        let mut checked = 0;
        for word in reference
            .lines()
            .filter(|line| line.starts_with('*'))
            .flat_map(str::split_whitespace)
            .filter(|word| word.contains('(') && word.contains(')'))
        {
            assert!(parse_word_body(word).is_ok(), "legal control: {word}");
            let missing_close = word.replace(')', "");
            assert!(matches!(
                parse_word_body(&missing_close),
                Err(WordBodyError::UnclosedShortening)
            ));
            let trailing_open = format!("{word}(");
            assert!(matches!(
                parse_word_body(&trailing_open),
                Err(WordBodyError::UnclosedShortening)
            ));
            let (prefix, shortening) = word.split_once('(').unwrap();
            let (_, suffix) = shortening.split_once(')').unwrap();
            let empty = format!("{prefix}(){suffix}");
            assert!(matches!(
                parse_word_body(&empty),
                Err(WordBodyError::EmptySegment(EmptyText))
            ));
            checked += 1;
        }
        assert_eq!(checked, 3);
    }

    /// The public scanner refuses malformed structural controls rather than
    /// manufacturing lexical text. This is a scanner-boundary test, not a
    /// claim that the lexer emits malformed rich word tokens.
    #[test]
    fn underline_prefix_requires_its_structural_discriminator() {
        use crate::{lexer::Lexer, token::Token};

        let reference = include_str!("../../../../corpus/reference/content/words-prosody.cha");
        let mut checked = std::collections::BTreeSet::new();
        for (token, _) in Lexer::new(reference, crate::lexer::COND_INITIAL) {
            let Token::Word { body, .. } = token else {
                continue;
            };
            if !body.contains('\u{0002}') {
                continue;
            }
            let admitted = parse_word_body(body).expect("reference word body");
            assert!(admitted.iter().any(|item| matches!(
                item,
                WordBodyItem::UnderlineBegin(_) | WordBodyItem::UnderlineEnd(_)
            )));
            for marker in ["\u{0002}\u{0001}", "\u{0002}\u{0002}"] {
                if !body.contains(marker) {
                    continue;
                }
                let malformed = body.replace(marker, "\u{0002}x");
                assert!(
                    matches!(
                        parse_word_body(&malformed),
                        Err(WordBodyError::MalformedUnderline)
                    ),
                    "{malformed:?}"
                );
                checked.insert(marker);
            }
            let truncated = format!("{body}\u{0002}");
            assert!(matches!(
                parse_word_body(&truncated),
                Err(WordBodyError::MalformedUnderline)
            ));
        }
        assert_eq!(
            checked,
            std::collections::BTreeSet::from(["\u{0002}\u{0001}", "\u{0002}\u{0002}"]),
            "both marker kinds must come from the reference fixture"
        );
    }
}
