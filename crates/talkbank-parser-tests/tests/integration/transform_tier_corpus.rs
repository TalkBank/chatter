//! Distinct-payload regeneration contracts using only parsed reference tiers.
#![allow(clippy::expect_used, clippy::panic)]

use talkbank_model::model::{DependentTier, SemanticEq, TranscriptName, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{chat_corpus::ChatCorpus, test_error::strict_parse};

/// Public consumer policy: select the first stored payload, preserve identity
/// for borrowed access, and distinguish absent tiers from empty payloads.
#[test]
fn reference_tier_access_preserves_payload_identity_and_absence() {
    use std::collections::BTreeMap;
    use talkbank_model::ErrorCollector;

    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut witnesses = BTreeMap::<&str, (usize, usize)>::new();
    for fixture in corpus.fixtures() {
        let parsed =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
        let admitted = parsed
            .validate_into(&ErrorCollector::new(), TranscriptName::Anonymous)
            .expect("tier consumers require a valid reference document");
        let file = admitted.document();
        // Parsing/validation and serialization have their own normalization
        // contract. Here isolate whether tier consumption changes the document.
        let before = file.to_chat_string();
        for utterance in file.utterances() {
            macro_rules! check_access {
                ($variant:ident, $method:ident $(, $projection:ident)?) => {{
                    let expected = utterance.dependent_tiers.iter().filter_map(|entry| {
                        match &entry.tier {
                            DependentTier::$variant(tier) => Some(tier$(.$projection())?),
                            _ => None,
                        }
                    }).next();
                    let actual = utterance.$method();
                    let counts = witnesses.entry(stringify!($method)).or_default();
                    match (actual, expected) {
                        (Some(actual), Some(expected)) => {
                            assert!(std::ptr::eq(actual, expected),
                                "{}: {} must borrow the first stored payload",
                                fixture.path().display(), stringify!($method));
                            counts.0 += 1;
                        }
                        (None, None) => counts.1 += 1,
                        _ => panic!("{}: {} confused presence and absence",
                            fixture.path().display(), stringify!($method)),
                    }
                }};
            }
            check_access!(Mor, mor_tier);
            check_access!(Gra, gra_tier);
            check_access!(Wor, wor_tier);
            check_access!(Pho, pho_tier);
            check_access!(Mod, mod_tier);
            check_access!(Sin, sin_tier);
            check_access!(Act, act);
            check_access!(Cod, cod);
            check_access!(Com, com);
            check_access!(Exp, exp);
            check_access!(Add, add);
            check_access!(Spa, spa);
            check_access!(Sit, sit);
            check_access!(Gpx, gpx);
            check_access!(Int, int);
            check_access!(Modsyl, modsyl_tier);
            check_access!(Phosyl, phosyl_tier);
            check_access!(Phoaln, phoaln_tier);
            check_access!(Xphoint, xphoint_tier);
            check_access!(Ort, ort, as_str);
            check_access!(Eng, eng, as_str);
            check_access!(Gls, gls, as_str);
            check_access!(Alt, alt, as_str);
            check_access!(Coh, coh, as_str);
            check_access!(Def, def, as_str);
            check_access!(Err, err, as_str);
            check_access!(Fac, fac, as_str);
            check_access!(Flo, flo, as_str);
            check_access!(Par, par, as_str);
            check_access!(Tim, tim, as_str);
            assert_eq!(utterance.pho().as_ref(), utterance.pho_tier());
            assert_eq!(utterance.sin().as_ref(), utterance.sin_tier());
        }
        assert_eq!(
            file.to_chat_string(),
            before,
            "tier consumption must preserve the reference document"
        );
    }
    assert_eq!(
        witnesses.len(),
        30,
        "every selected accessor needs corpus evidence"
    );
    for (method, (present, absent)) in witnesses {
        assert!(
            present > 0 && absent > 0,
            "reference corpus must witness both outcomes of {method}: present={present}, absent={absent}"
        );
    }
}

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
