//! Tests for utterance language metadata and alignment metadata behavior.
//!
//! This suite guards metadata derivation rules (tier vs word language sources)
//! and interaction points with alignment units and parse-health handling.

use super::super::Utterance;
use crate::Severity;
use crate::model::Quotation;
use crate::model::dependent_tier::DependentTier;
use crate::model::language_metadata::WordLanguages;
use crate::model::{
    Action, AlignmentUnits, Annotated, BracketedContent, BracketedItem, Bullet, CodeSwitchSpan,
    ContentAnnotation, GraTier, GrammaticalRelation, Group, LanguageCode, LanguageSource, MainTier,
    Mor, MorTier, MorWord, ParseHealthTier, PhoItem, PhoTier, PhoWord, PosCategory, SinItem,
    SinTier, SinToken, Terminator, UtteranceContent, UtteranceLanguage, UtteranceLanguageMetadata,
    WorTier, Word, WordLanguageMarker,
};
use crate::validation::ValidationContext;
use crate::{ErrorCode, Span};

/// Builds `LanguageCode` values for test fixtures.
fn codes(list: &[&str]) -> Vec<LanguageCode> {
    list.iter()
        .map(|code| LanguageCode::new(*code).expect("test fixture codes are non-empty"))
        .collect()
}

/// Short helper for constructing one `LanguageCode`.
fn lc(code: &str) -> LanguageCode {
    LanguageCode::new(code).expect("test fixture codes are non-empty")
}

/// Default-language resolution populates utterance and per-word metadata.
///
/// This is the baseline path when neither tier-scoped nor word-scoped overrides are present.
#[test]
fn test_compute_language_metadata() -> Result<(), String> {
    // @Languages: zho, eng
    // *CHI:\tni3 hao3 .

    let word1 = Word::new_unchecked("ni3", "ni3");
    let word2 = Word::new_unchecked("hao3", "hao3");

    let main_tier = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Word(Box::new(word1)),
            UtteranceContent::Word(Box::new(word2)),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let mut utterance = Utterance::new(main_tier);

    let declared_languages = codes(&["zho", "eng"]);
    let default_language = declared_languages.first();
    utterance.compute_language_metadata(default_language, &declared_languages);

    let metadata = utterance
        .language_metadata
        .as_computed()
        .ok_or_else(|| "Expected language metadata".to_string())?;
    assert_eq!(
        utterance.utterance_language,
        UtteranceLanguage::ResolvedDefault { code: lc("zho") }
    );
    assert_eq!(
        crate::model::ValidationTagged::validation_tag(&utterance.utterance_language),
        crate::model::ValidationTag::Clean
    );
    assert_eq!(metadata.tier_language, Some(lc("zho")));
    assert_eq!(metadata.word_languages.len(), 2);

    // Both words should resolve to zho with Default source
    assert_eq!(
        metadata.word_languages[0].languages,
        WordLanguages::Single(lc("zho"))
    );
    assert_eq!(metadata.word_languages[0].source, LanguageSource::Default);

    assert_eq!(
        metadata.word_languages[1].languages,
        WordLanguages::Single(lc("zho"))
    );
    assert_eq!(metadata.word_languages[1].source, LanguageSource::Default);

    // Not code-switching
    assert!(!metadata.is_code_switching());
    Ok(())
}

/// Word-level shortcuts trigger code-switching metadata.
///
/// The test checks both language assignment provenance and aggregate switching detection.
#[test]
fn test_compute_language_metadata_code_switching() -> Result<(), String> {
    // @Languages: zho, eng
    // *CHI:\tni3 hello@s .
    // First word zho, second word eng (via @s)

    let word1 = Word::new_unchecked("ni3", "ni3");

    let mut word2 = Word::new_unchecked("hello@s", "hello");
    word2.lang = Some(WordLanguageMarker::Shortcut);

    let main_tier = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Word(Box::new(word1)),
            UtteranceContent::Word(Box::new(word2)),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let mut utterance = Utterance::new(main_tier);

    let declared_languages = codes(&["zho", "eng"]);
    let default_language = declared_languages.first();
    utterance.compute_language_metadata(default_language, &declared_languages);

    let metadata = utterance
        .language_metadata
        .as_computed()
        .ok_or_else(|| "Expected language metadata".to_string())?;
    assert_eq!(
        utterance.utterance_language,
        UtteranceLanguage::ResolvedDefault { code: lc("zho") }
    );
    assert_eq!(metadata.word_languages.len(), 2);

    // First word: zho (default)
    assert_eq!(
        metadata.word_languages[0].languages,
        WordLanguages::Single(lc("zho"))
    );
    assert_eq!(metadata.word_languages[0].source, LanguageSource::Default);

    // Second word: eng (via @s shortcut)
    assert_eq!(
        metadata.word_languages[1].languages,
        WordLanguages::Single(lc("eng"))
    );
    assert_eq!(
        metadata.word_languages[1].source,
        LanguageSource::WordShortcut
    );

    // This IS code-switching
    assert!(metadata.is_code_switching());

    // Count by language
    let counts = metadata.count_by_language();
    assert_eq!(counts.get(&lc("zho")), Some(&1));
    assert_eq!(counts.get(&lc("eng")), Some(&1));
    Ok(())
}

/// Tier-scoped `[- lang]` overrides become the utterance baseline language.
///
/// All words should inherit the tier language when no per-word overrides are present.
#[test]
fn test_compute_language_metadata_tier_scoped() -> Result<(), String> {
    // @Languages: zho, eng
    // *CHI:\t[- eng] hello world .

    let word1 = Word::new_unchecked("hello", "hello");
    let word2 = Word::new_unchecked("world", "world");

    let mut main_tier = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Word(Box::new(word1)),
            UtteranceContent::Word(Box::new(word2)),
        ],
        Terminator::Period { span: Span::DUMMY },
    );
    main_tier.content.language_code =
        Some(LanguageCode::new("eng").expect("test literal is non-empty"));

    let mut utterance = Utterance::new(main_tier);

    let declared_languages = codes(&["zho", "eng"]);
    let default_language = declared_languages.first();
    utterance.compute_language_metadata(default_language, &declared_languages);

    let metadata = utterance
        .language_metadata
        .as_computed()
        .ok_or_else(|| "Expected language metadata".to_string())?;
    assert_eq!(
        utterance.utterance_language,
        UtteranceLanguage::ResolvedTierScoped { code: lc("eng") }
    );
    assert_eq!(metadata.tier_language, Some(lc("eng"))); // Tier override

    // Both words should resolve to eng with TierScoped source
    assert_eq!(
        metadata.word_languages[0].languages,
        WordLanguages::Single(lc("eng"))
    );
    assert_eq!(
        metadata.word_languages[0].source,
        LanguageSource::TierScoped
    );

    assert_eq!(
        metadata.word_languages[1].languages,
        WordLanguages::Single(lc("eng"))
    );
    assert_eq!(
        metadata.word_languages[1].source,
        LanguageSource::TierScoped
    );
    Ok(())
}

/// Language extraction recurses through grouped content.
///
/// Group-internal words must contribute to the same flat alignable-word metadata sequence.
#[test]
fn test_compute_language_metadata_recurses_into_groups() -> Result<(), String> {
    let grouped_default = Word::new_unchecked("ni3", "ni3");

    let mut grouped_switched = Word::new_unchecked("hello@s", "hello");
    grouped_switched.lang = Some(WordLanguageMarker::Shortcut);

    let group = Group::new(BracketedContent::new(vec![
        BracketedItem::Word(Box::new(grouped_default)),
        BracketedItem::Word(Box::new(grouped_switched)),
    ]));

    let trailing_word = Word::new_unchecked("hao3", "hao3");
    let main_tier = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Group(group),
            UtteranceContent::Word(Box::new(trailing_word)),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let mut utterance = Utterance::new(main_tier);
    let declared_languages = codes(&["zho", "eng"]);
    let default_language = declared_languages.first();
    utterance.compute_language_metadata(default_language, &declared_languages);

    let metadata = utterance
        .language_metadata
        .as_computed()
        .ok_or_else(|| "Expected language metadata".to_string())?;

    assert_eq!(metadata.word_languages.len(), 3);
    assert_eq!(
        metadata.word_languages[0].languages,
        WordLanguages::Single(lc("zho"))
    );
    assert_eq!(metadata.word_languages[0].source, LanguageSource::Default);

    assert_eq!(
        metadata.word_languages[1].languages,
        WordLanguages::Single(lc("eng"))
    );
    assert_eq!(
        metadata.word_languages[1].source,
        LanguageSource::WordShortcut
    );

    assert_eq!(
        metadata.word_languages[2].languages,
        WordLanguages::Single(lc("zho"))
    );
    assert_eq!(metadata.word_languages[2].source, LanguageSource::Default);
    assert!(metadata.is_code_switching());
    Ok(())
}

/// A word inside a QUOTATION is a word, and gets a language record.
///
/// The walk recursed into `Group`/`AnnotatedGroup` and sent every other
/// container to `_ => {}`, so words inside a quotation, phonological group,
/// sign group or retrace got no record AND did not advance the index counter.
/// The second half is the damage: the counter is shared, so every word after
/// a quotation carried a `word_index` for a different word.
///
/// Asserts the COUNT and the per-word resolution. It used to also assert that
/// the stored index equalled the vector position; that field is gone, because
/// it was the vector position, and a test asserting two things stay equal is a
/// standing confession that one of them should not exist.
#[test]
fn test_compute_language_metadata_recurses_into_every_container() -> Result<(), String> {
    let quoted = Word::new_unchecked("ni3", "ni3");
    let quotation = Quotation::new(BracketedContent::new(vec![BracketedItem::Word(Box::new(
        quoted,
    ))]));

    let main_tier = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Word(Box::new(Word::new_unchecked("hao3", "hao3"))),
            UtteranceContent::Quotation(quotation),
            UtteranceContent::Word(Box::new(Word::new_unchecked("ma", "ma"))),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let mut utterance = Utterance::new(main_tier);
    let declared_languages = codes(&["zho"]);
    utterance.compute_language_metadata(declared_languages.first(), &declared_languages);

    let metadata = utterance
        .language_metadata
        .as_computed()
        .ok_or_else(|| "Expected language metadata".to_string())?;

    assert_eq!(
        metadata.word_languages.len(),
        3,
        "the quoted word must get a record too"
    );
    // The quoted word is the middle one, so its presence is also an assertion
    // about ORDER: in-order traversal puts it between the two plain words.
    let sources: Vec<_> = metadata
        .word_languages
        .iter()
        .map(|info| info.source.clone())
        .collect();
    assert_eq!(sources, vec![LanguageSource::Default; 3]);
    Ok(())
}

/// Missing tier/default language leaves utterance language unresolved.
///
/// The unresolved status should propagate to per-word metadata entries.
#[test]
fn test_compute_language_metadata_unresolved_without_tier_or_default() -> Result<(), String> {
    let word = Word::new_unchecked("hello", "hello");
    let main_tier = MainTier::new(
        "CHI",
        vec![UtteranceContent::Word(Box::new(word))],
        Terminator::Period { span: Span::DUMMY },
    );
    let mut utterance = Utterance::new(main_tier);

    let declared_languages: Vec<LanguageCode> = vec![];
    let default_language: Option<&LanguageCode> = None;
    utterance.compute_language_metadata(default_language, &declared_languages);

    let metadata = utterance
        .language_metadata
        .as_computed()
        .ok_or_else(|| "Expected language metadata".to_string())?;
    assert_eq!(utterance.utterance_language, UtteranceLanguage::Unresolved);
    assert_eq!(
        crate::model::ValidationTagged::validation_tag(&utterance.utterance_language),
        crate::model::ValidationTag::Error
    );
    assert_eq!(metadata.tier_language, None);
    assert_eq!(metadata.word_languages.len(), 1);
    assert_eq!(
        metadata.word_languages[0].languages,
        WordLanguages::Unresolved
    );
    assert_eq!(
        metadata.word_languages[0].source,
        LanguageSource::Unresolved
    );
    Ok(())
}

/// `UtteranceLanguage::Uncomputed` maps to warning-level validation state.
///
/// This keeps "not yet computed" distinct from true parse/semantic errors.
#[test]
fn test_utterance_language_uncomputed_is_warning_tag() {
    let state = UtteranceLanguage::Uncomputed;
    assert_eq!(
        crate::model::ValidationTagged::validation_tag(&state),
        crate::model::ValidationTag::Warning
    );
    assert!(crate::model::ValidationTagged::is_validation_warning(
        &state
    ));
}

/// Default language metadata state starts as uncomputed warning.
///
/// The default communicates "analysis pending" rather than invalid transcript content.
#[test]
fn test_language_metadata_state_defaults_to_uncomputed_warning() {
    let state = UtteranceLanguageMetadata::default();
    assert!(matches!(state, UtteranceLanguageMetadata::Uncomputed));
    assert_eq!(
        crate::model::ValidationTagged::validation_tag(&state),
        crate::model::ValidationTag::Warning
    );
}

/// Builds alignment fixture utterance for downstream use.
fn build_alignment_fixture_utterance() -> Utterance {
    let main = MainTier::new(
        "CHI",
        vec![UtteranceContent::Word(Box::new(Word::simple("hello")))],
        None::<Terminator>,
    );

    let mor_item = Mor::new(MorWord::new(PosCategory::new("noun"), "hello"));
    let mor = MorTier::new_mor(
        vec![mor_item],
        crate::Terminator::Period {
            span: crate::Span::DUMMY,
        },
    );

    let gra = GraTier::new_gra(vec![
        GrammaticalRelation::new(1, 0, "ROOT"),
        GrammaticalRelation::new(2, 1, "PUNCT"),
    ]);
    let pho = PhoTier::new_pho(vec![PhoItem::Word(PhoWord::new("helo"))]);
    let wor = WorTier::from_words(vec![Word::simple("hello")]);

    Utterance::new(main)
        .with_mor(mor)
        .with_gra(gra)
        .with_pho(pho)
        .add_dependent_tier(DependentTier::Wor(wor))
}

/// Alignment computation produces both main↔`%mor` and `%mor`↔`%gra` mappings.
///
/// This integration test ensures downstream consumers can rely on both alignment layers.
#[test]
fn compute_alignments_produces_mor_and_gra_alignment() -> Result<(), String> {
    let mut utterance = build_alignment_fixture_utterance();
    let context = ValidationContext::default();
    utterance.compute_alignments(&context);

    let alignments = utterance
        .alignments
        .as_ref()
        .ok_or_else(|| "Expected computed alignments".to_string())?;

    // main <-> %mor alignment should be present and error-free
    let mor = alignments
        .mor
        .as_ref()
        .ok_or_else(|| "Expected main↔%mor alignment".to_string())?;
    assert!(
        mor.is_error_free(),
        "main↔%mor alignment should have no errors"
    );
    assert!(
        !mor.pairs.is_empty(),
        "main↔%mor alignment should have pairs"
    );

    // %mor <-> %gra alignment should be present and error-free
    let gra = alignments
        .gra
        .as_ref()
        .ok_or_else(|| "Expected %mor↔%gra alignment".to_string())?;
    assert!(
        gra.is_error_free(),
        "%mor↔%gra alignment should have no errors"
    );
    assert!(
        !gra.pairs.is_empty(),
        "%mor↔%gra alignment should have pairs"
    );

    Ok(())
}

/// Alignment computation produces `%wor` mapping while preserving inline bullets.
///
/// The test verifies both alignment-pair output and that timing bullets remain
/// attached to `%wor` words after computation.
#[test]
fn compute_alignments_produces_wor_alignment_with_inline_bullets() -> Result<(), String> {
    let main = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Word(Box::new(Word::simple("one"))),
            UtteranceContent::Word(Box::new(Word::simple("two"))),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let mut timed_word = Word::simple("one");
    timed_word.inline_bullet = Some(Bullet::new(100, 220));
    let wor = WorTier::from_words(vec![timed_word, Word::simple("two")]);

    let mut utterance = Utterance::new(main).add_dependent_tier(DependentTier::Wor(wor));
    let context = ValidationContext::default();
    utterance.compute_alignments(&context);

    let alignments = utterance
        .alignments
        .as_ref()
        .ok_or_else(|| "Expected computed alignments".to_string())?;

    let wor_sidecar = alignments
        .wor_timings
        .as_ref()
        .ok_or_else(|| "Expected main↔%wor timing sidecar".to_string())?;
    assert_eq!(
        *wor_sidecar,
        crate::alignment::WorTimingSidecar::Positional { count: 2 },
        "main↔%wor timing sidecar should be positional with count 2",
    );

    // Verify inline_bullet is preserved on the wor tier word
    let wor_words: Vec<&Word> = utterance
        .dependent_tiers
        .iter()
        .filter_map(|dt| match &dt.tier {
            DependentTier::Wor(wor) => Some(wor.words()),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(wor_words.len(), 2);
    assert_eq!(
        wor_words[0].inline_bullet,
        Some(Bullet::new(100, 220)),
        "First wor word should have inline_bullet"
    );
    assert!(
        wor_words[1].inline_bullet.is_none(),
        "Second wor word should have no inline_bullet"
    );

    Ok(())
}

/// Confirms `%sin` alignment-unit counting includes annotated actions on the main tier.
#[test]
fn alignment_units_count_annotated_action_for_sin_domain() {
    let main = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::AnnotatedAction(Annotated::new(Action::new())),
            UtteranceContent::Word(Box::new(Word::new_unchecked("word", "word"))),
        ],
        Terminator::Period { span: Span::DUMMY },
    );
    let sin = SinTier::new(vec![
        SinItem::Token(SinToken::new_unchecked("0")),
        SinItem::Token(SinToken::new_unchecked("0")),
    ]);

    let utterance = Utterance::new(main).with_sin(sin);
    let units = AlignmentUnits::from_utterance(&utterance, &ValidationContext::default());

    assert_eq!(
        units.main_sin.len(),
        2,
        "main_sin must count annotated action"
    );
    assert_eq!(
        units.sin.len(),
        2,
        "%sin units should reflect tier item count"
    );
}

/// Parse-health taint on `%gra` suppresses only `%gra`-dependent alignment paths.
///
/// Other alignment domains should remain available and error-free.
#[test]
fn parse_health_taints_only_gra_alignment_when_gra_tier_is_tainted() -> Result<(), String> {
    let mut utterance = build_alignment_fixture_utterance();
    utterance.mark_parse_taint(ParseHealthTier::Gra);
    let context = ValidationContext::default();
    utterance.compute_alignments(&context);

    let alignments = utterance
        .alignments
        .as_ref()
        .ok_or_else(|| "Expected computed alignments".to_string())?;

    assert!(
        alignments
            .mor
            .as_ref()
            .ok_or_else(|| "Expected main↔%mor alignment".to_string())?
            .is_error_free()
    );
    assert!(
        alignments
            .pho
            .as_ref()
            .ok_or_else(|| "Expected main↔%pho alignment".to_string())?
            .is_error_free()
    );
    // `%wor` is a sidecar, presence of `Positional` is the analogue of
    // the old `is_error_free()` check on the other alignments.
    assert!(
        alignments
            .wor_timings
            .as_ref()
            .ok_or_else(|| "Expected main↔%wor timing sidecar".to_string())?
            .is_positional()
    );

    let gra = alignments
        .gra
        .as_ref()
        .ok_or_else(|| "Expected %mor↔%gra alignment".to_string())?;
    assert_eq!(gra.errors.len(), 1);
    assert_eq!(gra.errors[0].code, ErrorCode::TierValidationError);
    assert_eq!(gra.errors[0].severity, Severity::Warning);
    assert!(gra.errors[0].message.contains(
        "Tier validation warning: skipped %mor↔%gra alignment because %gra tier had parse errors during recovery"
    ));
    Ok(())
}

/// Main-tier parse taint suppresses main-dependent alignments but keeps `%mor↔%gra`.
///
/// This guards the contract that `%mor↔%gra` can still run when `%mor/%gra`
/// are clean, even if main-tier recovery marked `%mor/%pho/%wor` as skipped.
#[test]
fn parse_health_taints_main_dependent_alignments_but_keeps_mor_gra_alignment() -> Result<(), String>
{
    let mut utterance = build_alignment_fixture_utterance();
    utterance.mark_parse_taint(ParseHealthTier::Main);
    let context = ValidationContext::default();
    utterance.compute_alignments(&context);

    let alignments = utterance
        .alignments
        .as_ref()
        .ok_or_else(|| "Expected computed alignments".to_string())?;

    let mor = alignments
        .mor
        .as_ref()
        .ok_or_else(|| "Expected main↔%mor alignment".to_string())?;
    assert_eq!(mor.errors.len(), 1);
    assert_eq!(mor.errors[0].code, ErrorCode::TierValidationError);
    assert!(mor.errors[0].message.contains(
        "Tier validation warning: skipped main↔%mor alignment because main tier had parse errors during recovery"
    ));

    let pho = alignments
        .pho
        .as_ref()
        .ok_or_else(|| "Expected main↔%pho alignment".to_string())?;
    assert_eq!(pho.errors.len(), 1);
    assert_eq!(pho.errors[0].code, ErrorCode::TierValidationError);
    assert!(pho.errors[0].message.contains(
        "Tier validation warning: skipped main↔%pho alignment because main tier had parse errors during recovery"
    ));

    // `%wor` is a timing sidecar, not a `TierAlignmentResult`. On parse-taint
    // the slot stays `None`, taint context lives on `ParseHealth`, not in
    // fabricated error-shaped alignments.
    assert!(
        alignments.wor_timings.is_none(),
        "%wor timing sidecar must be absent when main tier parse is tainted"
    );

    assert!(
        alignments
            .gra
            .as_ref()
            .ok_or_else(|| "Expected %mor↔%gra alignment".to_string())?
            .is_error_free(),
        "main-tier taint must not block %mor↔%gra alignment"
    );
    Ok(())
}

/// Words inside `<...> [@s]` resolve to the span's language, with the span
/// named as the provenance.
///
/// This is the behaviour the code-switch span exists for, and it is a
/// MEASUREMENT of resolution rather than an invariant a type could hold: the
/// answer depends on the declared-language context, which is data. The parse
/// shape is pinned separately by the generated construct corpus.
///
/// Both halves are asserted deliberately. The resolved CODE proves the span
/// applies at all; the `LanguageSource` proves a consumer can still tell a
/// span-governed word from one the transcriber marked individually, which is
/// the distinction `SpanShortcut` exists to preserve and which a shared variant
/// would have destroyed silently.
#[test]
fn span_words_resolve_to_the_span_language() -> Result<(), String> {
    // `ik <how to> [@s] .` under nld+eng: the bare span resolves the way a bare
    // `word@s` does, to the non-primary language.
    let outside = Word::new_unchecked("ik", "ik");
    let inside_first = Word::new_unchecked("how", "how");
    let inside_second = Word::new_unchecked("to", "to");

    let group = Annotated::new(Group::new(BracketedContent::new(vec![
        BracketedItem::Word(Box::new(inside_first)),
        BracketedItem::Word(Box::new(inside_second)),
    ])))
    .with_scoped_annotations(vec![ContentAnnotation::CodeSwitch(
        CodeSwitchSpan::Shortcut,
    )]);

    let main_tier = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Word(Box::new(outside)),
            UtteranceContent::AnnotatedGroup(group),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let mut utterance = Utterance::new(main_tier);
    let declared = codes(&["nld", "eng"]);
    utterance.compute_language_metadata(declared.first(), &declared);

    let metadata = utterance
        .language_metadata
        .as_computed()
        .ok_or_else(|| "expected computed language metadata".to_string())?;
    assert_eq!(metadata.word_languages.len(), 3);

    // The word outside the span keeps the utterance's language.
    assert_eq!(
        metadata.word_languages[0].languages,
        WordLanguages::Single(lc("nld"))
    );
    assert_eq!(metadata.word_languages[0].source, LanguageSource::Default);

    // Both words inside it switch, and say the SPAN put them there.
    for index in [1, 2] {
        assert_eq!(
            metadata.word_languages[index].languages,
            WordLanguages::Single(lc("eng")),
            "word {index} should take the span's language"
        );
        assert_eq!(
            metadata.word_languages[index].source,
            LanguageSource::SpanShortcut,
            "word {index} should name the span as its provenance"
        );
    }

    Ok(())
}

/// An explicit `<...> [@s:code]` names the language directly, and says so.
#[test]
fn explicit_span_words_resolve_to_the_named_language() -> Result<(), String> {
    let group = Annotated::new(Group::new(BracketedContent::new(vec![
        BracketedItem::Word(Box::new(Word::new_unchecked("hola", "hola"))),
    ])))
    .with_scoped_annotations(vec![ContentAnnotation::CodeSwitch(
        CodeSwitchSpan::Explicit(lc("spa")),
    )]);

    let main_tier = MainTier::new(
        "CHI",
        vec![UtteranceContent::AnnotatedGroup(group)],
        Terminator::Period { span: Span::DUMMY },
    );

    let mut utterance = Utterance::new(main_tier);
    // Deliberately NOT declared in @Languages: an explicit code carries no such
    // requirement, matching word-level `@s:code`.
    let declared = codes(&["eng"]);
    utterance.compute_language_metadata(declared.first(), &declared);

    let metadata = utterance
        .language_metadata
        .as_computed()
        .ok_or_else(|| "expected computed language metadata".to_string())?;
    assert_eq!(
        metadata.word_languages[0].languages,
        WordLanguages::Single(lc("spa"))
    );
    assert_eq!(
        metadata.word_languages[0].source,
        LanguageSource::SpanExplicit
    );

    Ok(())
}

/// A word's OWN marker wins over an enclosing span, and the provenance follows
/// the marker rather than the span.
///
/// The redundant combination is a proposed validation finding that nothing
/// reports yet; resolution must still answer, and this pins which answer.
#[test]
fn a_words_own_marker_wins_over_an_enclosing_span() -> Result<(), String> {
    let mut marked = Word::new_unchecked("ciao@s:ita", "ciao");
    marked.lang = Some(WordLanguageMarker::explicit(lc("ita")));

    let group = Annotated::new(Group::new(BracketedContent::new(vec![
        BracketedItem::Word(Box::new(marked)),
    ])))
    .with_scoped_annotations(vec![ContentAnnotation::CodeSwitch(
        CodeSwitchSpan::Explicit(lc("spa")),
    )]);

    let main_tier = MainTier::new(
        "CHI",
        vec![UtteranceContent::AnnotatedGroup(group)],
        Terminator::Period { span: Span::DUMMY },
    );

    let mut utterance = Utterance::new(main_tier);
    let declared = codes(&["eng"]);
    utterance.compute_language_metadata(declared.first(), &declared);

    let metadata = utterance
        .language_metadata
        .as_computed()
        .ok_or_else(|| "expected computed language metadata".to_string())?;
    assert_eq!(
        metadata.word_languages[0].languages,
        WordLanguages::Single(lc("ita")),
        "the word's own marker decides the code"
    );
    assert_eq!(
        metadata.word_languages[0].source,
        LanguageSource::WordExplicit,
        "and the provenance names the word, not the span"
    );

    Ok(())
}

/// A code-switch annotation attached to ONE word, with no angle brackets,
/// governs that word.
///
/// CHAT's convention is that any scoped annotation may attach to a single
/// content item directly, so `hallo [@s]` is the one-word form of `<a b> [@s]`
/// and means what `hallo@s` means. It is NOT an error to be rejected.
///
/// This is a regression guard with a real history: the first implementation
/// opened the language scope only at an annotated GROUP, so this form parsed,
/// validated and round-tripped byte-identically while resolving to the tier
/// language. The transcriber's mark was preserved in the bytes and dropped in
/// meaning, with no diagnostic anywhere.
#[test]
fn a_code_switch_annotation_on_one_word_governs_that_word() -> Result<(), String> {
    let plain = Word::new_unchecked("ik", "ik");
    let annotated =
        Annotated::new(Word::new_unchecked("hallo", "hallo")).with_scoped_annotations(vec![
            ContentAnnotation::CodeSwitch(CodeSwitchSpan::Shortcut),
        ]);

    let main_tier = MainTier::new(
        "CHI",
        vec![
            UtteranceContent::Word(Box::new(plain)),
            UtteranceContent::AnnotatedWord(Box::new(annotated)),
        ],
        Terminator::Period { span: Span::DUMMY },
    );

    let mut utterance = Utterance::new(main_tier);
    let declared = codes(&["nld", "eng"]);
    utterance.compute_language_metadata(declared.first(), &declared);

    let metadata = utterance
        .language_metadata
        .as_computed()
        .ok_or_else(|| "expected computed language metadata".to_string())?;

    assert_eq!(
        metadata.word_languages[0].languages,
        WordLanguages::Single(lc("nld"))
    );
    assert_eq!(
        metadata.word_languages[1].languages,
        WordLanguages::Single(lc("eng")),
        "the annotated word takes the switched language"
    );
    assert_eq!(
        metadata.word_languages[1].source,
        LanguageSource::SpanShortcut
    );

    Ok(())
}
