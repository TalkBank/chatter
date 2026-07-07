use crate::ErrorCollector;
use crate::Span;
use crate::model::{
    BracketedContent, BracketedItem, Bullet, Group, MainTier, ReplacedWord, Replacement,
    Terminator, UtteranceContent, Word, WordCategory, WordContent, WordLanguageMarker,
    WordShortening,
};
use crate::validation::ValidationContext;
use crate::{ErrorCode, Validate};

/// Generates wor tier produces flat words with timing.
#[test]
fn generate_wor_tier_produces_flat_words_with_timing() -> Result<(), String> {
    let mut timed = Word::simple("hello");
    timed.inline_bullet = Some(Bullet::new(100, 200));
    let plain = Word::simple("world");

    let main = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Word(Box::new(timed.clone())),
            UtteranceContent::Word(Box::new(plain.clone())),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let wor = main.generate_wor_tier();
    let words: Vec<&Word> = wor.words().collect();
    assert_eq!(words.len(), 2);

    assert_eq!(words[0].cleaned_text(), "hello");
    match &words[0].inline_bullet {
        Some(b) => {
            assert_eq!(b.timing.start_ms, 100);
            assert_eq!(b.timing.end_ms, 200);
        }
        None => return Err("expected inline_bullet on first word".into()),
    }

    assert_eq!(words[1].cleaned_text(), "world");
    assert!(words[1].inline_bullet.is_none());
    Ok(())
}

/// Generates wor tier extracts words from groups.
#[test]
fn generate_wor_tier_extracts_words_from_groups() -> Result<(), String> {
    let mut timed = Word::simple("hello");
    timed.inline_bullet = Some(Bullet::new(50, 150));

    let group = Group::new(BracketedContent::new(vec![BracketedItem::Word(Box::new(
        timed.clone(),
    ))]));

    let main = MainTier::new(
        "CHI",
        vec![UtteranceContent::Group(group)],
        Terminator::Period { span: Span::DUMMY },
    );

    let wor = main.generate_wor_tier();
    let words: Vec<&Word> = wor.words().collect();
    assert_eq!(words.len(), 1);

    assert_eq!(words[0].cleaned_text(), "hello");
    match &words[0].inline_bullet {
        Some(b) => {
            assert_eq!(b.timing.start_ms, 50);
            assert_eq!(b.timing.end_ms, 150);
        }
        None => return Err("expected inline_bullet on grouped word".into()),
    }
    Ok(())
}

#[test]
fn find_context_dependent_ca_omission_span_detects_grouped_ca_omission() {
    let omission_span = Span::from_usize(12, 18);
    let omission = Word::new_unchecked("(word)", "word")
        .with_category(WordCategory::CAOmission)
        .with_span(omission_span);
    let main = MainTier::new(
        "CHI",
        vec![UtteranceContent::Group(Group::new(BracketedContent::new(
            vec![BracketedItem::Word(Box::new(omission))],
        )))],
        Terminator::Period { span: Span::DUMMY },
    );

    assert_eq!(
        main.find_context_dependent_ca_omission_span(),
        Some(omission_span)
    );
}

#[test]
fn find_context_dependent_ca_omission_span_detects_replacement_shortening() {
    let shortening_span = Span::from_usize(24, 30);
    let replacement_shortening = Word::new_unchecked("(lo)", "lo")
        .with_content(vec![WordContent::Shortening(
            WordShortening::new_unchecked("lo"),
        )])
        .with_span(shortening_span);
    let replaced = ReplacedWord::new(
        Word::simple("hello"),
        Replacement::new(vec![replacement_shortening]),
    );
    let main = MainTier::new(
        "CHI",
        vec![UtteranceContent::ReplacedWord(Box::new(replaced))],
        Terminator::Period { span: Span::DUMMY },
    );

    assert_eq!(
        main.find_context_dependent_ca_omission_span(),
        Some(shortening_span)
    );
}

#[test]
fn validate_flags_all_at_s_single_language_utterance() {
    let mut hola = Word::simple("hola");
    hola.lang = Some(WordLanguageMarker::Shortcut);
    let mut amiga = Word::simple("amiga");
    amiga.lang = Some(WordLanguageMarker::Shortcut);

    let main = MainTier::new(
        "PAR",
        vec![
            UtteranceContent::Word(Box::new(hola)),
            UtteranceContent::Word(Box::new(amiga)),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let context = ValidationContext::new()
        .with_default_language(
            crate::model::LanguageCode::new("eng").expect("test literal is non-empty"),
        )
        .with_declared_languages(vec![
            crate::model::LanguageCode::new("eng").expect("test literal is non-empty"),
            crate::model::LanguageCode::new("spa").expect("test literal is non-empty"),
        ]);
    let errors = ErrorCollector::new();
    main.validate(&context, &errors);
    let error_vec = errors.into_vec();
    assert!(
        error_vec
            .iter()
            .any(|err| err.code == ErrorCode::WholeUtteranceLanguageSwitchShouldUsePrecode),
        "all-@s utterance should be rejected as a whole-utterance language switch"
    );
}

#[test]
fn validate_allows_mixed_tagged_and_untagged_utterance() {
    let mut hola = Word::simple("hola");
    hola.lang = Some(WordLanguageMarker::Shortcut);
    let friend = Word::simple("friend");

    let main = MainTier::new(
        "PAR",
        vec![
            UtteranceContent::Word(Box::new(hola)),
            UtteranceContent::Word(Box::new(friend)),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let context = ValidationContext::new()
        .with_default_language(
            crate::model::LanguageCode::new("eng").expect("test literal is non-empty"),
        )
        .with_declared_languages(vec![
            crate::model::LanguageCode::new("eng").expect("test literal is non-empty"),
            crate::model::LanguageCode::new("spa").expect("test literal is non-empty"),
        ]);
    let errors = ErrorCollector::new();
    main.validate(&context, &errors);
    let error_vec = errors.into_vec();
    assert!(
        !error_vec
            .iter()
            .any(|err| err.code == ErrorCode::WholeUtteranceLanguageSwitchShouldUsePrecode),
        "utterances with untagged lexical words should stay on the normal word-level path"
    );
}

// ========================================================================
// Regression tests for `whole_utterance_language_switch_target`,
// the predicate behind `chatter debug fix-s` and validator E255.
//
// Bug history (2026-05-06): the predicate originally collected words
// via the MOR-domain walker, which silently skipped fillers (`&~`,
// `&-`, `&+`) and other nonwords. For utterances like
// `*CHI: ballet@s , &~dang3 &~dang1 &~dang1 .`, the predicate saw
// only `[ballet@s]` and concluded "monolingual eng," rewriting the
// utterance to `[- eng] ballet , &~dang3 &~dang1 &~dang1 .` and
// producing E220 ("digits not allowed in eng word") on the Cantonese
// tone fillers downstream. The fix is to walk ALL word-bearing
// items including fillers; the per-word `lang.is_none() → return
// None` guard then catches every filler that lacks an explicit
// `@s:LANG` marker.
//
// See the 2026-05-06 corpus-wide damage assessment for the
// motivating evidence.
// ========================================================================

/// GREEN baseline, clean all-`@s` utterance is correctly detected
/// as a monolingual whole-utterance language switch. The `@s`
/// shortcut resolves to "the OTHER declared language" relative to
/// the tier-default language; here, default=`yue` makes `@s`
/// resolve to `eng`.
#[test]
fn whole_utterance_target_returns_some_for_uniform_at_s_only_words() {
    use crate::model::LanguageCode;
    let main = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Word(Box::new(Word::simple("ballet").with_language_shortcut())),
            UtteranceContent::Word(Box::new(Word::simple("hello").with_language_shortcut())),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let default = LanguageCode::new("yue").expect("test literal is non-empty");
    let declared = vec![
        LanguageCode::new("yue").expect("test literal is non-empty"),
        LanguageCode::new("eng").expect("test literal is non-empty"),
    ];
    let target = main.whole_utterance_language_switch_target(Some(&default), &declared);
    assert_eq!(
        target.as_ref().map(|c| c.as_str()),
        Some("eng"),
        "all-@s utterance with default=yue, declared=yue,eng must resolve to eng"
    );
}

/// RED → GREEN regression, the AliciaCan shape: one `@s` lexical
/// word + several Cantonese tone-bearing nonword fillers (`&~dang3`,
/// etc.). The fillers carry no explicit `@s:LANG` marker, so the
/// predicate must return `None` (cannot confirm whole-utterance
/// monolingual scope).
///
/// Source: `Biling/YipMatthews/Can/AliciaCan/011016.cha:2611`
///, the smoking-gun case for the 2026-05-06 fix-s over-rewrite
/// damage (440 files, 679 utterances).
#[test]
fn whole_utterance_target_returns_none_when_nonword_filler_lacks_lang_marker() {
    use crate::model::LanguageCode;
    let mut nonword = Word::new_unchecked("&~dang3", "dang3");
    nonword = nonword.with_category(WordCategory::Nonword);
    let main = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Word(Box::new(Word::simple("ballet").with_language_shortcut())),
            UtteranceContent::Word(Box::new(nonword)),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let declared = vec![
        LanguageCode::new("yue").expect("test literal is non-empty"),
        LanguageCode::new("eng").expect("test literal is non-empty"),
    ];
    let target = main.whole_utterance_language_switch_target(None, &declared);
    assert_eq!(
        target, None,
        "a nonword filler without an @s:LANG marker must force whole-utterance \
         predicate to return None, otherwise fix-s rewrites the utterance to \
         [- LANG] and produces E220 on Cantonese tone fillers (AliciaCan bug)"
    );
}

/// Same invariant for `&-um`-style filler, the BA2-equivalent
/// English filler, when paired with an `@s` lexical word in a
/// non-English context. Without an explicit `@s:LANG` marker, the
/// filler has language-null status and the predicate must refuse
/// to declare whole-utterance scope.
#[test]
fn whole_utterance_target_returns_none_when_filler_lacks_lang_marker() {
    use crate::model::LanguageCode;
    let mut filler = Word::new_unchecked("&-um", "um");
    filler = filler.with_category(WordCategory::Filler);
    let main = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Word(Box::new(Word::simple("dile").with_language_shortcut())),
            UtteranceContent::Word(Box::new(filler)),
            UtteranceContent::Word(Box::new(Word::simple("a").with_language_shortcut())),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let declared = vec![
        LanguageCode::new("eng").expect("test literal is non-empty"),
        LanguageCode::new("spa").expect("test literal is non-empty"),
    ];
    let target = main.whole_utterance_language_switch_target(None, &declared);
    assert_eq!(
        target, None,
        "an unmarked filler in an otherwise @s-tagged utterance must force the \
         predicate to return None, otherwise fix-s wrongly declares whole-utterance \
         scope despite the filler's unknown language status"
    );
}

/// Same invariant for `&+`-style phonological fragment.
#[test]
fn whole_utterance_target_returns_none_when_phonological_fragment_lacks_lang_marker() {
    use crate::model::LanguageCode;
    let mut frag = Word::new_unchecked("&+fr", "fr");
    frag = frag.with_category(WordCategory::PhonologicalFragment);
    let main = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Word(Box::new(Word::simple("hola").with_language_shortcut())),
            UtteranceContent::Word(Box::new(frag)),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let declared = vec![
        LanguageCode::new("eng").expect("test literal is non-empty"),
        LanguageCode::new("spa").expect("test literal is non-empty"),
    ];
    let target = main.whole_utterance_language_switch_target(None, &declared);
    assert_eq!(
        target, None,
        "a phonological-fragment word without an @s:LANG marker must force the \
         predicate to return None"
    );
}

/// RED → GREEN regression, utterance with an unmarked filler
/// INSIDE a retrace block. Mirrors wild patterns observed in real
/// corpus data, like
/// `*MAR: eh@s la@s &~s [///] el@s viernes@s ...` and
/// `*WYN: people@s [//] (.) some@s ... &~sə [//] strange@s .`.
///
/// Per CHAT semantics, retracted content is still uttered, even
/// though the speaker self-corrected, the false-start words were
/// spoken. Whole-utterance language scope therefore covers the
/// retrace too. If a retracted filler/nonword has no `@s:LANG`
/// marker, the predicate must return None (we cannot confirm
/// monolingual scope).
#[test]
fn whole_utterance_target_returns_none_when_retraced_filler_lacks_lang_marker() {
    use crate::model::LanguageCode;
    use crate::model::content::Retrace;
    use crate::model::content::retrace::RetraceKind;

    // Inside the retrace: bare nonword `&~s` with no lang marker.
    let mut nonword = Word::new_unchecked("&~s", "s");
    nonword = nonword.with_category(WordCategory::Nonword);
    let retrace_content = BracketedContent::new(vec![BracketedItem::Word(Box::new(nonword))]);
    let retrace = Retrace::new(retrace_content, RetraceKind::Multiple);

    // Post-retrace: clean @s shortcut words.
    let main = MainTier::new(
        "MAR",
        vec![
            UtteranceContent::Retrace(Box::new(retrace)),
            UtteranceContent::Word(Box::new(Word::simple("el").with_language_shortcut())),
            UtteranceContent::Word(Box::new(Word::simple("viernes").with_language_shortcut())),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let default = LanguageCode::new("eng").expect("test literal is non-empty");
    let declared = vec![
        LanguageCode::new("eng").expect("test literal is non-empty"),
        LanguageCode::new("spa").expect("test literal is non-empty"),
    ];
    let target = main.whole_utterance_language_switch_target(Some(&default), &declared);
    assert_eq!(
        target, None,
        "an unmarked nonword filler INSIDE a retrace must still force \
         the predicate to return None, retracted-but-uttered content \
         counts toward whole-utterance language scope"
    );
}

/// GREEN guard, if a filler IS explicitly tagged with the same
/// `@s:LANG` as the lexical content, the predicate accepts the
/// rewrite. Locks in that the fix doesn't over-reject, fillers
/// that legitimately match the target language must still allow
/// the precode promotion.
#[test]
fn whole_utterance_target_accepts_rewrite_when_filler_has_matching_explicit_lang() {
    use crate::model::LanguageCode;
    let lang = LanguageCode::new("eng").expect("test literal is non-empty");
    let mut filler = Word::new_unchecked("&-um", "um");
    filler = filler
        .with_category(WordCategory::Filler)
        .with_lang(lang.clone());
    let main = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Word(Box::new(Word::simple("hello").with_lang(lang.clone()))),
            UtteranceContent::Word(Box::new(filler)),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let declared = vec![
        LanguageCode::new("yue").expect("test literal is non-empty"),
        lang.clone(),
    ];
    let target = main.whole_utterance_language_switch_target(None, &declared);
    assert_eq!(
        target.as_ref().map(|c| c.as_str()),
        Some("eng"),
        "filler with explicit matching @s:LANG must not block the rewrite"
    );
}
