//! A GRA label is one lexical token; vocabulary and graph validity remain separate.

use talkbank_model::model::{
    ChatFile, GrammaticalRelation, GrammaticalRelationType, TranscriptName, WriteChat,
};
use talkbank_model::{ChatParser, ErrorCode, ErrorCollector};
use talkbank_parser_tests::repo_paths::workspace_root;

#[test]
fn grammatical_relation_wire_and_schema_refuse_token_boundaries() {
    let schema = serde_json::to_value(schemars::schema_for!(GrammaticalRelation)).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    for invalid in [
        serde_json::json!(null),
        serde_json::json!(true),
        serde_json::json!(1),
        serde_json::json!([]),
        serde_json::json!({}),
        serde_json::json!(""),
        serde_json::json!("ROOT-X\n@End"),
        serde_json::json!("ROOT-X|2|1|DEP"),
        serde_json::json!("ROOT-X Y"),
        serde_json::json!("ROOT-X\tY"),
        serde_json::json!("ROOT-X\rY"),
        serde_json::json!("ROOT-X\0Y"),
        serde_json::json!("ROOTé"),
        serde_json::json!("ROOT_"),
        serde_json::json!("1ROOT"),
        serde_json::json!("-ROOT"),
    ] {
        let wire = serde_json::json!({"index": 1, "head": 0, "relation": invalid});
        assert!(!validator.is_valid(&wire), "schema admitted {wire}");
        assert!(
            serde_json::from_value::<GrammaticalRelation>(wire.clone()).is_err(),
            "wire admitted {wire}"
        );
    }
    for text in [
        "ROOT",
        "root",
        "ROOT-x",
        "NSUBJ-CUSTOM-PART",
        "NSUBJJ",
        "A--B",
        "CUSTOM42-",
    ] {
        let wire = serde_json::json!({"index": 1, "head": 0, "relation": text});
        assert!(validator.is_valid(&wire), "schema refused token {text}");
        let relation: GrammaticalRelation = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(relation.relation.as_str(), text);
        assert_eq!(relation.to_chat_string(), format!("1|0|{text}"));
        assert_eq!(serde_json::to_value(relation).unwrap(), wire);
    }
}

fn check_gra_label_contracts(parser: &impl ChatParser) {
    let specs = talkbank_parser_tests::error_specs::load(workspace_root()).unwrap();
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E761.md")
        .unwrap();
    for example in spec.examples() {
        let errors = ErrorCollector::new();
        let source = example.chat.as_str();
        let mut file = parser
            .parse_chat_file(source, 37, &errors)
            .into_option()
            .unwrap();
        assert!(
            errors.is_empty(),
            "{}: {:?}",
            parser.parser_name(),
            errors.to_vec()
        );
        let mut witnessed = 0;
        for utterance in file.utterances() {
            let gra = utterance.gra_tier().unwrap();
            for relation in gra.relations() {
                let label = GrammaticalRelationType::new(relation.relation.as_str()).unwrap();
                assert_eq!(label, relation.relation);
                let spelling = relation.to_chat_string();
                assert!(source.contains(&spelling));
                let fragment = parser
                    .parse_gra_relation(&spelling, 73, &errors)
                    .into_option()
                    .unwrap();
                assert_eq!(&fragment, relation);
                assert_eq!(
                    serde_json::to_value(&relation.relation).unwrap(),
                    relation.relation.as_str()
                );
                witnessed += 1;
            }
        }
        assert!(witnessed > 0);
        assert!(
            errors.is_empty(),
            "{}: {:?}",
            parser.parser_name(),
            errors.to_vec()
        );
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let expected = usize::from(example.claim.demonstrates_own_code());
        let findings: Vec<_> = errors
            .into_vec()
            .into_iter()
            .filter(|error| error.code == ErrorCode::GraRelationHeadNotUniversal)
            .collect();
        assert_eq!(
            findings.len(),
            expected,
            "{} {:?}",
            parser.parser_name(),
            example.title
        );
        let mut restored: ChatFile =
            serde_json::from_value(serde_json::to_value(&file).unwrap()).unwrap();
        let errors = ErrorCollector::new();
        restored.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let findings: Vec<_> = errors
            .into_vec()
            .into_iter()
            .filter(|error| error.code == ErrorCode::GraRelationHeadNotUniversal)
            .collect();
        assert_eq!(
            findings.len(),
            expected,
            "JSON {} {:?}",
            parser.parser_name(),
            example.title
        );
    }
}

#[test]
fn authored_gra_labels_preserve_source_wire_and_vocabulary_in_both_backends() {
    check_gra_label_contracts(&talkbank_parser::TreeSitterParser::new().unwrap());
    check_gra_label_contracts(&talkbank_parser_re2c::Re2cParser::new());
}
