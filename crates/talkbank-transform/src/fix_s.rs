//! Rewrite whole-utterance `@s` runs into utterance-level `[- LANG]` precodes.

use talkbank_model::alignment::helpers::{
    WordItem, WordItemMut, walk_code_switch_spans, walk_words, walk_words_mut,
};
use talkbank_model::model::{
    ChatFile, CodeSwitchSpan, Header, LanguageCode, Line, MainTier, ReplacedWord,
    UnspannedSwitchTarget, Word, WordLanguageMarker,
};

/// Rewrite summary for one CHAT file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FixSRewriteStats {
    /// Number of utterances rewritten from per-word `@s` markers to `[- LANG]`.
    pub rewritten_utterances: usize,
    /// Number of language codes appended to `@Languages` from explicit `@s:LANG`.
    pub appended_language_codes: usize,
}

impl FixSRewriteStats {
    /// Returns `true` when no utterance rewrite was needed.
    pub fn is_empty(self) -> bool {
        self.rewritten_utterances == 0 && self.appended_language_codes == 0
    }
}

/// Rewrite whole-utterance language switches in one CHAT file in place.
///
/// Uses the same `%mor`-bearing detection semantics as E255:
/// if all lexical content in an utterance resolves to one language override,
/// set the utterance precode and clear per-word markers that resolve to that
/// same target language. Also appends any explicit `@s:LANG` codes missing from
/// the file's `@Languages` header.
pub fn rewrite_whole_utterance_language_switches(chat_file: &mut ChatFile) -> FixSRewriteStats {
    let appended_language_codes = append_missing_explicit_language_declarations(chat_file);
    let default_language = chat_file.languages.first().cloned();
    let declared_languages = chat_file.languages.iter().cloned().collect::<Vec<_>>();
    let mut stats = FixSRewriteStats {
        appended_language_codes,
        ..FixSRewriteStats::default()
    };

    for line in &mut chat_file.lines {
        let Line::Utterance(utterance) = line else {
            continue;
        };

        if rewrite_main_tier_language_switch(
            &mut utterance.main,
            default_language.as_ref(),
            &declared_languages,
        ) {
            stats.rewritten_utterances += 1;
        }
    }

    stats
}

fn append_missing_explicit_language_declarations(chat_file: &mut ChatFile) -> usize {
    let missing = collect_missing_explicit_language_codes(chat_file);
    if missing.is_empty() {
        return 0;
    }

    let Some(header_idx) = chat_file.lines.iter().rposition(|line| {
        matches!(
            line,
            Line::Header { header, .. } if matches!(header.as_ref(), Header::Languages { .. })
        )
    }) else {
        return 0;
    };

    chat_file.languages.extend(missing.iter().cloned());
    if let Line::Header { header, .. } = &mut chat_file.lines.as_mut_slice()[header_idx]
        && let Header::Languages { codes } = header.as_mut()
    {
        codes.extend(missing.iter().cloned());
    }

    missing.len()
}

fn collect_missing_explicit_language_codes(chat_file: &ChatFile) -> Vec<LanguageCode> {
    let mut known = chat_file.languages.iter().cloned().collect::<Vec<_>>();
    let mut missing = Vec::new();

    for line in &chat_file.lines {
        let Line::Utterance(utterance) = line else {
            continue;
        };
        collect_missing_explicit_languages_from_main_tier(
            &utterance.main,
            &mut known,
            &mut missing,
        );
    }

    missing
}

fn collect_missing_explicit_languages_from_main_tier(
    main_tier: &MainTier,
    known: &mut Vec<LanguageCode>,
    missing: &mut Vec<LanguageCode>,
) {
    // TWO QUESTIONS, TWO WALKS, and they are genuinely different questions.
    //
    // An explicit code can be named by a `<...> [@s:eng]` span as well as by a
    // word's own `@s:eng`, and both mean the same thing about the transcript,
    // so both must reach the header. Reading only `word.lang` declared `eng`
    // for `how@s:eng to@s:eng` and not for `<how to> [@s:eng]`, which made the
    // header depend on the transcriber's choice of notation.
    //
    // Taking the SPAN half off the word walk's scope argument fixed that case
    // and left a subtler one, because a scope is only delivered when a word is
    // delivered. A span is then reported once per word it encloses (redundant),
    // and a span enclosing no word at all is reported NEVER (wrong). Measured:
    // `*CHI: I said <how to> [//] [@s:fra] this .` yields no `%mor` word leaves,
    // since the domain filter skips a reformulation group, so `fra` never
    // reached the header while a `deu` from a plain word marker did. The same
    // notation-dependence the comment above says was fixed, one level down.
    //
    // So the spans come from a walk over SPANS, which is not domain-filtered:
    // which languages a tier NAMES is a fact about the transcript, and a French
    // reformulation is French whether or not it is morphologically analysed.
    walk_code_switch_spans(&main_tier.content.content, &mut |span| {
        if let CodeSwitchSpan::Explicit(code) = span {
            record_missing_code(code, known, missing);
        }
    });

    // The WORD half is NOT domain-filtered either, for the same reason. It was,
    // and that left the invariant above false with the sign flipped: measured,
    // `*CHI: I said <how@s:fra to@s:fra> [//] this .` is valid CHAT carrying two
    // explicit `fra` markers and declared NOTHING, while the span spelling of
    // the same content declared `fra`. Filtering one half and not the other is
    // how a fix for notation-dependence reintroduces it one notation over.
    //
    // OUTPUT CHANGE, stated because it is one: codes named only inside
    // retraced or reformulated material now reach `@Languages`. That is the
    // intended direction (the header should say which languages the transcript
    // contains), but it is a change in what `fix-s` writes.
    walk_words(&main_tier.content.content, None, &mut |item| match item {
        WordItem::Word(word) => record_missing_explicit_language(word, known, missing),
        WordItem::ReplacedWord(replaced) => {
            record_missing_explicit_languages_in_replaced_word(replaced, known, missing);
        }
        WordItem::Separator(_) => {}
    });
}

fn record_missing_explicit_languages_in_replaced_word(
    replaced: &ReplacedWord,
    known: &mut Vec<LanguageCode>,
    missing: &mut Vec<LanguageCode>,
) {
    record_missing_explicit_language(&replaced.word, known, missing);
    for word in &replaced.replacement.words {
        record_missing_explicit_language(word, known, missing);
    }
}

fn record_missing_explicit_language(
    word: &Word,
    known: &mut Vec<LanguageCode>,
    missing: &mut Vec<LanguageCode>,
) {
    let Some(WordLanguageMarker::Explicit(code)) = word.lang.as_ref() else {
        return;
    };
    record_missing_code(code, known, missing);
}

/// Record one explicit code, whatever notation named it.
///
/// Split out so a word's `@s:code` and a span's `[@s:code]` reach the header by
/// the same route; they are the same claim about the transcript.
fn record_missing_code(
    code: &LanguageCode,
    known: &mut Vec<LanguageCode>,
    missing: &mut Vec<LanguageCode>,
) {
    if known.contains(code) {
        return;
    }

    known.push(code.clone());
    missing.push(code.clone());
}

fn rewrite_main_tier_language_switch(
    main_tier: &mut MainTier,
    default_language: Option<&LanguageCode>,
    declared_languages: &[LanguageCode],
) -> bool {
    let Some(target) =
        main_tier.whole_utterance_language_switch_target(default_language, declared_languages)
    else {
        return false;
    };

    let original_tier_language = main_tier
        .content
        .language_code
        .as_ref()
        .or(default_language)
        .cloned();
    let mut cleared_any_word_marker = false;

    // Walk EVERY main-tier word, regular words AND fillers (`&~`,
    // `&-`, `&+`) AND nonwords. Domain-filtering to MOR here would
    // skip fillers, leaving any `@s` shortcut on a filler in place;
    // that shortcut would then resolve against the new tier-language
    // (set by the precode below) and FLIP its meaning. The predicate
    // has already verified that every word's `@s` resolves to the target and
    // that NO word is span-governed, which is what `UnspannedSwitchTarget`
    // carries; so it is safe, and necessary, to clear every `@s` marker that
    // resolves there.
    walk_words_mut(
        main_tier.content.content.as_mut_slice(),
        None,
        &mut |item| match item {
            WordItemMut::Word(word) => {
                cleared_any_word_marker |= clear_matching_word_language_marker(
                    word,
                    original_tier_language.as_ref(),
                    declared_languages,
                    &target,
                );
            }
            WordItemMut::ReplacedWord(replaced) => {
                cleared_any_word_marker |= clear_matching_replaced_word_language_markers(
                    replaced,
                    original_tier_language.as_ref(),
                    declared_languages,
                    &target,
                );
            }
            WordItemMut::Separator(_) => {}
        },
    );

    let language_changed = main_tier.content.language_code.as_ref() != Some(target.language());
    if language_changed {
        main_tier.content.language_code = Some(target.language().clone());
    }

    language_changed || cleared_any_word_marker
}

fn clear_matching_word_language_marker(
    word: &mut Word,
    tier_language: Option<&LanguageCode>,
    declared_languages: &[LanguageCode],
    target: &UnspannedSwitchTarget,
) -> bool {
    let Some(_) = word.lang.as_ref() else {
        return false;
    };

    // The unscoped resolution this needs, and the argument for why it is sound,
    // both live on `UnspannedSwitchTarget::governs`. They used to be a free
    // `GoverningMark::of(word, None)` here under a comment, with `target`
    // passed alongside purely for its language, so the proof was a parameter
    // any `&LanguageCode` could have satisfied and the dangerous call was
    // writable by anyone.
    if target.governs(word, tier_language, declared_languages) {
        word.lang = None;
        true
    } else {
        false
    }
}

fn clear_matching_replaced_word_language_markers(
    replaced: &mut ReplacedWord,
    tier_language: Option<&LanguageCode>,
    declared_languages: &[LanguageCode],
    target: &UnspannedSwitchTarget,
) -> bool {
    let mut cleared_any_word_marker = clear_matching_word_language_marker(
        &mut replaced.word,
        tier_language,
        declared_languages,
        target,
    );
    for word in &mut replaced.replacement.words {
        cleared_any_word_marker |=
            clear_matching_word_language_marker(word, tier_language, declared_languages, target);
    }
    cleared_any_word_marker
}

#[cfg(test)]
mod tests {
    use super::{FixSRewriteStats, rewrite_whole_utterance_language_switches};
    use talkbank_model::model::WriteChat;
    use talkbank_parser::TreeSitterParser;

    fn rewrite(chat: &str) -> (String, FixSRewriteStats) {
        let parser = TreeSitterParser::new().expect("parser");
        let mut parsed = parser.parse_chat_file(chat).expect_built();
        let stats = rewrite_whole_utterance_language_switches(&mut parsed);
        (parsed.to_chat_string(), stats)
    }

    #[test]
    fn rewrites_whole_utterance_shortcuts_to_precode() {
        let input = "@UTF8
@Begin
@Languages:\teng, spa
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\thola@s amiga@s .
@End
";
        let expected = "@UTF8
@Begin
@Languages:\teng, spa
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\t[- spa] hola amiga .
@End
";

        let (rewritten, stats) = rewrite(input);
        assert_eq!(
            stats,
            FixSRewriteStats {
                rewritten_utterances: 1,
                appended_language_codes: 0,
            }
        );
        assert_eq!(rewritten, expected);
    }

    #[test]
    fn rewrites_existing_precode_when_shortcuts_resolve_to_other_language() {
        let input = "@UTF8
@Begin
@Languages:\teng, spa
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\t[- spa] hello@s there@s .
@End
";
        let expected = "@UTF8
@Begin
@Languages:\teng, spa
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\t[- eng] hello there .
@End
";

        let (rewritten, stats) = rewrite(input);
        assert_eq!(
            stats,
            FixSRewriteStats {
                rewritten_utterances: 1,
                appended_language_codes: 0,
            }
        );
        assert_eq!(rewritten, expected);
    }

    #[test]
    fn leaves_mixed_tagged_and_untagged_utterance_unchanged() {
        let input = "@UTF8
@Begin
@Languages:\teng, spa
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\thola@s friend .
@End
";

        let (rewritten, stats) = rewrite(input);
        assert!(stats.is_empty());
        assert_eq!(rewritten, input);
    }

    #[test]
    fn appends_missing_explicit_word_language_to_languages_header() {
        let input = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\thola@s:spa friend .
@End
";
        let expected = "@UTF8
@Begin
@Languages:\teng, spa
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\thola@s:spa friend .
@End
";

        let (rewritten, stats) = rewrite(input);
        assert_eq!(
            stats,
            FixSRewriteStats {
                rewritten_utterances: 0,
                appended_language_codes: 1,
            }
        );
        assert_eq!(rewritten, expected);
    }

    #[test]
    fn appends_missing_explicit_original_replaced_word_language_to_languages_header() {
        let input = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\thola@s:spa [: hello] friend .
@End
";
        let expected = "@UTF8
@Begin
@Languages:\teng, spa
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\thola@s:spa [: hello] friend .
@End
";

        let (rewritten, stats) = rewrite(input);
        assert_eq!(
            stats,
            FixSRewriteStats {
                rewritten_utterances: 0,
                appended_language_codes: 1,
            }
        );
        assert_eq!(rewritten, expected);
    }

    /// RED → GREEN regression, when fix-s adds a `[- LANG]` precode,
    /// EVERY `@s`-marked word in the main tier (including fillers,
    /// nonwords, phonological fragments) must have its marker cleared.
    /// Otherwise the marker's resolution flips:
    ///
    /// - Pre-rewrite (tier-default `spa`, declared `[spa, eng]`):
    ///   `&~orin@s` shortcut resolves to "the OTHER declared
    ///   language" = `eng`.
    /// - Post-rewrite with `[- eng]` precode but unchanged filler:
    ///   tier is now `eng`, shortcut resolves to "the OTHER
    ///   declared language" = `spa`.
    /// - The filler flipped from `eng` to `spa` despite being the
    ///   same source text.
    ///
    /// The bug was that fix-s's clear pass walked
    /// `walk_words_mut(... Some(TierDomain::Mor) ...)`, which skips
    /// fillers/nonwords. Widening to a domain-agnostic walk fixes
    /// it because the predicate already verified every word
    /// (including fillers) resolves to the target language, so
    /// clearing all `@s` markers is safe.
    #[test]
    fn filler_with_at_s_shortcut_is_cleared_to_avoid_resolution_flip() {
        let input = "@UTF8
@Begin
@Languages:\tspa, eng
@Participants:\tCHI Target_Child
@ID:\tspa|corpus|CHI|||||Target_Child|||
*CHI:\thello@s &~orin@s .
@End
";
        let expected = "@UTF8
@Begin
@Languages:\tspa, eng
@Participants:\tCHI Target_Child
@ID:\tspa|corpus|CHI|||||Target_Child|||
*CHI:\t[- eng] hello &~orin .
@End
";
        let (rewritten, stats) = rewrite(input);
        assert_eq!(
            stats,
            FixSRewriteStats {
                rewritten_utterances: 1,
                appended_language_codes: 0,
            }
        );
        assert_eq!(rewritten, expected);
    }

    /// RED → GREEN regression, same flip-prevention rule for
    /// `&-`-style filler and `&+`-style phonological fragment with
    /// `@s` shortcut markers.
    #[test]
    fn dash_and_plus_form_fillers_clear_their_at_s_shortcut() {
        let input = "@UTF8
@Begin
@Languages:\tspa, eng
@Participants:\tCHI Target_Child
@ID:\tspa|corpus|CHI|||||Target_Child|||
*CHI:\thello@s &-um@s &+w@s .
@End
";
        let expected = "@UTF8
@Begin
@Languages:\tspa, eng
@Participants:\tCHI Target_Child
@ID:\tspa|corpus|CHI|||||Target_Child|||
*CHI:\t[- eng] hello &-um &+w .
@End
";
        let (rewritten, stats) = rewrite(input);
        assert_eq!(
            stats,
            FixSRewriteStats {
                rewritten_utterances: 1,
                appended_language_codes: 0,
            }
        );
        assert_eq!(rewritten, expected);
    }

    /// A span's code reaches `@Languages` even when the span encloses no word
    /// the `%mor` walk delivers.
    ///
    /// The reformulation group is skipped by `TierDomain::Mor`, so no word leaf
    /// is emitted from inside it. While the span half of this collection was
    /// read off the WORD walk's scope argument, that meant `fra` was never seen
    /// at all: a scope is only delivered when a word is, and no word was. The
    /// `deu` beside it came through, so the header depended on which notation
    /// the transcriber used, which is the exact failure the span-aware
    /// collection was introduced to fix, one level down.
    #[test]
    fn a_span_on_a_reformulation_still_declares_its_language() {
        let input = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\tI said <how to> [//] [@s:fra] this .
*CHI:\tand hello@s:deu there .
@End
";
        let (rewritten, _) = rewrite(input);
        assert!(
            rewritten.contains("@Languages:\teng, fra, deu"),
            "both notations must reach the header:\n{rewritten}"
        );
    }

    /// A span enclosing no word AT ALL still declares its language.
    ///
    /// `<&=laughs> [@s:hin]` holds one event and no word, so the word walk has
    /// nothing to hand a scope to. Marking a laugh as Hindi is unusual; the
    /// point is that the transcriber wrote a code and the header must say so,
    /// and that a walk over words cannot answer a question about spans.
    #[test]
    fn a_span_enclosing_no_word_still_declares_its_language() {
        let input = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\thello <&=laughs> [@s:hin] there .
@End
";
        let (rewritten, _) = rewrite(input);
        assert!(
            rewritten.contains("@Languages:\teng, hin"),
            "a word-free span still names a language:\n{rewritten}"
        );
    }

    /// A code named by many words of one span is declared ONCE.
    ///
    /// The redundancy that made the two bugs above visible: reading spans off
    /// the word walk reported a span once per word it encloses. Idempotent, so
    /// it produced the right header, which is why it survived; the same shape
    /// produced the wrong header wherever the word count was zero.
    #[test]
    fn a_multi_word_span_declares_its_language_once() {
        let input = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\tI said <how are you> [@s:fra] today .
@End
";
        let (rewritten, _) = rewrite(input);
        // The HEADER line only: the utterance itself legitimately contains
        // `[@s:fra]`, so counting across the whole file counts the source text
        // as well as the declaration, which is what the first version of this
        // assertion did.
        let languages = rewritten
            .lines()
            .find(|line| line.starts_with("@Languages:"))
            .unwrap_or_else(|| panic!("no @Languages header in:\n{rewritten}"));
        assert_eq!(
            languages.matches("fra").count(),
            1,
            "the code belongs to the span, not to each word under it: {languages}"
        );
    }

    /// A span on a REPLACED word declares its language.
    ///
    /// `dog [: cat] [@s:fra]` carries its scoped annotations on the replaced
    /// word itself, not on an enclosing group. The first span walk hand-wrote
    /// an exhaustive match per content enum and listed `ReplacedWord` among the
    /// leaves that carry no annotations, so this declared nothing while the
    /// group form `<the dog> [@s:fra]` worked: the same notation-dependence the
    /// walk exists to remove, one variant over.
    ///
    /// `ScopedAnnotated`'s own docs record a previous hand-written accessor
    /// making exactly this mistake on exactly this variant, which is why the
    /// walk now asks `ContentStructure` instead of matching the enum itself.
    #[test]
    fn a_span_on_a_replaced_word_declares_its_language() {
        let input = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\tI want the dog [: cat] [@s:fra] .
*CHI:\tand hello@s:deu there .
@End
";
        let (rewritten, _) = rewrite(input);
        assert!(
            rewritten.contains("@Languages:\teng, fra, deu"),
            "a span on a replaced word names a language:\n{rewritten}"
        );
    }

    /// Word markers inside a reformulation declare their language.
    ///
    /// The mirror of the span case: `<how@s:fra to@s:fra> [//]` is valid CHAT
    /// carrying two explicit codes, and while the WORD half of this collection
    /// was filtered to `TierDomain::Mor` it declared nothing, because the
    /// filter skips a reformulation group. The span spelling of the same
    /// content declared `fra`, so the header still depended on notation after
    /// the span half was fixed.
    #[test]
    fn word_markers_inside_a_reformulation_declare_their_language() {
        let input = "@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\tI said <how@s:fra to@s:fra> [//] this .
@End
";
        let (rewritten, _) = rewrite(input);
        assert!(
            rewritten.contains("@Languages:\teng, fra"),
            "both notations must reach the header:\n{rewritten}"
        );
    }
}
