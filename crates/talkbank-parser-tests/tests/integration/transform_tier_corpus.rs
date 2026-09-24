//! Distinct-payload regeneration contracts using only parsed reference tiers.
#![allow(clippy::expect_used, clippy::panic)]

use talkbank_model::model::{DependentTier, SemanticEq};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{chat_corpus::ChatCorpus, test_error::strict_parse};

/// These are the API's declared conservative hints, not linguistic gold labels.
#[test]
fn reference_pos_hints_preserve_the_declared_clan_to_ud_contract() {
    use talkbank_model::alignment::helpers::{WordItem, walk_words};
    use talkbank_model::model::dependent_tier::mor::clan_to_ud_upos;
    use talkbank_parser_tests::repo_paths::workspace_root;

    let source = std::fs::read_to_string(
        workspace_root().join("corpus/reference/word/pos-hint-vocabulary.cha"),
    )
    .expect("authored POS-hint reference");
    let parser = TreeSitterParser::new().expect("parser");
    let file = strict_parse(parser.parse_chat_file(&source)).expect("reference parses");
    let mut hints = Vec::new();
    for utterance in file.utterances() {
        walk_words(&utterance.main.content.content, None, &mut |item| {
            let word = match item {
                WordItem::Word(word) => word,
                WordItem::ReplacedWord(replaced) => &replaced.word,
                WordItem::Separator(_) => return,
            };
            let tag = word
                .part_of_speech
                .as_deref()
                .expect("reference word carries POS hint");
            hints.push((tag.to_owned(), clan_to_ud_upos(tag)));
        });
    }
    let expected = [
        ("n", Some("NOUN")),
        ("v", Some("VERB")),
        ("adj", Some("ADJ")),
        ("adv", Some("ADV")),
        ("pro:per", Some("PRON")),
        ("det:art", Some("DET")),
        ("prep", Some("ADP")),
        ("post", Some("ADP")),
        ("conj", Some("CCONJ")),
        ("comp", Some("SCONJ")),
        ("part", Some("PART")),
        ("mod", Some("AUX")),
        ("aux", Some("AUX")),
        ("qn", Some("DET")),
        ("num", Some("NUM")),
        ("co", Some("INTJ")),
        ("int", Some("INTJ")),
        ("intj", Some("INTJ")),
        ("sym", Some("SYM")),
        ("punct", Some("PUNCT")),
        ("cm", Some("PUNCT")),
        ("end", Some("PUNCT")),
        ("beg", Some("PUNCT")),
        ("n:prop", Some("PROPN")),
        ("n:gerund", Some("NOUN")),
        ("zzzunknown", None),
    ];
    assert_eq!(
        hints
            .iter()
            .map(|(tag, hint)| (tag.as_str(), *hint))
            .collect::<Vec<_>>(),
        expected
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RegenerationKey<'a> {
    Mor,
    Gra,
    Wor,
    UserDefined(&'a str),
}

impl<'a> RegenerationKey<'a> {
    fn from_tier(tier: &'a DependentTier) -> Option<Self> {
        match tier {
            DependentTier::Mor(_) => Some(Self::Mor),
            DependentTier::Gra(_) => Some(Self::Gra),
            DependentTier::Wor(_) => Some(Self::Wor),
            DependentTier::UserDefined(tier) => Some(Self::UserDefined(tier.label.as_str())),
            _ => None,
        }
    }

    fn family(self) -> usize {
        match self {
            Self::Mor => 0,
            Self::Gra => 1,
            Self::Wor => 2,
            Self::UserDefined(_) => 3,
        }
    }
}

#[test]
fn distinct_reference_payload_replaces_only_its_matching_entry() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("canonical reference corpus");
    let files: Vec<_> = corpus
        .fixtures()
        .iter()
        .map(|fixture| {
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses")
        })
        .collect();
    let donors: Vec<_> = files
        .iter()
        .flat_map(|file| file.utterances())
        .flat_map(|utterance| utterance.dependent_tiers.iter())
        .filter_map(|entry| RegenerationKey::from_tier(&entry.tier).map(|key| (key, &entry.tier)))
        .collect();
    let mut witnessed = [false; 4];
    for file in &files {
        for utterance in file.utterances() {
            for (index, entry) in utterance.dependent_tiers.iter().enumerate() {
                let Some(key) = RegenerationKey::from_tier(&entry.tier) else {
                    continue;
                };
                let Some((_, donor)) = donors
                    .iter()
                    .find(|(candidate, tier)| *candidate == key && !tier.semantic_eq(&entry.tier))
                else {
                    continue;
                };
                // Independently state the result: one payload changes; every
                // other payload, position and source separator stays identical.
                let mut expected = utterance.dependent_tiers.clone();
                expected[index].tier = (*donor).clone();
                let mut actual = utterance.dependent_tiers.clone();
                talkbank_transform::dependent_tiers::replace_or_add_tier(
                    &mut actual,
                    (*donor).clone(),
                );
                assert_eq!(
                    actual, expected,
                    "replacement must change exactly the selected payload"
                );
                witnessed[key.family()] = true;
            }
        }
    }
    assert!(
        witnessed.into_iter().all(|seen| seen),
        "distinct donor required for every regeneration family: {witnessed:?}"
    );
}
