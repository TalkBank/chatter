//! Internal mechanics exercised only on parsed canonical reference words.

use super::*;
use crate::pseudonymize::NameMap;
use std::collections::BTreeSet;
use talkbank_model::RuleSelection;
use talkbank_model::alignment::helpers::{WordItem, walk_words};
use talkbank_model::model::TranscriptName;
use talkbank_parser::TreeSitterParser;

#[test]
fn mapped_phonetic_word_inside_retrace_cannot_be_rewritten_as_orthography() {
    let parser = TreeSitterParser::new().unwrap();
    let map = NameMap::from_toml(
        "version = 1\n[[transcripts]]\nkey = 'sample'\n\
         [[transcripts.names]]\noriginal = 'tika'\nreplacement = 'PersonA'\n",
        &parser,
    )
    .unwrap();
    let names = map.for_transcript("sample").unwrap();
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/annotation/retrace.cha"
    ));
    let input = names
        .admit_document(
            source,
            TranscriptName::Anonymous,
            RuleSelection::new(),
            &parser,
        )
        .unwrap();
    let mut witnessed = 0;
    for utterance in input.document().document().utterances() {
        walk_words(&utterance.main.content.content, None, &mut |item| {
            let WordItem::ReplacedWord(replaced) = item else {
                return;
            };
            if replaced.word.raw_text() == "tika@u" {
                assert!(matches!(
                    WordPlan::assess(&replaced.word, names),
                    Err(WordRefusal::PhoneticMaterial)
                ));
                witnessed += 1;
            }
        });
    }
    assert_eq!(witnessed, 1);
}

#[test]
fn selective_components_preserve_structure_and_refuse_pronunciation_leaks() {
    let parser = TreeSitterParser::new().unwrap();
    let mut config = "version = 1\n[[transcripts]]\nkey = 'sample'\n".to_owned();
    for original in [
        "parce", "ice", "le", "hello", "no", "faster", "segment", "i",
    ] {
        config.push_str(&format!(
            "[[transcripts.names]]\noriginal = '{original}'\nreplacement = 'PersonA'\n"
        ));
    }
    let map = NameMap::from_toml(&config, &parser).unwrap();
    let names = map.for_transcript("sample").unwrap();
    let sources = [
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/content/shortenings-in-words.cha"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/content/words-markers.cha"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/ca/nonvocal-and-long-features.cha"
        )),
    ];
    let expected = [
        ("parc(e)", "PersonA", "parce"),
        ("ice+cream", "PersonA+cream", "ice"),
        ("le~ha", "PersonA~ha", "le"),
        ("ˈhello", "ˈPersonA", "hello"),
        ("no::", "PersonA::", "no"),
        ("∆faster∆", "∆PersonA∆", "faster"),
    ];
    let mut witnessed = BTreeSet::new();
    let mut refused = 0;
    let mut near_misses = 0;
    for source in sources {
        let input = names
            .admit_document(
                source,
                TranscriptName::Anonymous,
                RuleSelection::new(),
                &parser,
            )
            .unwrap();
        for utterance in input.document().document().utterances() {
            walk_words(&utterance.main.content.content, None, &mut |item| {
                let word = match item {
                    WordItem::Word(word) => word,
                    WordItem::ReplacedWord(_) | WordItem::Separator(_) => return,
                };
                let before = word.to_chat();
                let before_cleaned = word.cleaned_text().to_owned();
                let plan = WordPlan::assess(word, names);
                if word.raw_text() == "↫s-s-s↫segment" {
                    assert!(matches!(plan, Err(WordRefusal::RepeatedMaterial)));
                    refused += 1;
                    return;
                }
                let plan = plan.unwrap();
                let rewritten = plan.apply();
                assert_eq!(word.to_chat(), before, "source must remain immutable");
                assert_eq!(word.cleaned_text(), before_cleaned);
                if let Some(&(raw, output, matched)) =
                    expected.iter().find(|row| row.0 == word.raw_text())
                {
                    assert_eq!(rewritten.to_chat(), output, "{raw}");
                    assert_eq!(
                        rewritten.raw_text(),
                        output,
                        "cached wire text must follow typed content"
                    );
                    assert!(plan.decisions().any(|decision| matches!(decision,
                        ComponentDecision::Replace { original, replacement }
                            if original == matched && replacement.as_ref() == "PersonA")));
                    assert_eq!(rewritten.span, word.span);
                    assert_eq!(rewritten.inline_bullet, word.inline_bullet);
                    assert_eq!(rewritten.form_type, word.form_type);
                    assert_eq!(rewritten.lang, word.lang);
                    assert_eq!(rewritten.category, word.category);
                    let again = WordPlan::assess(&rewritten, names).unwrap();
                    assert!(
                        !again
                            .decisions()
                            .any(|decision| matches!(decision, ComponentDecision::Replace { .. }))
                    );
                    assert_eq!(again.apply().to_chat(), output);
                    witnessed.insert(raw);
                } else {
                    assert_eq!(rewritten.to_chat(), before);
                    if word.raw_text() == "I" {
                        assert!(plan.decisions().any(|decision| matches!(decision,
                            ComponentDecision::CaseNearMiss { original } if original == "I")));
                        near_misses += 1;
                    }
                }
            });
        }
    }
    assert_eq!(witnessed.len(), expected.len());
    assert_eq!(refused, 1);
    assert_eq!(near_misses, 1);
}
