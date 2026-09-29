//! Authored multiline labels exercise normalization, not production provenance.
use talkbank_model::ErrorCollector;
use talkbank_model::model::{Header, Line, SemanticEq, TranscriptName, WriteChat};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::repo_paths::workspace_root;
use talkbank_parser_tests::test_error::strict_parse;

#[test]
fn multiline_reference_gem_labels_preserve_order_and_roundtrip() {
    let parser = TreeSitterParser::new().expect("parser");
    for (name, expected) in [
        (
            "multiline-paired-gems.cha",
            [("@Bg", "morning café play"), ("@Eg", "morning café play")],
        ),
        (
            "multiline-lazy-gems.cha",
            [("@G", "morning café play"), ("@G", "reading time")],
        ),
    ] {
        let source = std::fs::read_to_string(
            workspace_root()
                .join("corpus/reference/edge-cases")
                .join(name),
        )
        .expect("authored reference");
        let mut file = strict_parse(parser.parse_chat_file(&source)).expect("clean parse");
        let errors = ErrorCollector::new();
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        assert!(errors.is_empty(), "{name}: {:?}", errors.to_vec());
        let labels: Vec<_> = file
            .lines
            .iter()
            .filter_map(|line| match line {
                Line::Header { header, .. } => match header.as_ref() {
                    Header::BeginGem { label } => Some(("@Bg", label)),
                    Header::EndGem { label } => Some(("@Eg", label)),
                    Header::LazyGem { label } => Some(("@G", label)),
                    _ => None,
                },
                Line::Utterance(_) => None,
            })
            .map(|(kind, label)| (kind, label.as_ref().expect("nonempty label").as_str()))
            .collect();
        assert_eq!(labels, expected, "{name}: exact logical labels");
        let serialized = file.to_chat_string();
        let reparsed = strict_parse(parser.parse_chat_file(&serialized)).expect("roundtrip parse");
        assert!(file.semantic_eq(&reparsed), "{name}: semantic roundtrip");
        assert_eq!(
            serialized,
            reparsed.to_chat_string(),
            "{name}: stable output"
        );
    }
}
