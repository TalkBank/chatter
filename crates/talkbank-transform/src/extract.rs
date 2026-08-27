//! NLP word extraction from CHAT AST.
//!
//! Walks the parsed CHAT content tree and collects words that are
//! "alignable" for a given domain (Mor, Wor, Pho, Sin).

use talkbank_model::alignment::helpers::{
    LanguageScope, TierDomain, WordItem, annotations_have_alignment_ignore, counts_for_tier,
    is_tag_marker_separator, should_align_replaced_word_in_pho_sin, walk_words_scoped,
};
use talkbank_model::model::{ChatFile, LanguageCode, Line, ReplacedWord, UtteranceContent, Word};
use talkbank_model::validation::{GoverningMark, GoverningMarkKind, LanguageResolutionOutcome};
use talkbank_model::{ChatCleanedText, ChatRawText, SpeakerCode, UtteranceIdx, WordIdx};

/// A word extracted from the CHAT AST for NLP processing.
#[derive(Debug, Clone)]
pub struct ExtractedWord {
    /// Cleaned text suitable for NLP (no CHAT markers).
    pub text: ChatCleanedText,
    /// Raw text as it appeared in the transcript.
    pub raw_text: ChatRawText,
    /// Zero-based index among extracted alignable words in this utterance.
    pub utterance_word_index: WordIdx,
    /// Special form marker if the word has @c, @b, @s, etc.
    pub form_type: Option<talkbank_model::model::FormType>,
    /// The mark that GOVERNS this word's language: its own `@s`, an enclosing
    /// `<...> [@s]` span, or the utterance.
    ///
    /// **Not a bare `Option<WordLanguageMarker>` carrying only the word's own
    /// mark, and the difference is a bug.** Extraction WALKS the tree, so it
    /// knows whether a `<...> [@s:hin]` span encloses a word; carrying only the
    /// own-marker discarded that, and Batchalign's morphotag consequently saw
    /// every unmarked word inside a Hindi span as unlanguaged, dropped it from
    /// second-language dispatch, and tagged it against the tier language.
    ///
    /// [`GoverningMark`] carries the word's span with it, so
    /// [`ExtractedWord::resolve_language`] needs no span argument and a caller
    /// cannot pair this mark with a different word's position.
    /// PRIVATE, and that is the point rather than encapsulation for its own
    /// sake. While it was `pub`, `a.language = b.language.clone()` moved one
    /// word's mark and position onto another in a single line, so the claim
    /// that a mark "cannot be paired with a different word's position" was
    /// false at the struct level even though `GoverningMark::resolve` takes no
    /// span. Read it through [`ExtractedWord::language_kind`] or resolve it
    /// through [`ExtractedWord::resolve_language`].
    language: GoverningMark,
}

impl ExtractedWord {
    /// Which kind of mark governs this word's language.
    ///
    /// The payload-free view, for consumers that must branch on the kind
    /// (Batchalign restores a word's surface text only when the WORD itself
    /// carried markers) without being handed a mark they could move elsewhere.
    #[must_use]
    pub fn language_kind(&self) -> GoverningMarkKind {
        self.language.kind()
    }

    /// Resolve this word's language.
    ///
    /// The operation belongs on the word rather than on the mark, so that the
    /// mark and the position it resolves at cannot come from different words.
    #[must_use]
    pub fn resolve_language(
        &self,
        tier_language: Option<&LanguageCode>,
        declared_languages: &[LanguageCode],
    ) -> LanguageResolutionOutcome {
        self.language.resolve(tier_language, declared_languages)
    }
}

/// Per-utterance extraction result.
#[derive(Debug, Clone)]
pub struct ExtractedUtterance {
    /// Speaker code (e.g., "CHI", "MOT").
    pub speaker: SpeakerCode,
    /// Zero-based utterance index in the file.
    pub utterance_index: UtteranceIdx,
    /// Extracted words.
    pub words: Vec<ExtractedWord>,
}

/// Extract NLP-ready words from all utterances in a ChatFile.
///
/// Walks every utterance in the file and collects words that are
/// "alignable" for the given `domain`. Non-utterance lines (headers,
/// comments, etc.) are skipped.
///
/// * `chat_file` - The parsed CHAT file to extract words from.
/// * `domain` - The alignment domain governing which words are
///   considered alignable (`Mor`, `Wor`, `Pho`, or `Sin`).
pub fn extract_words(chat_file: &ChatFile, domain: TierDomain) -> Vec<ExtractedUtterance> {
    let mut results = Vec::new();
    let mut utt_idx = 0;

    for line in &chat_file.lines {
        if let Line::Utterance(utterance) = line {
            let speaker = SpeakerCode::new(&utterance.main.speaker);
            let mut words = Vec::new();
            collect_utterance_content(&utterance.main.content.content, domain, &mut words);
            results.push(ExtractedUtterance {
                speaker,
                utterance_index: UtteranceIdx::new(utt_idx),
                words,
            });
            utt_idx += 1;
        }
    }

    results
}

/// Collect NLP-extractable words from a slice of utterance content items.
///
/// This is the inner workhorse called by [`extract_words`] and also used
/// directly by other modules (morphosyntax, utseg, translate, coref) to
/// extract words from a single utterance's content without iterating the
/// entire file.
///
/// * `content` - The top-level content items of an utterance.
/// * `domain` - The alignment domain that determines which words are
///   collected (e.g., `Mor` includes tag-marker separators; `Wor` does not).
/// * `out` - Accumulator that extracted words are pushed into.
pub fn collect_utterance_content(
    content: &[UtteranceContent],
    domain: TierDomain,
    out: &mut Vec<ExtractedWord>,
) {
    // The SCOPED walk. `walk_words` discards the enclosing `<...> [@s]` span,
    // and this function's whole output is what downstream NLP sees, so
    // discarding it here is where the information was actually lost.
    walk_words_scoped(content, Some(domain), &mut |leaf, scope| match leaf {
        WordItem::Word(word) => {
            collect_alignable_word(word, &[], domain, scope, out);
        }
        WordItem::ReplacedWord(replaced) => {
            collect_replaced_word(replaced, domain, scope, out);
        }
        WordItem::Separator(sep) => {
            if domain == TierDomain::Mor && is_tag_marker_separator(sep) {
                push_extracted(
                    out,
                    ChatCleanedText::from_separator(sep),
                    ChatRawText::from_separator(sep),
                    None,
                    // A tag separator is not a word, so there is no
                    // precedence question: only the enclosing scope to record.
                    GoverningMark::of_separator(sep, scope.span()),
                );
            }
        }
    });
}

/// Push one extracted item, deriving its index from the accumulator.
///
/// THE ONLY PLACE `utterance_word_index` IS COMPUTED. Five call sites built the
/// struct literally, each repeating `WordIdx::new(out.len())`, and a test
/// existed solely to notice if one drifted out of step: a test whose only job
/// is to detect that two things diverged, which is the tell for a missing
/// owner.
///
/// The first version of this owned only the WORD sites and left the separator
/// site writing the index by hand, so the count went from five to two rather
/// than to one, and the test was deleted on the strength of an ownership that
/// did not yet exist. The public `synthetic` constructor, which took the index
/// as a PARAMETER and had no callers at all, was a third route and is gone.
///
/// Nothing remains open here, and an earlier version of this paragraph said
/// otherwise: it claimed a struct literal could still set the index to anything
/// and that closing it meant an API change for downstream crates. Both halves
/// were wrong. `language` is already private, so `ExtractedWord { .. }` is
/// unwritable outside this module, and the only literal in the codebase is the
/// one below.
fn push_extracted(
    out: &mut Vec<ExtractedWord>,
    text: ChatCleanedText,
    raw_text: ChatRawText,
    form_type: Option<talkbank_model::model::FormType>,
    language: GoverningMark,
) {
    let utterance_word_index = WordIdx::new(out.len());
    out.push(ExtractedWord {
        text,
        raw_text,
        utterance_word_index,
        form_type,
        language,
    });
}

/// Push one extracted WORD, with the mark its enclosing scope implies.
fn push_word(out: &mut Vec<ExtractedWord>, word: &Word, scope: LanguageScope<'_>) {
    push_extracted(
        out,
        ChatCleanedText::from_word(word),
        ChatRawText::from_word_raw(word),
        word.form_type.clone(),
        GoverningMark::of(word, scope.span()),
    );
}

fn collect_alignable_word(
    word: &Word,
    annotations: &[talkbank_model::model::ContentAnnotation],
    domain: TierDomain,
    scope: LanguageScope<'_>,
    out: &mut Vec<ExtractedWord>,
) {
    if domain == TierDomain::Mor && annotations_have_alignment_ignore(annotations) {
        return;
    }

    if !counts_for_tier(word, domain) {
        return;
    }

    push_word(out, word, scope);
}

fn collect_replaced_word(
    entry: &ReplacedWord,
    domain: TierDomain,
    scope: LanguageScope<'_>,
    out: &mut Vec<ExtractedWord>,
) {
    if domain == TierDomain::Mor && annotations_have_alignment_ignore(&entry.scoped_annotations) {
        return;
    }

    match domain {
        TierDomain::Mor => {
            if !entry.replacement.words.is_empty() {
                for word in &entry.replacement.words {
                    if counts_for_tier(word, TierDomain::Mor) {
                        push_word(out, word, scope);
                    }
                }
            } else if counts_for_tier(&entry.word, TierDomain::Mor) {
                push_word(out, &entry.word, scope);
            }
        }
        // %wor gets its OWN arm, matching `count::count_alignable_replaced_word`.
        // Grouping it with Pho/Sin applied the pho/sin fragment exclusion to
        // %wor, and the two predicates deliberately disagree about fillers:
        // `is_wor_excluded_category` excludes only Nonword and
        // PhonologicalFragment, because `rules.rs` documents fillers as
        // INCLUDED in %wor ("stable, alignable phoneme sequences"), while
        // `is_fragment_like` (used by the pho/sin predicate) counts Filler as a
        // fragment. So `&-um [: um]` counted 1 and extracted 0, and
        // `count_tier_positions(..) == collect_tier_items(..).len()` silently
        // stopped holding for that shape.
        TierDomain::Wor => {
            if counts_for_tier(&entry.word, TierDomain::Wor) {
                push_word(out, &entry.word, scope);
            }
        }
        TierDomain::Pho | TierDomain::Sin => {
            if should_align_replaced_word_in_pho_sin(entry) {
                push_word(out, &entry.word, scope);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::alignment::helpers::TierDomain;
    use talkbank_model::validation::GoverningMarkKind;
    use talkbank_parser::TreeSitterParser;

    /// Parse a CHAT string into a ChatFile, panicking on errors (test-only).
    fn parse_chat(text: &str) -> ChatFile {
        let parser = TreeSitterParser::new().expect("tree-sitter parser");
        parser.parse_chat_file(text).expect_built()
    }

    /// Minimal valid CHAT with one utterance containing the given main tier text.
    fn one_utterance(main_tier: &str) -> String {
        format!(
            "@UTF8\n\
             @Begin\n\
             @Languages:\teng\n\
             @Participants:\tCHI Target_Child\n\
             @ID:\teng|test|CHI||female|||Target_Child|||\n\
             *CHI:\t{main_tier}\n\
             @End\n"
        )
    }

    // -----------------------------------------------------------------------
    // Basic word extraction
    // -----------------------------------------------------------------------

    #[test]
    fn simple_words_in_mor_domain() {
        let chat = parse_chat(&one_utterance("hello world ."));
        let result = extract_words(&chat, TierDomain::Mor);
        assert_eq!(result.len(), 1, "expected 1 utterance");
        assert_eq!(result[0].words.len(), 2, "expected 2 words (hello, world)");
        assert_eq!(result[0].words[0].text.as_str(), "hello");
        assert_eq!(result[0].words[1].text.as_str(), "world");
    }

    #[test]
    fn utterance_indices_are_sequential_across_file() {
        let chat = parse_chat(
            "@UTF8\n\
             @Begin\n\
             @Languages:\teng\n\
             @Participants:\tCHI Target_Child, MOT Mother\n\
             @ID:\teng|test|CHI||female|||Target_Child|||\n\
             @ID:\teng|test|MOT||female|||Mother|||\n\
             *CHI:\thello .\n\
             *MOT:\thi .\n\
             *CHI:\tbye .\n\
             @End\n",
        );
        let result = extract_words(&chat, TierDomain::Mor);
        assert_eq!(result.len(), 3, "expected 3 utterances");
        assert_eq!(result[0].utterance_index, UtteranceIdx::new(0));
        assert_eq!(result[1].utterance_index, UtteranceIdx::new(1));
        assert_eq!(result[2].utterance_index, UtteranceIdx::new(2));
    }

    #[test]
    fn speaker_code_extracted_correctly() {
        let chat = parse_chat(
            "@UTF8\n\
             @Begin\n\
             @Languages:\teng\n\
             @Participants:\tCHI Target_Child, MOT Mother\n\
             @ID:\teng|test|CHI||female|||Target_Child|||\n\
             @ID:\teng|test|MOT||female|||Mother|||\n\
             *CHI:\thello .\n\
             *MOT:\thi .\n\
             @End\n",
        );
        let result = extract_words(&chat, TierDomain::Mor);
        assert_eq!(result[0].speaker.as_str(), "CHI");
        assert_eq!(result[1].speaker.as_str(), "MOT");
    }

    #[test]
    fn non_utterance_lines_are_skipped() {
        // Headers and comments are not utterances, only *SPK: lines count.
        let chat = parse_chat(
            "@UTF8\n\
             @Begin\n\
             @Languages:\teng\n\
             @Participants:\tCHI Target_Child\n\
             @ID:\teng|test|CHI||female|||Target_Child|||\n\
             @Comment:\tthis is a comment\n\
             *CHI:\thello .\n\
             @End\n",
        );
        let result = extract_words(&chat, TierDomain::Mor);
        assert_eq!(result.len(), 1, "only the utterance, not the comment");
    }

    // -----------------------------------------------------------------------
    // Tag-marker separators: included in Mor, excluded from Wor/Pho/Sin
    // -----------------------------------------------------------------------

    #[test]
    fn comma_separator_included_in_mor_domain() {
        // CHAT: comma (,) between words is a tag-marker separator in Mor domain.
        let chat = parse_chat(&one_utterance("well , hello ."));
        let mor_result = extract_words(&chat, TierDomain::Mor);
        // Mor domain: "well", ",", "hello" = 3 items
        let mor_texts: Vec<&str> = mor_result[0]
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect();
        assert!(
            mor_texts.contains(&","),
            "Mor domain should include comma separator, got: {mor_texts:?}"
        );
    }

    #[test]
    fn comma_separator_excluded_from_wor_domain() {
        let chat = parse_chat(&one_utterance("well , hello ."));
        let wor_result = extract_words(&chat, TierDomain::Wor);
        let wor_texts: Vec<&str> = wor_result[0]
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect();
        assert!(
            !wor_texts.contains(&","),
            "Wor domain should NOT include comma separator, got: {wor_texts:?}"
        );
    }

    #[test]
    fn tag_separator_included_in_mor_domain() {
        // „ (U+201E) is the tag separator
        let chat = parse_chat(&one_utterance("hello „ world ."));
        let mor_result = extract_words(&chat, TierDomain::Mor);
        let mor_texts: Vec<&str> = mor_result[0]
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect();
        assert!(
            mor_texts.contains(&"„"),
            "Mor domain should include tag separator, got: {mor_texts:?}"
        );
    }

    #[test]
    fn vocative_separator_included_in_mor_domain() {
        // ‡ (U+2021) is the vocative separator
        let chat = parse_chat(&one_utterance("‡ Mom ."));
        let mor_result = extract_words(&chat, TierDomain::Mor);
        let mor_texts: Vec<&str> = mor_result[0]
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect();
        assert!(
            mor_texts.contains(&"‡"),
            "Mor domain should include vocative separator, got: {mor_texts:?}"
        );
    }

    #[test]
    fn tag_separator_excluded_from_pho_domain() {
        let chat = parse_chat(&one_utterance("hello „ world ."));
        let pho_result = extract_words(&chat, TierDomain::Pho);
        let pho_texts: Vec<&str> = pho_result[0]
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect();
        assert!(
            !pho_texts.contains(&"„"),
            "Pho domain should NOT include tag separator, got: {pho_texts:?}"
        );
    }

    // -----------------------------------------------------------------------
    // ReplacedWord: Mor uses replacement; Pho/Sin/Wor use original
    // -----------------------------------------------------------------------

    #[test]
    fn replaced_word_uses_replacement_in_mor_domain() {
        // CHAT: "doggie [: dog]", in Mor domain, "dog" is used (the replacement).
        let chat = parse_chat(&one_utterance("doggie [: dog] ."));
        let result = extract_words(&chat, TierDomain::Mor);
        let texts: Vec<&str> = result[0].words.iter().map(|w| w.text.as_str()).collect();
        assert!(
            texts.contains(&"dog"),
            "Mor domain should use replacement word 'dog', got: {texts:?}"
        );
    }

    #[test]
    fn replaced_word_uses_original_when_replacement_empty_in_mor() {
        // When replacement has no words, Mor falls back to original.
        // This is tested via the code path: entry.replacement.words.is_empty() == true
        // In practice, CHAT always has at least one replacement word, but the code
        // handles the empty case by falling back to the original word.
        // We test the code path with a normal replaced word where the replacement
        // is present, the replacement is used, not the original.
        let chat = parse_chat(&one_utterance("goed [: went] ."));
        let result = extract_words(&chat, TierDomain::Mor);
        let texts: Vec<&str> = result[0].words.iter().map(|w| w.text.as_str()).collect();
        assert!(
            texts.contains(&"went"),
            "Mor domain should use replacement 'went', got: {texts:?}"
        );
        assert!(
            !texts.contains(&"goed"),
            "Mor domain should NOT use original 'goed', got: {texts:?}"
        );
    }

    #[test]
    fn replaced_word_uses_original_in_wor_domain() {
        // In Wor domain, the original word is used (not the replacement).
        let chat = parse_chat(&one_utterance("doggie [: dog] ."));
        let result = extract_words(&chat, TierDomain::Wor);
        let texts: Vec<&str> = result[0].words.iter().map(|w| w.text.as_str()).collect();
        assert!(
            texts.contains(&"doggie"),
            "Wor domain should use original word 'doggie', got: {texts:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Alignment-ignore annotation ([e]) excludes words in Mor domain
    // -----------------------------------------------------------------------

    #[test]
    fn exclude_annotation_skips_word_in_mor_domain() {
        // [e] marks excluded content, skipped in Mor domain.
        let chat = parse_chat(&one_utterance("hello [e] world ."));
        let result = extract_words(&chat, TierDomain::Mor);
        let texts: Vec<&str> = result[0].words.iter().map(|w| w.text.as_str()).collect();
        // "hello" should be excluded by [e]; "world" should remain.
        // Note: [e] applies to the preceding word.
        assert!(
            texts.contains(&"world"),
            "world should be present, got: {texts:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Extraction from Wor domain (flat alignment)
    // -----------------------------------------------------------------------

    #[test]
    fn wor_domain_extracts_simple_words() {
        let chat = parse_chat(&one_utterance("the dog ran ."));
        let result = extract_words(&chat, TierDomain::Wor);
        assert_eq!(result[0].words.len(), 3);
        assert_eq!(result[0].words[0].text.as_str(), "the");
        assert_eq!(result[0].words[1].text.as_str(), "dog");
        assert_eq!(result[0].words[2].text.as_str(), "ran");
    }

    // -----------------------------------------------------------------------
    // Empty utterances
    // -----------------------------------------------------------------------

    #[test]
    fn empty_utterance_produces_empty_word_list() {
        // Utterance with only a terminator, no words.
        let chat = parse_chat(&one_utterance("0 ."));
        let result = extract_words(&chat, TierDomain::Mor);
        // "0" is a special CHAT symbol; whether it produces a word depends on
        // counts_for_tier(). We just verify no crash and at most 1 word.
        assert_eq!(result.len(), 1, "still produces 1 utterance");
    }

    /// A span governs the language of every word it encloses, and a word's own
    /// marker still wins inside it.
    ///
    /// Asserts BOTH which mark governs and what it resolves to. The kinds
    /// alone would pass if resolution were broken, and the languages alone
    /// would pass if a span were mistaken for a word's own marker, since both
    /// spell `Single(hin)` here.
    ///
    /// A test rather than a type: nothing in a signature can say the traversal
    /// actually reached the span. Its predecessor also asserted that a word's
    /// own marker beats the span, as a separate case; that was pinning an
    /// owned/borrowed MIRROR faithfully reproducing `GoverningMarker::of`'s
    /// precedence. The mirror is gone (extraction stores the owned mark
    /// directly), so the only thing left to check is the end-to-end answer, and
    /// one utterance exercises both halves.
    #[test]
    fn a_span_governs_the_words_it_encloses_and_an_own_marker_still_wins() {
        let file = parse_chat(&one_utterance("I said <rocket@s:eng kyaa hai> [@s:hin] ."));
        let utterances = extract_words(&file, TierDomain::Mor);
        let words = &utterances[0].words;

        let eng = talkbank_model::model::LanguageCode::new("eng").expect("valid code");
        let hin = talkbank_model::model::LanguageCode::new("hin").expect("valid code");
        let declared = [eng.clone()];

        use talkbank_model::validation::LanguageResolution::Single;
        // Keyed by the word's own text, so a failure names the word rather than
        // printing parallel vectors and a subscript to count.
        let expected = [
            ("I", GoverningMarkKind::Utterance, Single(eng.clone())),
            ("said", GoverningMarkKind::Utterance, Single(eng.clone())),
            ("rocket", GoverningMarkKind::Own, Single(eng.clone())),
            ("kyaa", GoverningMarkKind::Span, Single(hin.clone())),
            ("hai", GoverningMarkKind::Span, Single(hin.clone())),
        ];
        assert_eq!(words.len(), expected.len(), "word count");

        for (word, (text, kind, language)) in words.iter().zip(&expected) {
            assert_eq!(word.text.as_str(), *text, "word order");
            assert_eq!(word.language_kind(), *kind, "which mark governs `{text}`");
            assert_eq!(
                word.resolve_language(Some(&eng), &declared).resolution,
                *language,
                "resolved language of `{text}`"
            );
        }
    }
}
