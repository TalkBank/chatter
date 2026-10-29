//! Distinct-payload regeneration contracts using only parsed reference tiers.
#![allow(clippy::expect_used, clippy::panic)]

use talkbank_model::model::{DependentTier, SemanticEq, TranscriptName, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{chat_corpus::ChatCorpus, test_error::strict_parse};

#[test]
fn authored_custom_form_preserves_both_parser_wire_and_schema_admission() {
    fn check(parser: &impl talkbank_model::ChatParser, source: &str) {
        use talkbank_model::{ErrorCollector, FormType, UtteranceContent, Word};
        let schema = serde_json::to_value(schemars::schema_for!(Word)).expect("word schema");
        let validator = jsonschema::validator_for(&schema).expect("word validator");
        for origin in [0, 59] {
            let errors = ErrorCollector::new();
            let main = parser
                .parse_main_tier(source, origin, &errors)
                .into_option()
                .expect("authored custom form");
            assert!(errors.is_empty(), "{:?}", errors.to_vec());
            let UtteranceContent::Word(word) = &main.content.content[0] else {
                panic!("authored first word");
            };
            let Some(FormType::UserDefined(label)) = word.form_type() else {
                panic!("declared custom label");
            };
            assert_eq!(label.as_str(), "m");
            let span = word.location.span().expect("located word");
            assert_eq!(
                &source[span.start() - origin..span.end() - origin],
                word.to_chat()
            );
            let original = serde_json::to_value(word).expect("word JSON");
            assert_eq!(original["form_type"], serde_json::json!({ "z": "m" }));
            assert!(validator.is_valid(&original));
            let roundtrip: Word = serde_json::from_value(original.clone()).expect("word readback");
            assert_eq!(roundtrip.to_chat(), word.to_chat());
            assert!(roundtrip.location.span().is_none());
            for label in [
                "",
                "two words",
                "x\ty",
                "x\n",
                "x\n@End",
                "x@f",
                "x$n",
                "x:y",
                "x-y",
                "名",
            ] {
                let mut wire = original.clone();
                wire["form_type"] = serde_json::json!({ "z": label });
                assert!(!validator.is_valid(&wire), "schema accepted {label:?}");
                assert!(
                    serde_json::from_value::<Word>(wire).is_err(),
                    "model accepted {label:?}"
                );
            }
        }
    }
    let spec = include_str!("../../../../spec/constructs/word/user_form.md");
    let (_, fence) = spec.split_once("```main_tier\n").expect("authored input");
    let (source, _) = fence.split_once("```").expect("input closing fence");
    check(&TreeSitterParser::new().expect("parser"), source);
    check(&talkbank_parser_re2c::Re2cParser::new(), source);
}

#[test]
fn reference_pos_hints_preserve_both_parser_wire_and_schema_admission() {
    fn check(parser: &impl talkbank_model::ChatParser, source: &str) {
        use talkbank_model::alignment::helpers::{WordItem, walk_words};
        use talkbank_model::{ErrorCollector, Word};
        let errors = ErrorCollector::new();
        let file = parser
            .parse_chat_file(source, 0, &errors)
            .into_option()
            .expect("POS reference model");
        assert!(
            errors.is_empty(),
            "POS reference must have no parser findings"
        );
        let schema = serde_json::to_value(schemars::schema_for!(Word)).expect("word schema");
        let validator = jsonschema::validator_for(&schema).expect("word validator");
        let mut witnesses = 0;
        for utterance in file.utterances() {
            walk_words(&utterance.main.content.content, None, &mut |item| {
                let word = match item {
                    WordItem::Word(word) => word,
                    WordItem::ReplacedWord(replaced) => &replaced.word,
                    WordItem::Separator(_) => return,
                };
                let Some(tag) = &word.part_of_speech else {
                    return;
                };
                witnesses += 1;
                let original = serde_json::to_value(word).expect("word JSON");
                assert_eq!(original["part_of_speech"], tag.as_str());
                assert!(validator.is_valid(&original), "{original}");
                let roundtrip: Word =
                    serde_json::from_value(original.clone()).expect("word readback");
                assert_eq!(roundtrip.to_chat(), word.to_chat());
                assert!(roundtrip.location.span().is_none());
                for payload in [
                    "", "n v", "n\tv", "n\n", "n\n@End", "n$adj", "n@f", "n1", "名",
                ] {
                    let mut wire = original.clone();
                    wire["part_of_speech"] = serde_json::json!(payload);
                    assert!(!validator.is_valid(&wire), "schema accepted {payload:?}");
                    assert!(
                        serde_json::from_value::<Word>(wire).is_err(),
                        "model accepted {payload:?}"
                    );
                }
            });
        }
        assert!(witnesses > 0, "reference must exercise POS hints");
    }
    let source = include_str!("../../../../corpus/reference/word/pos-hint-vocabulary.cha");
    check(&TreeSitterParser::new().expect("parser"), source);
    check(&talkbank_parser_re2c::Re2cParser::new(), source);
}

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
    Com,
    Eng,
    UserDefined(&'a str),
}

impl<'a> RegenerationKey<'a> {
    fn from_tier(tier: &'a DependentTier) -> Option<Self> {
        match tier {
            DependentTier::Mor(_) => Some(Self::Mor),
            DependentTier::Gra(_) => Some(Self::Gra),
            DependentTier::Wor(_) => Some(Self::Wor),
            DependentTier::Com(_) => Some(Self::Com),
            DependentTier::Eng(_) => Some(Self::Eng),
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
            Self::Com => 4,
            Self::Eng => 5,
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
    let mut witnessed = [false; 6];
    for file in &files {
        for utterance in file.utterances() {
            for (index, entry) in utterance.dependent_tiers.iter().enumerate() {
                let Some(key) = RegenerationKey::from_tier(&entry.tier) else {
                    continue;
                };
                if utterance.dependent_tiers[..index]
                    .iter()
                    .any(|earlier| RegenerationKey::from_tier(&earlier.tier) == Some(key))
                {
                    continue;
                }
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

/// Upserting an unchanged payload must never manufacture a second tier line.
/// This applies to every family observed in the reference corpus, not only
/// the morphology/timing families commonly regenerated by tools.
#[test]
fn every_reference_tier_replaces_itself_without_duplication() {
    use talkbank_model::{ChatParser, ErrorCollector};
    let tree = TreeSitterParser::new().expect("parser");
    let native = talkbank_parser_re2c::Re2cParser::new();
    let corpus = ChatCorpus::reference().expect("reference corpus");
    fn check(parser: &impl ChatParser, corpus: &ChatCorpus) {
        let mut witnessed = std::collections::BTreeSet::new();
        for fixture in corpus.fixtures() {
            let errors = ErrorCollector::new();
            let file = parser
                .parse_chat_file(fixture.source(), 0, &errors)
                .into_option()
                .expect("reference parses");
            assert!(
                !errors.has_errors(),
                "{}: {:?}",
                fixture.path().display(),
                errors.into_vec()
            );
            for utterance in file.utterances() {
                for entry in &utterance.dependent_tiers {
                    let mut tiers = utterance.dependent_tiers.clone();
                    tiers.clear();
                    tiers.push(entry.clone());
                    talkbank_transform::dependent_tiers::replace_or_add_tier(
                        &mut tiers,
                        entry.tier.clone(),
                    );
                    assert_eq!(
                        tiers.as_slice(),
                        std::slice::from_ref(entry),
                        "{}: %{} replacement must preserve its one slot and provenance",
                        fixture.path().display(),
                        entry.kind()
                    );
                    witnessed.insert(entry.kind().to_owned());
                }
            }
        }
        for required in ["com", "eng", "act", "cod", "tim", "mor", "gra", "wor"] {
            assert!(
                witnessed.contains(required),
                "missing reference witness for %{required}"
            );
        }
    }
    check(&tree, &corpus);
    check(&native, &corpus);
}
