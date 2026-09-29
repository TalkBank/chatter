//! Word-level sanitization: replace `WordContent::Text` and `Shortening`
//! segments with deterministic placeholders, preserve all other
//! structural elements verbatim.
//!
//! Word spelling and the JSON `raw_text` field derive from typed content.
//! Mutation therefore changes structure only; the replacement API invalidates
//! cached cleaned text without a separate raw-spelling synchronization step.

use talkbank_model::non_empty_literal;
use talkbank_model::{Word, WordContent, WordShortening, WordText};

use super::placeholder::{PlaceholderState, PlaceholderToken};

/// Sanitizer decision for one typed word-content leaf.
///
/// A closed enum makes every future [`WordContent`] variant choose explicitly
/// between preservation and replacement. Replacement data travels with the
/// decision, so no separate flag can claim content changed when it did not, or
/// forget to apply a replacement that matters for privacy.
enum ContentRedaction {
    Preserve,
    Replace(WordContent),
}

fn redact_content(content: &WordContent, placeholder: &PlaceholderToken) -> ContentRedaction {
    match content {
        WordContent::Text(_) => ContentRedaction::Replace(WordContent::Text(WordText::from(
            placeholder.text().clone(),
        ))),
        // A @u phonetic form is SPOKEN content: it can encode a name
        // phonetically, so it must be redacted like text, never preserved as a
        // structural marker.
        WordContent::Phonetic(_) => ContentRedaction::Replace(WordContent::Phonetic(
            talkbank_model::WordPhonetic::from(placeholder.text().clone()),
        )),
        WordContent::Shortening(_) => ContentRedaction::Replace(WordContent::Shortening(
            WordShortening::from(non_empty_literal!("x")),
        )),
        // Structural / prosodic markers, preserved verbatim. Listed
        // explicitly (not `_ => {}`) so a new WordContent variant fails to
        // compile here, forcing an explicit redact-vs-preserve decision for
        // any future leaf type.
        WordContent::OverlapPoint(_)
        | WordContent::CAElement(_)
        | WordContent::CADelimiter(_)
        | WordContent::StressMarker(_)
        | WordContent::Lengthening(_)
        | WordContent::SyllablePause(_)
        | WordContent::UnderlineBegin(_)
        | WordContent::UnderlineEnd(_)
        | WordContent::CompoundMarker(_)
        | WordContent::CliticBoundary(_) => ContentRedaction::Preserve,
    }
}

/// Sanitizes a single `Word` in place.
///
/// Untranscribed markers (`xxx`/`yyy`/`www`) are passed through
/// unchanged, replacing them changes their semantic meaning.
pub(crate) fn sanitize_word(word: &mut Word, state: &mut PlaceholderState) {
    if word.untranscribed().is_some() {
        return;
    }

    let placeholder = PlaceholderToken::word(state.next());
    for i in 0..word.content().len() {
        match redact_content(&word.content()[i], &placeholder) {
            ContentRedaction::Preserve => {}
            ContentRedaction::Replace(replacement) => word.replace_content_at(i, replacement),
        }
    }
}

#[cfg(test)]
mod tests {
    use talkbank_model::WordCategory;
    use talkbank_parser::TreeSitterParser;

    use super::*;

    /// `untranscribed()` reads and caches cleaned text before redaction. The
    /// cache must not preserve the original lexical value after typed content
    /// is replaced.
    #[test]
    fn lexical_redaction_cannot_retain_untrusted_cleaned_text() {
        let mut word = Word::simple("private-name");
        let mut state = PlaceholderState::new();

        sanitize_word(&mut word, &mut state);

        assert_eq!(word.raw_text(), "w1");
        assert_eq!(word.cleaned_text(), "w1");
    }

    /// Derived JSON spelling must serialize the complete typed
    /// word, not just its lexical leaves, or privacy redaction would silently
    /// discard category and suffix markers from that representation.
    #[test]
    fn derived_spelling_preserves_nonlexical_word_markers() {
        // PARSED, not fabricated. The pair `("&-private-name", "private-name")`
        // stated the input twice with nothing forcing the second to be what
        // cleaning the first produces, and `.with_category(Filler)` restated
        // in Rust what the `&-` prefix already says in CHAT. The parse carries
        // all three, so this test now asserts against a word the toolchain can
        // actually make.
        let parser = TreeSitterParser::new().expect("the grammar is linked in");
        let mut word = parser
            .parse_word("&-private-name")
            .expect("`&-private-name` is a word this grammar admits");
        assert_eq!(
            word.category,
            Some(WordCategory::Filler),
            "the `&-` prefix is what makes this a filler; the fabrication used \
             to assert it by hand"
        );
        let mut state = PlaceholderState::new();

        sanitize_word(&mut word, &mut state);

        assert_eq!(word.raw_text(), "&-w1");
    }
}
