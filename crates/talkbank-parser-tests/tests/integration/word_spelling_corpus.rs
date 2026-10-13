//! Reference-derived spelling and source-coordinate boundaries. Display text
//! derives from structure; source slices remain separately owned evidence.
use super::*;
use talkbank_model::alignment::helpers::{WordItem, walk_words};
use talkbank_model::model::{SemanticEq, Word};

/// Category, content order, form and alignment remain independent contracts.
#[test]
fn phonological_fragments_preserve_typed_features_source_and_tier_policy() {
    use talkbank_model::alignment::helpers::{TierDomain, counts_for_tier};
    use talkbank_model::model::{
        CADelimiterType, FormType, TranscriptName, WordCategory, WordContent,
    };
    use talkbank_model::{ChatParser, ErrorCollector, ParseOutcome};

    #[derive(Debug, PartialEq, Eq)]
    enum Element {
        Text,
        Lengthening,
        Repetition,
        Phonetic,
    }
    use Element::{Lengthening as L, Phonetic as P, Repetition as R, Text as T};
    let expected = [
        ("&+gr", vec![T], None),
        ("&+br:", vec![T, L], None),
        ("&+k:ra", vec![T, L, T], None),
        ("&+↫b-b↫ba", vec![R, T, R, T], None),
        ("&+tʃ@u", vec![P], Some(FormType::U)),
    ];
    let source = std::fs::read_to_string(
        workspace_root().join("corpus/reference/word-features/phonological-fragments.cha"),
    )
    .expect("authored fragment reference");
    let parser = TreeSitterParser::new().expect("parser");
    let mut file = strict_parse(parser.parse_chat_file(&source)).expect("clean fragment reference");
    let validation = ErrorCollector::new();
    file.validate_with_alignment(&validation, TranscriptName::Anonymous);
    assert!(validation.is_empty(), "{:?}", validation.to_vec());
    let mut words = Vec::new();
    for utterance in file.utterances() {
        walk_words(
            &utterance.main.content.content,
            None,
            &mut |item| match item {
                WordItem::Word(word) => words.push(word),
                WordItem::ReplacedWord(_) | WordItem::Separator(_) => {
                    panic!("unreplaced fragments")
                }
            },
        );
    }
    assert_eq!(words.len(), expected.len());
    for (word, (spelling, sequence, form)) in words.into_iter().zip(expected) {
        let actual: Vec<_> = word
            .content()
            .iter()
            .map(|item| match item {
                WordContent::Text(_) => T,
                WordContent::Lengthening(value) => {
                    assert_eq!(value.count.get(), 1);
                    L
                }
                WordContent::CADelimiter(value) => {
                    assert_eq!(value.delimiter_type, CADelimiterType::SegmentRepetition);
                    R
                }
                WordContent::Phonetic(value) => {
                    assert_eq!(value.as_ref(), "tʃ");
                    P
                }
                _ => panic!("unexpected fragment content: {item:?}"),
            })
            .collect();
        assert_eq!(actual, sequence, "{spelling}");
        assert_eq!(word.category, Some(WordCategory::PhonologicalFragment));
        assert_eq!(word.form_type, form);
        assert_eq!(word.lang, None);
        assert_eq!(word.part_of_speech, None);
        assert_eq!(
            source.get(word.span.start as usize..word.span.end as usize),
            Some(spelling)
        );
        for (domain, counts) in [
            (TierDomain::Mor, false),
            (TierDomain::Wor, false),
            (TierDomain::Pho, true),
            (TierDomain::Sin, true),
        ] {
            assert_eq!(
                counts_for_tier(word, domain),
                counts,
                "{spelling}: {domain:?}"
            );
        }
        let errors = ErrorCollector::new();
        let fragment = ChatParser::parse_word(&parser, spelling, word.span.start as usize, &errors);
        assert!(errors.is_empty(), "{:?}", errors.to_vec());
        let ParseOutcome::Parsed(fragment) = fragment else {
            panic!("fragment refused")
        };
        assert_eq!(fragment.span, word.span);
        assert_eq!(fragment.to_chat(), spelling);
        assert!(fragment.semantic_eq(word));
    }
    assert_eq!(file.to_chat_string(), source);
}

/// The invalid controls add exactly one annotation at a parsed word boundary.
#[test]
fn fragment_feature_specs_reject_only_the_added_replacement() {
    use talkbank_model::model::{TranscriptName, WordCategory};
    use talkbank_model::{ErrorCode, ErrorCollector};
    let parser = TreeSitterParser::new().expect("parser");
    let source_for = |index| {
        std::fs::read_to_string(workspace_root().join(format!(
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E387_fragment_features_{index}.cha",
    ))).expect("canonical fragment feature specimen")
    };
    for (control, mutation, annotation) in [
        (2, 1, " [: branch]"),
        (4, 3, " [: craft]"),
        (6, 5, " [: bay]"),
        (8, 7, " [: cheer]"),
    ] {
        let source = source_for(control);
        let mut file = strict_parse(parser.parse_chat_file(&source)).expect("clean control");
        let mut words = Vec::new();
        for utterance in file.utterances() {
            walk_words(
                &utterance.main.content.content,
                None,
                &mut |item| match item {
                    WordItem::Word(word) => words.push(word.clone()),
                    WordItem::ReplacedWord(_) | WordItem::Separator(_) => {
                        panic!("unreplaced control")
                    }
                },
            );
        }
        assert_eq!(words.len(), 1);
        let word = &words[0];
        assert_eq!(word.category, Some(WordCategory::PhonologicalFragment));
        let boundary = word.span.end as usize;
        let before = source.get(..boundary).expect("UTF-8 prefix");
        let after = source.get(boundary..).expect("UTF-8 suffix");
        let validation = ErrorCollector::new();
        file.validate_with_alignment(&validation, TranscriptName::Anonymous);
        assert!(validation.is_empty(), "{:?}", validation.to_vec());
        assert_eq!(file.to_chat_string(), source);
        let mutation_source = source_for(mutation);
        assert_eq!(
            format!("{before}{annotation}{after}"),
            mutation_source,
            "only the annotation is inserted: {mutation}"
        );
        let mut mutated =
            strict_parse(parser.parse_chat_file(&mutation_source)).expect("clean mutation parse");
        let mut produced = Vec::new();
        for utterance in mutated.utterances() {
            walk_words(
                &utterance.main.content.content,
                None,
                &mut |item| match item {
                    WordItem::ReplacedWord(replaced) => produced.push(replaced.word.clone()),
                    WordItem::Word(_) | WordItem::Separator(_) => panic!("one replaced fragment"),
                },
            );
        }
        assert_eq!(produced.len(), 1);
        assert!(produced[0].semantic_eq(word));
        assert_eq!(produced[0].span, word.span);
        let errors = ErrorCollector::new();
        mutated.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let diagnostics = errors.into_vec();
        assert_eq!(diagnostics.len(), 1, "{mutation}: {diagnostics:?}");
        assert_eq!(diagnostics[0].code, ErrorCode::ReplacementOnFragment);
        assert_eq!(mutated.to_chat_string(), mutation_source);
    }
}

/// Opaque suffix decoding must retain authored candidates, even the unknown
/// codes whose invalidity belongs to later registry validation. Marker JSON
/// and the public word-fragment boundary preserve that same spelling/order.
#[test]
fn language_candidate_suffix_specs_preserve_every_code_and_source_spelling() {
    use talkbank_model::model::WordLanguageMarker;
    use talkbank_model::{ChatParser, ErrorCollector, ParseOutcome};

    let parser = TreeSitterParser::new().expect("parser");
    for (spec, kind, codes) in [
        ("E519_3", "explicit", &["qzz"][..]),
        ("E519_4", "explicit", &["spa"][..]),
        ("E519_10", "ambiguous", &["spa", "fra"][..]),
        ("E519_11", "ambiguous", &["spa", "qzz"][..]),
        ("E519_12", "ambiguous", &["qzz", "fra"][..]),
        ("E519_13", "multiple", &["spa", "fra"][..]),
        ("E519_14", "multiple", &["spa", "qzz"][..]),
        ("E220_3", "multiple", &["eng", "zho"][..]),
        ("E220_4", "multiple", &["eng", "fra"][..]),
        ("E220_5", "ambiguous", &["eng", "zho"][..]),
        ("E220_6", "ambiguous", &["eng", "fra"][..]),
    ] {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/{spec}.cha",
        )))
        .expect("canonical language suffix specimen");
        let file = strict_parse(parser.parse_chat_file(&source)).expect("spec syntax parses");
        let mut marked = Vec::new();
        for utterance in file.utterances() {
            walk_words(&utterance.main.content.content, None, &mut |item| {
                let word = match item {
                    WordItem::Word(word) => word,
                    WordItem::ReplacedWord(replaced) => &replaced.word,
                    WordItem::Separator(_) => return,
                };
                if word.lang.is_some() {
                    marked.push(word);
                }
            });
        }
        assert_eq!(marked.len(), 1, "spec has exactly one suffix: {spec}");
        let word = marked[0];
        let marker = word.lang.as_ref().expect("authored suffix retained");
        let wire = match kind {
            "explicit" => serde_json::json!({"type": kind, "code": codes[0]}),
            "multiple" | "ambiguous" => serde_json::json!({"type": kind, "code": codes}),
            _ => panic!("unreviewed suffix shape"),
        };
        assert_eq!(
            serde_json::to_value(marker).expect("marker JSON"),
            wire,
            "{spec}"
        );
        let restored: WordLanguageMarker =
            serde_json::from_value(wire).expect("marker wire restores");
        assert_eq!(&restored, marker, "{spec}");
        let spelling = source
            .get(word.span.start as usize..word.span.end as usize)
            .expect("producing source range");
        assert_eq!(word.to_chat(), spelling, "{spec}");
        let errors = ErrorCollector::new();
        let fragment = ChatParser::parse_word(&parser, spelling, word.span.start as usize, &errors);
        assert!(errors.is_empty(), "{spec}: {:?}", errors.to_vec());
        let ParseOutcome::Parsed(fragment) = fragment else {
            panic!("authored suffix refused: {spec}");
        };
        assert_eq!(fragment.lang.as_ref(), Some(marker), "{spec}");
        assert_eq!(fragment.span, word.span, "{spec}");
        assert_eq!(fragment.to_chat(), spelling, "{spec}");
        assert_eq!(file.to_chat_string(), source, "{spec}");
    }
}

#[test]
fn canonical_word_spelling_is_derived_separately_from_source_spans() {
    let parser = TreeSitterParser::new().expect("parser");
    let mut observed_words = 0;
    let mut recovered_documents = 0;
    let mut source_differences = 0;
    for root in [
        "corpus/reference",
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors",
    ] {
        let corpus = ChatCorpus::read(&workspace_root().join(root)).expect("canonical corpus");
        for fixture in corpus.fixtures() {
            let errors = talkbank_model::ErrorCollector::new();
            let file = parser.parse_chat_file_streaming(fixture.source(), &errors);
            recovered_documents += usize::from(!errors.is_empty());
            // Observing retained words is not admission of a recovered file.
            for utterance in file.utterances() {
                walk_words(&utterance.main.content.content, None, &mut |item| {
                    let word = match item {
                        WordItem::Word(word) => word,
                        WordItem::ReplacedWord(replaced) => &replaced.word,
                        WordItem::Separator(_) => return,
                    };
                    let original = fixture
                        .source()
                        .get(word.span.start as usize..word.span.end as usize)
                        .expect("parser-produced word has a UTF-8 source range");
                    source_differences += usize::from(word.raw_text() != original);
                    assert_eq!(word.raw_text(), word.to_chat());
                    assert_eq!(
                        serde_json::to_value(word).expect("derived wire")["raw_text"],
                        word.to_chat()
                    );
                    observed_words += 1;
                });
            }
        }
    }
    assert!(observed_words > 0);
    assert!(
        recovered_documents > 0,
        "exercise recovery as well as valid controls"
    );
    assert!(
        source_differences > 0,
        "source slices are not derived word spelling"
    );
}

#[test]
fn reference_word_wire_preserves_recorded_spelling_without_source_coordinates() {
    use talkbank_model::model::{
        WordCompoundMarker, WordContents, WordLengthening, WordStressMarker,
    };
    use talkbank_model::model::{
        WordContent, WordLanguageMarker, WordPhonetic, WordShortening, WordText,
    };
    use talkbank_model::{ChatCleanedText, ChatRawText};
    macro_rules! assert_text_boundary {
        ($value:expr, $expected:expr, $alternative:expr, $path:expr) => {{
            let value = $value;
            let expected: &str = $expected;
            assert_eq!(AsRef::<str>::as_ref(&value), expected);
            assert_eq!(value.to_string(), expected);
            assert_eq!(
                serde_json::to_value(&value).expect("text JSON"),
                serde_json::Value::String(expected.to_owned())
            );
            assert_display_refusals(&value, $path);
            for candidate in [expected, $alternative] {
                let agrees = expected == candidate;
                let owned = candidate.to_owned();
                assert_eq!(<_ as PartialEq<str>>::eq(&value, candidate), agrees);
                assert_eq!(value == candidate, agrees);
                assert_eq!(value == owned, agrees);
                assert_eq!(<str as PartialEq<_>>::eq(candidate, &value), agrees);
                assert_eq!(candidate == value, agrees);
                assert_eq!(owned == value, agrees);
            }
        }};
    }
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut words = 0;
    let mut raw_clean_differences = 0;
    let mut annotation_witnesses = [0usize; 3];
    let mut language_witnesses = [0usize; 4];
    let mut text_leaf_witnesses = [0usize; 3];
    let mut structural_leaf_witnesses = [0usize; 2];
    let mut spelling_differences = std::collections::BTreeSet::new();
    for fixture in corpus.fixtures() {
        let file =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
        for utterance in file.utterances() {
            walk_words(&utterance.main.content.content, None, &mut |item| {
                let word = match item {
                    WordItem::Word(word) => word,
                    WordItem::ReplacedWord(replaced) => &replaced.word,
                    WordItem::Separator(_) => return,
                };
                let reconstructed: Word = serde_json::from_value(
                    serde_json::to_value(word).expect("serialize reference word"),
                )
                .expect("reconstruct producer-issued word JSON");
                assert!(reconstructed.span.is_dummy());
                assert_eq!(reconstructed.raw_text(), word.raw_text());
                assert!(reconstructed.semantic_eq(word));
                assert_eq!(reconstructed.to_chat_string(), word.to_chat_string());
                // Alignment consumers attach identifiers to existing words;
                // metadata must survive JSON without changing CHAT or spelling.
                let id = format!("word-{words}");
                let identified = word.clone().with_word_id(id.as_str());
                assert_eq!(identified.span, word.span);
                assert_eq!(identified.raw_text(), word.raw_text());
                assert_eq!(identified.cleaned_text(), word.cleaned_text());
                assert_eq!(identified.to_chat_string(), word.to_chat_string());
                let mut expected_wire = serde_json::to_value(word).expect("original wire");
                expected_wire["word_id"] = serde_json::json!(id);
                let wire = serde_json::to_value(&identified).expect("identified wire");
                assert_eq!(wire, expected_wire, "only the identifier may change");
                let decoded: Word =
                    serde_json::from_value(wire).expect("identified word roundtrip");
                assert_eq!(decoded.word_id.as_deref(), Some(id.as_str()));
                assert_eq!(decoded.to_chat_string(), word.to_chat_string());
                // Editor collection transfers retain typed leaves and source
                // spans; unlike JSON, they do not discard coordinates.
                let content = WordContents::from(word.content().to_vec());
                let mut transferred = WordContents::from(std::ops::Deref::deref(&content).clone());
                assert_eq!(transferred.is_empty(), word.content().is_empty());
                assert!(
                    (&mut transferred)
                        .into_iter()
                        .map(|leaf| &*leaf)
                        .eq(word.content().iter())
                );
                assert!(transferred.into_iter().eq(word.content().iter().cloned()));
                // Rebuild only annotations admitted by the parser. The builder
                // must preserve their wire representation and lexical content.
                let mut rebuilt = word.clone();
                // Prime the derived view before replacing admitted typed leaves.
                assert_eq!(rebuilt.cleaned_text(), word.cleaned_text());
                for (index, leaf) in word.content().iter().enumerate() {
                    let replacement = match leaf {
                        WordContent::Text(text) => {
                            text_leaf_witnesses[0] += 1;
                            WordContent::Text(
                                WordText::try_from(text.as_ref()).expect("admitted text"),
                            )
                        }
                        WordContent::Phonetic(text) => {
                            text_leaf_witnesses[1] += 1;
                            WordContent::Phonetic(
                                WordPhonetic::try_from(text.as_ref())
                                    .expect("admitted phonetic text"),
                            )
                        }
                        WordContent::Shortening(text) => {
                            text_leaf_witnesses[2] += 1;
                            WordContent::Shortening(
                                WordShortening::try_from(text.as_ref())
                                    .expect("admitted shortening"),
                            )
                        }
                        WordContent::StressMarker(marker) => {
                            let mut rebuilt_marker = WordStressMarker::new(marker.marker_type);
                            if let Some(span) = marker.span {
                                rebuilt_marker = rebuilt_marker.with_span(span);
                            }
                            structural_leaf_witnesses[0] += 1;
                            WordContent::StressMarker(rebuilt_marker)
                        }
                        WordContent::CompoundMarker(marker) => {
                            assert_eq!(marker.to_chat_string(), leaf.to_chat_string());
                            let mut rebuilt_marker = WordCompoundMarker::new();
                            if let Some(span) = marker.span {
                                rebuilt_marker = rebuilt_marker.with_span(span);
                            }
                            structural_leaf_witnesses[1] += 1;
                            WordContent::CompoundMarker(rebuilt_marker)
                        }
                        WordContent::Lengthening(marker) => {
                            let mut rebuilt_marker = if marker.count.get() == 1 {
                                WordLengthening::default()
                            } else {
                                WordLengthening::with_count(marker.count)
                            };
                            if let Some(span) = marker.span {
                                rebuilt_marker = rebuilt_marker.with_span(span);
                            }
                            WordContent::Lengthening(rebuilt_marker)
                        }
                        WordContent::OverlapPoint(_)
                        | WordContent::CAElement(_)
                        | WordContent::CADelimiter(_)
                        | WordContent::SyllablePause(_)
                        | WordContent::UnderlineBegin(_)
                        | WordContent::UnderlineEnd(_)
                        | WordContent::CliticBoundary(_) => continue,
                    };
                    assert_eq!(&replacement, leaf);
                    rebuilt.replace_content_at(index, replacement);
                }
                if let Some(category) = rebuilt.category.take() {
                    rebuilt = rebuilt.with_category(category);
                    annotation_witnesses[0] += 1;
                }
                if let Some(form) = rebuilt.form_type.take() {
                    rebuilt = rebuilt.with_form_type(form);
                    annotation_witnesses[1] += 1;
                }
                if let Some(pos) = rebuilt.part_of_speech.take() {
                    rebuilt = rebuilt.with_part_of_speech(pos);
                    annotation_witnesses[2] += 1;
                }
                if let Some(marker) = rebuilt.lang.take() {
                    assert_eq!(
                        marker.is_shortcut(),
                        matches!(marker, WordLanguageMarker::Shortcut)
                    );
                    match &marker {
                        WordLanguageMarker::Shortcut => {
                            assert!(marker.as_language().is_none());
                            assert!(marker.as_multiple().is_none());
                            assert!(marker.as_ambiguous().is_none());
                            rebuilt = rebuilt.with_language_shortcut();
                            language_witnesses[0] += 1;
                        }
                        WordLanguageMarker::Explicit(code) => {
                            assert_eq!(marker.as_language(), Some(code));
                            assert!(marker.as_multiple().is_none());
                            assert!(marker.as_ambiguous().is_none());
                            rebuilt = rebuilt.with_lang(code.clone());
                            language_witnesses[1] += 1;
                        }
                        WordLanguageMarker::Multiple(codes) => {
                            assert_eq!(marker.as_language(), codes.first());
                            assert_eq!(marker.as_multiple(), Some(codes.as_slice()));
                            assert!(marker.as_ambiguous().is_none());
                            rebuilt.lang = Some(WordLanguageMarker::multiple(codes.clone()));
                            language_witnesses[2] += 1;
                        }
                        WordLanguageMarker::Ambiguous(codes) => {
                            assert_eq!(marker.as_language(), codes.first());
                            assert!(marker.as_multiple().is_none());
                            assert_eq!(marker.as_ambiguous(), Some(codes.as_slice()));
                            rebuilt.lang = Some(WordLanguageMarker::ambiguous(codes.clone()));
                            language_witnesses[3] += 1;
                        }
                    }
                    assert_eq!(rebuilt.lang.as_ref(), Some(&marker));
                }
                assert_eq!(rebuilt.cleaned_text(), word.cleaned_text());
                assert_eq!(rebuilt.to_chat_string(), word.to_chat_string());
                assert_eq!(
                    serde_json::to_value(&rebuilt).expect("rebuilt word JSON"),
                    serde_json::to_value(word).expect("parsed word JSON")
                );
                let raw = ChatRawText::from_word_raw(word);
                let cleaned = ChatCleanedText::from_word(word);
                assert_eq!(cleaned.chars().collect::<String>(), word.cleaned_text());
                assert_eq!(cleaned.to_lowercase(), word.cleaned_text().to_lowercase());
                assert_text_boundary!(raw, &word.raw_text(), word.cleaned_text(), fixture.path());
                assert_text_boundary!(
                    cleaned,
                    word.cleaned_text(),
                    &word.raw_text(),
                    fixture.path()
                );
                raw_clean_differences += usize::from(word.raw_text() != word.cleaned_text());
                if word.raw_text() != word.to_chat_string() {
                    spelling_differences
                        .insert((word.raw_text().to_owned(), word.to_chat_string()));
                }
                words += 1;
            });
        }
    }
    assert!(words > 0, "reference words must exercise the boundary");
    assert!(
        annotation_witnesses.into_iter().all(|count| count > 0),
        "category, form and POS builders need corpus witnesses: {annotation_witnesses:?}"
    );
    assert!(
        language_witnesses.into_iter().all(|count| count > 0),
        "all four language marker forms need corpus witnesses: {language_witnesses:?}"
    );
    assert!(
        text_leaf_witnesses.into_iter().all(|count| count > 0),
        "all three typed text leaves need corpus witnesses: {text_leaf_witnesses:?}"
    );
    assert!(
        structural_leaf_witnesses.into_iter().all(|count| count > 0),
        "stress and compound markers need corpus witnesses: {structural_leaf_witnesses:?}"
    );
    assert!(WordText::try_from("").is_err());
    assert!(WordPhonetic::try_from("").is_err());
    assert!(WordShortening::try_from("").is_err());
    assert!(
        raw_clean_differences > 0,
        "raw and cleaned views must have distinct corpus witnesses"
    );
    eprintln!(
        "reference word observations: {words}; distinct raw/canonical differences: {}",
        spelling_differences.len()
    );
    for (raw, canonical) in spelling_differences {
        eprintln!("raw={raw:?}; canonical={canonical:?}");
    }
}

/// Ordered combinations need stronger witnesses than a set of content kinds.
/// This fixture is independently authored; existing full-reference controls
/// also exercise its source, wire and builder boundaries.
#[test]
fn shortening_combinations_preserve_typed_order_suffixes_and_replacement_scope() {
    use talkbank_model::model::{FormType, TranscriptName, WordContent, WordLanguageMarker};
    use talkbank_model::{ErrorCollector, Severity};

    #[derive(Debug, PartialEq, Eq)]
    enum Element {
        Text,
        Shortening,
        Lengthening,
    }
    use Element::{Lengthening as L, Shortening as S, Text as T};
    let expected = [
        ("ca(f)é(s)", vec![T, S, T, S], None, false, false),
        ("(a)mi(go)", vec![S, T, S], None, false, false),
        ("(a)mi(g)o", vec![S, T, S, T], None, false, false),
        ("ca(f)é(s)ito", vec![T, S, T, S, T], None, false, false),
        ("chir(p)@o", vec![T, S], Some(FormType::O), false, false),
        ("(bu)ba@b", vec![S, T], Some(FormType::B), false, false),
        ("ca(f)é@c", vec![T, S, T], Some(FormType::C), false, false),
        ("pla(n)@s", vec![T, S], None, true, false),
        ("(a)migo@s", vec![S, T], None, true, false),
        ("ami(g)o@s", vec![T, S, T], None, true, false),
        ("chir(p)@o", vec![T, S], Some(FormType::O), false, true),
        ("ca(f)é", vec![T, S, T], None, false, true),
        ("(a)migo", vec![S, T], None, false, true),
        ("ca:(fé)", vec![T, L, S], None, false, false),
        ("(a)ga:in", vec![S, T, L, T], None, false, false),
        ("ca(f)é:", vec![T, S, T, L], None, false, false),
    ];
    let source = std::fs::read_to_string(
        workspace_root().join("corpus/reference/word-features/shortening-combinations.cha"),
    )
    .expect("canonical shortening reference");
    let parser = TreeSitterParser::new().expect("parser");
    let mut file = strict_parse(parser.parse_chat_file(&source)).expect("clean strict parse");
    let validation = ErrorCollector::new();
    file.validate_with_alignment(&validation, TranscriptName::Anonymous);
    assert!(
        validation
            .to_vec()
            .iter()
            .all(|d| d.severity != Severity::Error),
        "valid reference: {:?}",
        validation.to_vec()
    );
    let mut observed = Vec::new();
    for utterance in file.utterances() {
        walk_words(
            &utterance.main.content.content,
            None,
            &mut |item| match item {
                WordItem::Word(word) => observed.push((word, false)),
                WordItem::ReplacedWord(replaced) => observed.push((&replaced.word, true)),
                WordItem::Separator(_) => panic!("authored reference has no separators"),
            },
        );
    }
    assert_eq!(observed.len(), expected.len());
    for ((word, replaced), (spelling, sequence, form, language, replacement)) in
        observed.into_iter().zip(expected)
    {
        let actual: Vec<Element> = word
            .content()
            .iter()
            .map(|item| match item {
                WordContent::Text(_) => T,
                WordContent::Shortening(_) => S,
                WordContent::Lengthening(v) => {
                    assert_eq!(v.count.get(), 1);
                    L
                }
                WordContent::Phonetic(_)
                | WordContent::OverlapPoint(_)
                | WordContent::CAElement(_)
                | WordContent::CADelimiter(_)
                | WordContent::StressMarker(_)
                | WordContent::SyllablePause(_)
                | WordContent::UnderlineBegin(_)
                | WordContent::UnderlineEnd(_)
                | WordContent::CompoundMarker(_)
                | WordContent::CliticBoundary(_) => {
                    panic!("unexpected content in authored shortening control: {item:?}")
                }
            })
            .collect();
        assert_eq!(actual, sequence, "{spelling}");
        assert_eq!(word.form_type, form, "{spelling}");
        assert_eq!(
            word.lang.as_ref(),
            language.then_some(&WordLanguageMarker::Shortcut),
            "{spelling}"
        );
        assert_eq!(replaced, replacement, "{spelling}");
        assert_eq!(word.category, None);
        assert_eq!(word.part_of_speech, None);
        assert_eq!(
            source.get(word.span.start as usize..word.span.end as usize),
            Some(spelling)
        );
    }
    assert_eq!(
        file.to_chat_string(),
        source,
        "source spelling and annotations survive together"
    );
}

/// Suffix decoding is separate from spoken-content validity: the legal
/// controls above and these mutations retain the same suffix types.
#[test]
fn shortening_suffix_specs_do_not_supply_missing_spoken_material() {
    use talkbank_model::model::{FormType, TranscriptName, WordContent, WordLanguageMarker};
    use talkbank_model::{ErrorCode, ErrorCollector};
    let parser = TreeSitterParser::new().expect("parser");
    for (index, spelling, form, language) in [
        (6, "(chirp)@o", Some(FormType::O), false),
        (7, "(amigo)@s", None, true),
    ] {
        let source = std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E209_shortening_combinations_{index}.cha",
        ))).expect("canonical suffix-only mutation");
        let mut file =
            strict_parse(parser.parse_chat_file(&source)).expect("suffix mutation parses cleanly");
        let utterance = file.utterances().next().expect("one authored utterance");
        let mut words = Vec::new();
        walk_words(
            &utterance.main.content.content,
            None,
            &mut |item| match item {
                WordItem::Word(word) => {
                    if word
                        .content()
                        .iter()
                        .any(|item| matches!(item, WordContent::Shortening(_)))
                    {
                        words.push(word);
                    }
                }
                WordItem::ReplacedWord(_) | WordItem::Separator(_) => {
                    panic!("one ordinary shortened word")
                }
            },
        );
        assert_eq!(words.len(), 1);
        let word = words[0];
        assert!(matches!(
            word.content().as_slice(),
            [WordContent::Shortening(_)]
        ));
        assert_eq!(word.form_type, form);
        assert_eq!(
            word.lang.as_ref(),
            language.then_some(&WordLanguageMarker::Shortcut)
        );
        assert_eq!(
            source.get(word.span.start as usize..word.span.end as usize),
            Some(spelling)
        );
        let validation = ErrorCollector::new();
        file.validate_with_alignment(&validation, TranscriptName::Anonymous);
        let diagnostics = validation.into_vec();
        assert_eq!(
            diagnostics.len(),
            1,
            "no suffix recovery or unrelated finding: {diagnostics:?}"
        );
        assert_eq!(diagnostics[0].code, ErrorCode::EmptySpokenContent);
    }
}

/// Attachment, order and byte coordinates are parser/source boundary contracts,
/// not inferred from a set of marker kinds. Whole-reference wire and builder
/// controls consume this independently authored fixture as well.
#[test]
fn ca_boundary_combinations_preserve_typed_order_and_source_spelling() {
    use talkbank_model::model::{CADelimiterType, CAElementType, TranscriptName, WordContent};
    use talkbank_model::{ChatParser, ErrorCollector, ParseOutcome};

    #[derive(Debug, PartialEq, Eq)]
    enum Element {
        Text,
        CA(CAElementType),
        Delimiter(CADelimiterType),
        Overlap,
        UnderlineBegin,
        UnderlineEnd,
        Lengthening,
    }
    use CADelimiterType::{Creaky, Faster, HighPitch, Slower, SmileVoice, Softer, Unsure};
    use CAElementType::{Inhalation, PitchDown, PitchUp};
    use Element::Delimiter as D;
    use Element::{CA, Lengthening as L, Overlap as O, Text as T};
    use Element::{UnderlineBegin as B, UnderlineEnd as E};
    let expected = [
        ("⌈garden", "garden", vec![O, T]),
        ("∙café", "café", vec![CA(Inhalation), T]),
        ("\u{2}\u{1}sun\u{2}\u{2}rise", "sunrise", vec![B, T, E, T]),
        (
            "moon\u{2}\u{1}light\u{2}\u{2}",
            "moonlight",
            vec![T, B, T, E],
        ),
        ("↑harbor", "harbor", vec![CA(PitchUp), T]),
        ("lantern↓", "lantern", vec![T, CA(PitchDown)]),
        ("↓meadow", "meadow", vec![CA(PitchDown), T]),
        ("⌈∙willow", "willow", vec![O, CA(Inhalation), T]),
        (
            "wa\u{2}\u{1}ve\u{2}\u{2}:let",
            "wavelet",
            vec![T, B, T, E, L, T],
        ),
        ("ri\u{2}\u{1}:\u{2}\u{2}ver", "river", vec![T, B, L, E, T]),
        (
            "\u{2}\u{1}sea\u{2}\u{2}:side",
            "seaside",
            vec![B, T, E, L, T],
        ),
        (
            "↑ra\u{2}\u{1}in\u{2}\u{2}bow",
            "rainbow",
            vec![CA(PitchUp), T, B, T, E, T],
        ),
    ];
    let delimiter_expected = [
        ("°silver", "silver", vec![D(Softer), T]),
        ("lining°", "lining", vec![T, D(Softer)]),
        ("∆rolling", "rolling", vec![D(Faster), T]),
        ("stone∆", "stone", vec![T, D(Faster)]),
        ("⁇hidden", "hidden", vec![D(Unsure), T]),
        ("trail⁇", "trail", vec![T, D(Unsure)]),
        ("⁎velvet", "velvet", vec![D(Creaky), T]),
        ("moss⁎", "moss", vec![T, D(Creaky)]),
        ("∇gentle", "gentle", vec![D(Slower), T]),
        ("current∇", "current", vec![T, D(Slower)]),
        ("☺sunlit", "sunlit", vec![D(SmileVoice), T]),
        ("garden☺", "garden", vec![T, D(SmileVoice)]),
        ("▔higher", "higher", vec![D(HighPitch), T]),
        ("ridge▔", "ridge", vec![T, D(HighPitch)]),
        ("⌈°pearl", "pearl", vec![O, D(Softer), T]),
        ("mist°⌉", "mist", vec![T, D(Softer), O]),
        ("°ca:fé°", "café", vec![D(Softer), T, L, T, D(Softer)]),
        ("°tea:°", "tea", vec![D(Softer), T, L, D(Softer)]),
    ];
    let parser = TreeSitterParser::new().expect("parser");
    for (fixture, expected) in [
        ("word-boundary-combinations.cha", expected.as_slice()),
        ("delimiter-stretches.cha", delimiter_expected.as_slice()),
    ] {
        let source =
            std::fs::read_to_string(workspace_root().join("corpus/reference/ca").join(fixture))
                .expect("canonical CA boundary reference");
        let mut file = strict_parse(parser.parse_chat_file(&source)).expect("clean strict parse");
        let validation = ErrorCollector::new();
        file.validate_with_alignment(&validation, TranscriptName::Anonymous);
        assert!(
            validation.is_empty(),
            "valid reference: {:?}",
            validation.to_vec()
        );
        let mut observed = Vec::new();
        for utterance in file.utterances() {
            walk_words(
                &utterance.main.content.content,
                None,
                &mut |item| match item {
                    WordItem::Word(word) => observed.push(word),
                    WordItem::ReplacedWord(_) | WordItem::Separator(_) => {
                        panic!("authored reference contains only ordinary words")
                    }
                },
            );
        }
        assert_eq!(observed.len(), expected.len());
        for (word, (spelling, cleaned, sequence)) in observed.into_iter().zip(expected) {
            let actual: Vec<_> = word
                .content()
                .iter()
                .map(|item| match item {
                    WordContent::Text(_) => T,
                    WordContent::CAElement(value) => CA(value.element_type),
                    WordContent::CADelimiter(value) => D(value.delimiter_type),
                    WordContent::OverlapPoint(_) => O,
                    WordContent::UnderlineBegin(_) => B,
                    WordContent::UnderlineEnd(_) => E,
                    WordContent::Lengthening(value) => {
                        assert_eq!(value.count.get(), 1);
                        L
                    }
                    WordContent::Phonetic(_)
                    | WordContent::Shortening(_)
                    | WordContent::StressMarker(_)
                    | WordContent::SyllablePause(_)
                    | WordContent::CompoundMarker(_)
                    | WordContent::CliticBoundary(_) => {
                        panic!("unexpected content in authored CA control: {item:?}")
                    }
                })
                .collect();
            assert_eq!(&actual, sequence, "{spelling:?}");
            assert_eq!(word.cleaned_text(), *cleaned, "{spelling:?}");
            assert_eq!(word.category, None);
            assert_eq!(word.form_type, None);
            assert_eq!(word.lang, None);
            assert_eq!(word.part_of_speech, None);
            assert_eq!(
                source.get(word.span.start as usize..word.span.end as usize),
                Some(*spelling)
            );
            let errors = ErrorCollector::new();
            let fragment =
                ChatParser::parse_word(&parser, spelling, word.span.start as usize, &errors);
            assert!(errors.is_empty(), "fragment: {:?}", errors.to_vec());
            let ParseOutcome::Parsed(fragment) = fragment else {
                panic!("authored word refused: {spelling:?}");
            };
            assert_eq!(fragment.span, word.span);
            assert_eq!(fragment.to_chat(), *spelling);
            assert!(fragment.semantic_eq(word));
        }
        assert_eq!(file.to_chat_string(), source);
    }
}

/// Pairing is utterance-wide and independent by delimiter type. Mutations
/// must remove only the reviewed typed marker span, never lexical material.
#[test]
fn ca_stretch_specs_pair_types_and_delete_only_one_source_bound_marker() {
    use talkbank_model::model::{CADelimiterType, TranscriptName, WordContent};
    use talkbank_model::{ErrorCode, ErrorCollector};

    let parser = TreeSitterParser::new().expect("parser");
    let source_for = |index| {
        std::fs::read_to_string(workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E230_delimiter_stretches_{index}.cha",
        ))).expect("canonical paired-stretch specimen")
    };
    use CADelimiterType::{Creaky, Faster, HighPitch, Slower, SmileVoice, Softer, Unsure};
    for (control, types, deletions) in [
        (1, vec![Softer, Softer], vec![(2, 1), (3, 0)]),
        (4, vec![Faster, Faster], vec![(5, 1), (6, 0)]),
        (7, vec![Unsure, Unsure], vec![(8, 1), (9, 0)]),
        (10, vec![Creaky, Creaky], vec![(11, 1), (12, 0)]),
        (13, vec![Slower, Slower], vec![(14, 1), (15, 0)]),
        (16, vec![SmileVoice, SmileVoice], vec![(17, 1), (18, 0)]),
        (19, vec![HighPitch, HighPitch], vec![(20, 1), (21, 0)]),
        (
            22,
            vec![Softer, Faster, Softer, Faster],
            vec![(23, 3), (24, 2)],
        ),
        (25, vec![Softer, Softer], vec![(26, 1)]),
    ] {
        let source = source_for(control);
        let mut file = strict_parse(parser.parse_chat_file(&source)).expect("clean paired control");
        let mut markers = Vec::new();
        for utterance in file.utterances() {
            walk_words(
                &utterance.main.content.content,
                None,
                &mut |item| match item {
                    WordItem::Word(word) => {
                        for content in word.content() {
                            if let WordContent::CADelimiter(marker) = content {
                                markers.push((
                                    marker.delimiter_type,
                                    marker.span.expect("parsed marker location"),
                                ));
                            }
                        }
                    }
                    WordItem::ReplacedWord(_) | WordItem::Separator(_) => {
                        panic!("ordinary-word control")
                    }
                },
            );
        }
        assert_eq!(
            markers.iter().map(|(kind, _)| *kind).collect::<Vec<_>>(),
            types
        );
        let validation = ErrorCollector::new();
        file.validate_with_alignment(&validation, TranscriptName::Anonymous);
        assert!(
            validation.is_empty(),
            "control {control}: {:?}",
            validation.to_vec()
        );
        assert_eq!(file.to_chat_string(), source);
        for (mutation, ordinal) in deletions {
            let (kind, span) = markers[ordinal];
            let start = span.start as usize;
            let end = span.end as usize;
            assert_eq!(source.get(start..end), Some(kind.to_symbol()));
            let before = source.get(..start).expect("UTF-8 prefix");
            let after = source.get(end..).expect("UTF-8 suffix");
            let deleted = format!("{before}{after}");
            let mutation_source = source_for(mutation);
            assert_eq!(
                deleted, mutation_source,
                "only the typed marker is removed: {mutation}"
            );
            let mut mutated = strict_parse(parser.parse_chat_file(&mutation_source))
                .expect("clean mutation parse");
            let errors = ErrorCollector::new();
            mutated.validate_with_alignment(&errors, TranscriptName::Anonymous);
            let diagnostics = errors.into_vec();
            assert_eq!(
                diagnostics.len(),
                1,
                "one unpaired occurrence: {mutation}: {diagnostics:?}"
            );
            assert_eq!(diagnostics[0].code, ErrorCode::UnbalancedCADelimiter);
            assert_eq!(mutated.to_chat_string(), mutation_source);
        }
    }
}

/// Validity belongs to the underline policy, independently of successful
/// pitch decoding. Both deletion mutations retain their authored source.
#[test]
fn ca_pitch_does_not_supply_an_underline_boundary() {
    use talkbank_model::model::{CAElementType, TranscriptName, WordContent};
    use talkbank_model::{ErrorCode, ErrorCollector};

    let parser = TreeSitterParser::new().expect("parser");
    for (spec, code) in [
        (
            "E356_ca_word_boundaries",
            ErrorCode::UnmatchedUnderlineBegin,
        ),
        ("E357_ca_word_boundaries", ErrorCode::UnmatchedUnderlineEnd),
    ] {
        for index in 1..=2 {
            let source = std::fs::read_to_string(workspace_root().join(format!(
                "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/{spec}_{index}.cha",
            ))).expect("canonical CA underline specimen");
            let mut file =
                strict_parse(parser.parse_chat_file(&source)).expect("clean strict parse");
            let utterance = file.utterances().next().expect("one utterance");
            let mut words = Vec::new();
            walk_words(
                &utterance.main.content.content,
                None,
                &mut |item| match item {
                    WordItem::Word(word) => words.push(word),
                    WordItem::ReplacedWord(_) | WordItem::Separator(_) => {
                        panic!("one ordinary word")
                    }
                },
            );
            assert_eq!(words.len(), 1);
            let word = words[0];
            assert!(
                matches!(word.content().first(), Some(WordContent::CAElement(value))
                if value.element_type == CAElementType::PitchUp)
            );
            assert_eq!(word.cleaned_text(), "rainbow");
            assert_eq!(
                source.get(word.span.start as usize..word.span.end as usize),
                Some(word.to_chat().as_str())
            );
            let validation = ErrorCollector::new();
            file.validate_with_alignment(&validation, TranscriptName::Anonymous);
            let diagnostics = validation.into_vec();
            if index == 1 {
                assert!(diagnostics.is_empty(), "legal control: {diagnostics:?}");
            } else {
                assert_eq!(
                    diagnostics.len(),
                    1,
                    "one boundary mutation: {diagnostics:?}"
                );
                assert_eq!(diagnostics[0].code, code);
            }
            assert_eq!(file.to_chat_string(), source);
        }
    }
}
