// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! Utterance language metadata and alignment metadata, over parsed utterances.
//!
//! # What this replaces
//!
//! `talkbank-model`'s `model/file/utterance/metadata/tests.rs` built each
//! `Utterance` by hand and wrote the CHAT it meant in a comment beside it:
//! `// *CHI:\tni3 hello@s .` above `Word::new_unchecked("hello@s", "hello")`
//! with `lang = Some(WordLanguageMarker::Shortcut)`. The comment and the
//! struct were two statements of one fact with nothing holding them together.
//! Here the comment IS the input.
//!
//! # Why it moved crates, stated accurately
//!
//! A `#[cfg(test)] mod` in `src/` cannot parse (second instantiation of the
//! crate); an integration test under `tests/` with a dev-dependency cycle
//! could, as `talkbank-derive` does. This crate already depends on the
//! tree-sitter parser and is the cheaper home.
//!
//! # What each test asserts
//!
//! The same things the originals did: the resolved utterance language and its
//! validation tag, the per-word language AND its provenance (`LanguageSource`),
//! the count of word records, code-switch detection, and per-language counts.
//! The provenance is not decoration: a consumer must be able to tell a
//! span-governed word from one the transcriber marked, which is the distinction
//! `SpanShortcut` exists to preserve.

use talkbank_model::model::{
    AlignmentUnits, DependentTier, LanguageCode, LanguageSource, ParseHealthTier, Utterance,
    UtteranceLanguage, UtteranceLanguageMetadata, ValidationTag, ValidationTagged, WordLanguages,
};
use talkbank_model::validation::ValidationContext;
use talkbank_model::{ErrorCode, Severity};
use talkbank_parser_tests::from_source::{Media, SingleSpeaker};
use talkbank_parser_tests::test_error::TestError;

fn lc(code: &str) -> LanguageCode {
    LanguageCode::new(code).expect("test fixture codes are non-empty")
}

fn codes(list: &[&str]) -> Vec<LanguageCode> {
    list.iter().map(|c| lc(c)).collect()
}

/// Parse one utterance (plus any dependent tiers) into an `Utterance`, after
/// the whole fixture has been checked to be VALID CHAT: every fixture here is
/// meant to be, and the review of the first draft found two that were not
/// (`@ID` naming a language `@Languages` did not declare; bullets with no
/// `@Media`), accepted by a helper that refused only parse faults.
fn utterance(languages: &str, body: &str) -> Result<Utterance, TestError> {
    utterance_with(languages, Media::Undeclared, body)
}

fn utterance_with(languages: &str, media: Media, body: &str) -> Result<Utterance, TestError> {
    SingleSpeaker {
        languages,
        options: None,
        media,
        lines: body,
    }
    .utterance(&[])
}

/// Compute language metadata the way the originals did, with the declared
/// languages given explicitly so the method under test is the only thing
/// deciding.
fn computed(
    mut utt: Utterance,
    declared: &[&str],
) -> Result<(Utterance, UtteranceLanguageMetadata), TestError> {
    let declared = codes(declared);
    utt.compute_language_metadata(declared.first(), &declared);
    let metadata = utt.language_metadata.clone();
    Ok((utt, metadata))
}

/// The per-word (language, source) pairs, in order.
fn word_languages(metadata: &UtteranceLanguageMetadata) -> Vec<(WordLanguages, LanguageSource)> {
    metadata
        .as_computed()
        .expect("metadata was computed")
        .word_languages
        .iter()
        .map(|w| (w.languages.clone(), w.source.clone()))
        .collect()
}

fn single(code: &str, source: LanguageSource) -> (WordLanguages, LanguageSource) {
    (WordLanguages::Single(lc(code)), source)
}

// ── compute_language_metadata ───────────────────────────────────────────

/// Default-language resolution populates the utterance and every word.
#[test]
fn default_language_resolves_every_word() -> Result<(), TestError> {
    let (utt, metadata) = computed(utterance("zho, eng", "*CHI:\tni3 hao3 .")?, &["zho", "eng"])?;
    assert_eq!(
        utt.utterance_language,
        UtteranceLanguage::ResolvedDefault { code: lc("zho") }
    );
    assert_eq!(
        utt.utterance_language.validation_tag(),
        ValidationTag::Clean
    );
    let computed = metadata.as_computed().expect("computed");
    assert_eq!(computed.tier_language, Some(lc("zho")));
    assert_eq!(
        word_languages(&metadata),
        vec![
            single("zho", LanguageSource::Default),
            single("zho", LanguageSource::Default)
        ]
    );
    assert!(!computed.is_code_switching());
    Ok(())
}

/// A word-level `@s` shortcut is a code switch, with its provenance.
#[test]
fn a_word_shortcut_is_a_code_switch() -> Result<(), TestError> {
    let (utt, metadata) = computed(
        utterance("zho, eng", "*CHI:\tni3 hello@s .")?,
        &["zho", "eng"],
    )?;
    assert_eq!(
        utt.utterance_language,
        UtteranceLanguage::ResolvedDefault { code: lc("zho") }
    );
    assert_eq!(
        word_languages(&metadata),
        vec![
            single("zho", LanguageSource::Default),
            single("eng", LanguageSource::WordShortcut)
        ]
    );
    let computed = metadata.as_computed().expect("computed");
    assert!(computed.is_code_switching());
    let counts = computed.count_by_language();
    assert_eq!(counts.get(&lc("zho")), Some(&1));
    assert_eq!(counts.get(&lc("eng")), Some(&1));
    Ok(())
}

/// A `[- code]` precode governs the whole tier.
#[test]
fn a_precode_scopes_the_tier() -> Result<(), TestError> {
    let (utt, metadata) = computed(
        // The words must be legal in the PRECODE language: `ni3 hao3` under
        // `[- eng]` is E220 (digits), which the first draft did not notice.
        utterance("zho, eng", "*CHI:\t[- eng] hello world .")?,
        &["zho", "eng"],
    )?;
    assert_eq!(
        utt.utterance_language,
        UtteranceLanguage::ResolvedTierScoped { code: lc("eng") }
    );
    assert_eq!(
        metadata.as_computed().expect("computed").tier_language,
        Some(lc("eng"))
    );
    assert_eq!(
        word_languages(&metadata),
        vec![
            single("eng", LanguageSource::TierScoped),
            single("eng", LanguageSource::TierScoped)
        ]
    );
    Ok(())
}

/// Words inside a group contribute to the same flat sequence, in order.
///
/// The original assembled a bare `Group`; a bare `<...>` with no annotation is
/// not a parse shape, so this uses a group carrying a scoped stressing
/// annotation, which the walker descends into the same way.
#[test]
fn words_inside_a_group_are_recorded_in_order() -> Result<(), TestError> {
    let (_, metadata) = computed(
        utterance("zho, eng", "*CHI:\t<ni3 hello@s> [!] hao3 .")?,
        &["zho", "eng"],
    )?;
    assert_eq!(
        word_languages(&metadata),
        vec![
            single("zho", LanguageSource::Default),
            single("eng", LanguageSource::WordShortcut),
            single("zho", LanguageSource::Default),
        ]
    );
    assert!(
        metadata
            .as_computed()
            .expect("computed")
            .is_code_switching()
    );
    Ok(())
}

/// A word inside a QUOTATION is a word and gets a record, in order.
///
/// The walk once recursed into groups only, so words inside a quotation got
/// no record AND did not advance the shared counter, and every word after a
/// quotation carried another word's index. The count and the order are the
/// two halves of that regression.
#[test]
fn words_inside_a_quotation_are_recorded_in_order() -> Result<(), TestError> {
    let (_, metadata) = computed(
        utterance("zho", "*CHI:\thao3 \u{201c}ni3\u{201d} ma .")?,
        &["zho"],
    )?;
    let sources: Vec<LanguageSource> = word_languages(&metadata)
        .into_iter()
        .map(|(_, s)| s)
        .collect();
    assert_eq!(
        sources,
        vec![LanguageSource::Default; 3],
        "the quoted word must get a record too"
    );
    Ok(())
}

/// With no tier language and no default, everything is unresolved, and the
/// utterance language carries an error tag.
#[test]
fn nothing_declared_leaves_everything_unresolved() -> Result<(), TestError> {
    // The FILE declares a language so it parses; the method is handed none,
    // which is the question the original asked.
    let (utt, metadata) = computed(utterance("eng", "*CHI:\thello .")?, &[])?;
    assert_eq!(utt.utterance_language, UtteranceLanguage::Unresolved);
    assert_eq!(
        utt.utterance_language.validation_tag(),
        ValidationTag::Error
    );
    assert_eq!(
        metadata.as_computed().expect("computed").tier_language,
        None
    );
    assert_eq!(
        word_languages(&metadata),
        vec![(WordLanguages::Unresolved, LanguageSource::Unresolved)]
    );
    Ok(())
}

// ── code-switch spans ───────────────────────────────────────────────────

/// Words inside `<...> [@s]` resolve to the span's language, and say the
/// SPAN was the reason.
#[test]
fn span_words_resolve_to_the_span_language() -> Result<(), TestError> {
    let (_, metadata) = computed(
        utterance("nld, eng", "*CHI:\tik <how to> [@s] .")?,
        &["nld", "eng"],
    )?;
    assert_eq!(
        word_languages(&metadata),
        vec![
            single("nld", LanguageSource::Default),
            single("eng", LanguageSource::SpanShortcut),
            single("eng", LanguageSource::SpanShortcut),
        ]
    );
    Ok(())
}

/// An explicit `<...> [@s:code]` names the language directly, and says so.
#[test]
fn explicit_span_words_resolve_to_the_named_language() -> Result<(), TestError> {
    let (_, metadata) = computed(utterance("eng", "*CHI:\t<hola> [@s:spa] .")?, &["eng"])?;
    assert_eq!(
        word_languages(&metadata),
        vec![single("spa", LanguageSource::SpanExplicit)]
    );
    Ok(())
}

/// A word's OWN marker wins over an enclosing span, and the provenance follows
/// the marker.
#[test]
fn a_words_own_marker_wins_over_an_enclosing_span() -> Result<(), TestError> {
    let (_, metadata) = computed(
        utterance("eng", "*CHI:\t<ciao@s:ita> [@s:spa] .")?,
        &["eng"],
    )?;
    assert_eq!(
        word_languages(&metadata),
        vec![single("ita", LanguageSource::WordExplicit)]
    );
    Ok(())
}

/// A code-switch annotation attached to ONE word, with no angle brackets,
/// governs that word: `hallo [@s]` means what `hallo@s` means.
///
/// Regression guard with a history: the first implementation opened the
/// language scope only at an annotated GROUP, so this form parsed, validated
/// and round-tripped byte-identically while resolving to the tier language.
#[test]
fn a_code_switch_annotation_on_one_word_governs_that_word() -> Result<(), TestError> {
    let (_, metadata) = computed(
        utterance("nld, eng", "*CHI:\tik hallo [@s] .")?,
        &["nld", "eng"],
    )?;
    assert_eq!(
        word_languages(&metadata),
        vec![
            single("nld", LanguageSource::Default),
            single("eng", LanguageSource::SpanShortcut)
        ]
    );
    Ok(())
}

// ── alignment metadata ──────────────────────────────────────────────────

const ALIGNED: &str =
    "*CHI:\thello .\n%mor:\tnoun|hello .\n%gra:\t1|0|ROOT 2|1|PUNCT\n%pho:\thelo\n%wor:\thello .";

/// Both main<->%mor and %mor<->%gra alignments come out, error-free.
#[test]
fn alignments_are_computed_for_mor_and_gra() -> Result<(), TestError> {
    let mut utt = utterance("eng", ALIGNED)?;
    utt.compute_alignments(&ValidationContext::default());
    let alignments = utt.alignments.as_ref().expect("computed alignments");
    let mor = alignments.mor.as_ref().expect("main<->%mor alignment");
    assert!(mor.is_error_free() && !mor.pairs.is_empty());
    let gra = alignments.gra.as_ref().expect("%mor<->%gra alignment");
    assert!(gra.is_error_free() && !gra.pairs.is_empty());
    Ok(())
}

/// A `%gra` taint suppresses only the `%mor<->%gra` alignment, as a warning.
#[test]
fn a_gra_taint_suppresses_only_the_gra_alignment() -> Result<(), TestError> {
    let mut utt = utterance("eng", ALIGNED)?;
    utt.mark_parse_taint(ParseHealthTier::Gra);
    utt.compute_alignments(&ValidationContext::default());
    let alignments = utt.alignments.as_ref().expect("computed alignments");
    assert!(alignments.mor.as_ref().expect("mor").is_error_free());
    assert!(alignments.pho.as_ref().expect("pho").is_error_free());
    assert!(matches!(
        alignments.wor_timings,
        Some(talkbank_model::alignment::WorTimingSidecar::Positional { .. })
    ));
    let gra = alignments.gra.as_ref().expect("gra");
    assert_eq!(gra.errors.len(), 1);
    assert_eq!(gra.errors[0].code, ErrorCode::TierValidationError);
    assert_eq!(gra.errors[0].severity, Severity::Warning);
    assert!(
        gra.errors[0].message.contains(
            "Tier validation warning: skipped %mor\u{2194}%gra alignment because %gra tier \
             had parse errors during recovery"
        ),
        "{}",
        gra.errors[0].message
    );
    Ok(())
}

/// A main-tier taint suppresses the main-dependent alignments and keeps
/// `%mor<->%gra`, which does not read the main tier.
#[test]
fn a_main_taint_keeps_the_mor_gra_alignment() -> Result<(), TestError> {
    let mut utt = utterance("eng", ALIGNED)?;
    utt.mark_parse_taint(ParseHealthTier::Main);
    utt.compute_alignments(&ValidationContext::default());
    let alignments = utt.alignments.as_ref().expect("computed alignments");
    for (name, errors) in [
        ("mor", &alignments.mor.as_ref().expect("mor").errors),
        ("pho", &alignments.pho.as_ref().expect("pho").errors),
    ] {
        assert_eq!(errors.len(), 1, "{name}");
        assert_eq!(errors[0].code, ErrorCode::TierValidationError, "{name}");
        let skipped = format!(
            "Tier validation warning: skipped main\u{2194}%{name} alignment because main tier \
             had parse errors during recovery"
        );
        assert!(
            errors[0].message.contains(&skipped),
            "{}",
            errors[0].message
        );
    }
    assert!(alignments.wor_timings.is_none());
    assert!(alignments.gra.as_ref().expect("gra").is_error_free());
    Ok(())
}

/// `%wor` alignment is positional over matching counts and keeps the inline
/// bullets on the `%wor` words.
#[test]
fn wor_alignment_is_positional_and_keeps_inline_bullets() -> Result<(), TestError> {
    let mut utt = utterance_with(
        "eng",
        Media::Declared,
        "*CHI:\tone two .\n%wor:\tone \u{15}100_220\u{15} two .",
    )?;
    utt.compute_alignments(&ValidationContext::default());
    let alignments = utt.alignments.as_ref().expect("computed alignments");
    assert_eq!(
        alignments.wor_timings,
        Some(talkbank_model::alignment::WorTimingSidecar::Positional { count: 2 })
    );
    let bullets: Vec<Option<(u64, u64)>> = utt
        .dependent_tiers
        .iter()
        .filter_map(|dt| match &dt.tier {
            DependentTier::Wor(wor) => Some(wor.words()),
            _ => None,
        })
        .flatten()
        .map(|w| {
            w.inline_bullet
                .as_ref()
                .map(|b| (b.timing.start_ms, b.timing.end_ms))
        })
        .collect();
    assert_eq!(bullets, vec![Some((100, 220)), None]);
    Ok(())
}

/// `%sin` alignment units count an action (the `0` zero marker) on the main tier.
#[test]
fn sin_alignment_units_count_an_action() -> Result<(), TestError> {
    let utt = utterance("eng", "*CHI:\t0 word .\n%sin:\t0 0")?;
    let units = AlignmentUnits::from_utterance(&utt, &ValidationContext::default());
    assert_eq!(units.main_sin.len(), 2, "main_sin must count the action");
    assert_eq!(units.sin.len(), 2);
    Ok(())
}
