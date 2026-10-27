//! Lexical admission and source ownership for scoped event markers.

use serde::{Serialize, de::DeserializeOwned};
use std::collections::BTreeSet;
use talkbank_model::alignment::helpers::{ContentItem, walk_content};
use talkbank_model::{ChatParser, ErrorCollector, Location, WriteChat};
use talkbank_parser_tests::chat_corpus::ChatCorpus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MarkerKind {
    LongBegin,
    LongEnd,
    NonvocalBegin,
    NonvocalEnd,
    NonvocalSimple,
}

fn marker_contract<T>(marker: &T, source: &str, location: impl Fn(&T) -> Location)
where
    T: WriteChat + Serialize + DeserializeOwned,
{
    let span = location(marker).span().unwrap();
    assert_eq!(
        &source[span.start() - 53..span.end() - 53],
        marker.to_chat_string()
    );
    let wire = serde_json::to_value(marker).unwrap();
    assert!(wire.get("location").is_none());
    let restored: T = serde_json::from_value(wire.clone()).unwrap();
    assert!(location(&restored).span().is_none());
    assert_eq!(restored.to_chat_string(), marker.to_chat_string());
    for label in ["", "label}", "label\n*CHI:\tsecret"] {
        let mut refused = wire.clone();
        refused["label"] = serde_json::json!(label);
        assert!(serde_json::from_value::<T>(refused).is_err(), "{label:?}");
    }
}

#[test]
fn both_parsers_keep_all_scoped_marker_sources_and_checked_wire_labels() {
    fn check(parser: &impl ChatParser) {
        let mut witnessed = BTreeSet::new();
        let corpus = ChatCorpus::reference().unwrap();
        for fixture in corpus.fixtures() {
            let errors = ErrorCollector::new();
            let file = parser
                .parse_chat_file(fixture.source(), 53, &errors)
                .into_option()
                .unwrap();
            assert!(
                errors.is_empty(),
                "{}: {:?}",
                fixture.path().display(),
                errors.to_vec()
            );
            for utterance in file.utterances() {
                walk_content(&utterance.main.content.content, None, &mut |item| {
                    let kind = match item {
                        ContentItem::LongFeatureBegin(value) => {
                            marker_contract(value, fixture.source(), |v| v.location);
                            MarkerKind::LongBegin
                        }
                        ContentItem::LongFeatureEnd(value) => {
                            marker_contract(value, fixture.source(), |v| v.location);
                            MarkerKind::LongEnd
                        }
                        ContentItem::NonvocalBegin(value) => {
                            marker_contract(value, fixture.source(), |v| v.location);
                            MarkerKind::NonvocalBegin
                        }
                        ContentItem::NonvocalEnd(value) => {
                            marker_contract(value, fixture.source(), |v| v.location);
                            MarkerKind::NonvocalEnd
                        }
                        ContentItem::NonvocalSimple(value) => {
                            marker_contract(value, fixture.source(), |v| v.location);
                            MarkerKind::NonvocalSimple
                        }
                        ContentItem::Word(_)
                        | ContentItem::ReplacedWord(_)
                        | ContentItem::Separator(_)
                        | ContentItem::Event(_)
                        | ContentItem::Pause(_)
                        | ContentItem::Action(_)
                        | ContentItem::OverlapPoint(_)
                        | ContentItem::OtherSpokenEvent(_)
                        | ContentItem::Freecode(_)
                        | ContentItem::OrphanAnnotation(_)
                        | ContentItem::InternalBullet(_)
                        | ContentItem::UnderlineBegin(_)
                        | ContentItem::UnderlineEnd(_) => return,
                    };
                    witnessed.insert(kind);
                });
            }
        }
        assert_eq!(
            witnessed,
            BTreeSet::from([
                MarkerKind::LongBegin,
                MarkerKind::LongEnd,
                MarkerKind::NonvocalBegin,
                MarkerKind::NonvocalEnd,
                MarkerKind::NonvocalSimple,
            ])
        );
    }
    check(&talkbank_parser::TreeSitterParser::new().unwrap());
    check(&talkbank_parser_re2c::Re2cParser::new());
}

#[test]
fn scoped_label_schemas_keep_legal_alphabet_and_refuse_delimiters() {
    use talkbank_model::{LongFeatureLabel, NonvocalLabel};
    for schema in [
        serde_json::to_value(schemars::schema_for!(LongFeatureLabel)).unwrap(),
        serde_json::to_value(schemars::schema_for!(NonvocalLabel)).unwrap(),
    ] {
        let schema = jsonschema::validator_for(&schema).unwrap();
        for label in ["singing", "cough", "custom_1-label@x%y", "0"] {
            assert!(schema.is_valid(&serde_json::json!(label)), "{label:?}");
        }
        for label in ["", "two labels", "label}", "é", "label\n", "label\0", "a=b"] {
            assert!(!schema.is_valid(&serde_json::json!(label)), "{label:?}");
        }
    }
}
