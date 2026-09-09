//! Word-level sanitization: replace `WordContent::Text` and `Shortening`
//! segments with deterministic placeholders, preserve all other
//! structural elements verbatim.
//!
//! `WriteChat for Word` ignores `Word.raw_text` and serializes from the
//! typed content exposed by `Word::content()`, so mutation must happen on
//! that structured content, not on `raw_text`. The replacement API also
//! invalidates derived cleaned text, and `raw_text` is rebuilt for downstream
//! JSON consumers (the Serialize impl emits raw_text directly).

use smol_str::SmolStr;
use talkbank_model::non_empty_literal;
use talkbank_model::{Word, WordContent, WordShortening, WordText, WriteChat};

use super::placeholder::{PlaceholderState, PlaceholderToken};

/// Sanitizer decision for one typed word-content leaf.
///
/// A closed enum makes every future [`WordContent`] variant choose explicitly
/// between preservation and replacement. Replacement data travels with the
/// decision, so no separate flag can claim content changed when it did not, or
/// forget that it changed when rebuilding `raw_text` matters for privacy.
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
        rebuild_raw_text(word);
        return;
    }

    let placeholder = PlaceholderToken::word(state.next());
    for i in 0..word.content().len() {
        match redact_content(&word.content()[i], &placeholder) {
            ContentRedaction::Preserve => {}
            ContentRedaction::Replace(replacement) => word.replace_content_at(i, replacement),
        }
    }

    // Always derive the raw JSON field from the sanitized typed structure.
    // Parser recovery can leave untrusted source text beside a
    // structural-only content sequence, where no lexical leaf is available to
    // trip a "modified" flag. Rebuilding closes that leak shape entirely.
    rebuild_raw_text(word);
}

/// Replaces source spelling with a serialization of the current typed state.
fn rebuild_raw_text(word: &mut Word) {
    let mut buffer = String::new();
    let _ = word.write_chat(&mut buffer);
    word.set_raw_text(SmolStr::new(&buffer));
}

#[cfg(test)]
mod tests {
    use talkbank_model::{WordCategory, WordCompoundMarker, WordContents};
    use talkbank_parser::TreeSitterParser;

    use super::*;

    /// Parser recovery can leave source text beside a structural-only content
    /// sequence. Sanitization must derive the raw JSON field from the sanitized
    /// structure even when no lexical leaf happened to be replaced.
    #[test]
    fn structural_only_recovery_cannot_retain_untrusted_raw_text() {
        let mut word = Word::simple("private-name").with_content(WordContents::from(vec![
            WordContent::CompoundMarker(WordCompoundMarker::new()),
        ]));
        let mut state = PlaceholderState::new();

        sanitize_word(&mut word, &mut state);

        assert_eq!(word.raw_text(), "+");
    }

    /// The semantic `xxx` pass-through decision is based on typed content, not
    /// on the source spelling cached in `raw_text`. Even this early-return path
    /// must therefore discard an inconsistent recovery spelling before JSON
    /// serialization can expose it.
    ///
    /// FABRICATED ON PURPOSE, and one of the few that should stay so. Its
    /// subject IS the inconsistent state, a `raw_text` of "private-name" over
    /// content that says `xxx`, which the parser will not produce for any
    /// input: no CHAT word both reads as untranscribed and carries that
    /// spelling. Converting it to a parse would delete the case rather than
    /// strengthen it. This is the "behaviour of a function that a signature
    /// cannot describe" category the standards name, and the value being
    /// fabricated is exactly the hazard the redaction must survive.
    #[test]
    fn untranscribed_pass_through_cannot_retain_untrusted_raw_text() {
        let mut word =
            Word::new_unchecked("private-name", "xxx").with_content(WordContents::from(vec![
                WordContent::Text(WordText::from(non_empty_literal!("xxx"))),
            ]));
        let mut state = PlaceholderState::new();

        sanitize_word(&mut word, &mut state);

        assert_eq!(word.raw_text(), "xxx");
    }

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

    /// Rebuilding the JSON raw spelling must serialize the complete typed
    /// word, not just its lexical leaves, or privacy redaction would silently
    /// discard category and suffix markers from that representation.
    #[test]
    fn raw_text_rebuild_preserves_nonlexical_word_markers() {
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
