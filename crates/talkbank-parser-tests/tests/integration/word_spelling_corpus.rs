//! Reference-derived spelling and source-coordinate boundaries. Display text
//! derives from structure; source slices remain separately owned evidence.
use super::*;
use talkbank_model::alignment::helpers::{WordItem, walk_words};
use talkbank_model::model::{SemanticEq, Word};

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
    use talkbank_model::model::{WordCompoundMarker, WordContents, WordStressMarker};
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
                        WordContent::OverlapPoint(_)
                        | WordContent::CAElement(_)
                        | WordContent::CADelimiter(_)
                        | WordContent::Lengthening(_)
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
