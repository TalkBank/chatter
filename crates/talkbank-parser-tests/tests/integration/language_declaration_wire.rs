//! Imported language authority comes from headers, never a legacy cache.

use talkbank_model::model::{ChatFile, LanguageDeclarationError, TranscriptName};
use talkbank_model::{ChatParser, ErrorCode, ErrorCollector};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::repo_paths::workspace_root;

#[test]
fn imported_language_state_comes_from_headers_not_a_forged_legacy_cache() {
    let source = std::fs::read_to_string(
        workspace_root().join("corpus/reference/core/headers-metadata.cha"),
    )
    .expect("reference");
    let parser = TreeSitterParser::new().expect("parser");
    let errors = ErrorCollector::new();
    let file = ChatParser::parse_chat_file(&parser, &source, 0, &errors)
        .into_option()
        .expect("reference model");
    let mut wire = serde_json::to_value(&file).expect("wire");
    let codes =
        serde_json::to_value(file.language_declaration().expect("declaration")).expect("codes");
    wire["languages"] = codes;
    let lines = wire["lines"].as_array_mut().expect("lines");
    let language_line = lines
        .iter_mut()
        .find(|line| line["header"]["type"] == "languages")
        .expect("language header");
    language_line["header"]["codes"] = serde_json::json!([]);
    let empty: ChatFile = serde_json::from_value(wire.clone()).expect("explicit empty declaration");
    assert_eq!(
        empty.language_declaration(),
        Err(LanguageDeclarationError::Empty)
    );
    let context_findings = ErrorCollector::new();
    let context = empty.validate_headers_only(&context_findings, TranscriptName::Anonymous);
    let current = talkbank_model::LanguageCode::new("eng").expect("code");
    assert_eq!(
        context.get_other_language(&current),
        Err(LanguageDeclarationError::Empty)
    );
    assert_eq!(
        context.is_tertiary_language(&current),
        Err(LanguageDeclarationError::Empty)
    );
    let findings = ErrorCollector::new();
    empty.validate(&findings, TranscriptName::Anonymous);
    assert!(
        findings
            .to_vec()
            .iter()
            .any(|e| e.code == ErrorCode::EmptyLanguagesHeader)
    );
    wire["lines"]
        .as_array_mut()
        .expect("lines")
        .retain(|line| line["header"]["type"] != "languages");
    let absent: ChatFile = serde_json::from_value(wire).expect("absent declaration");
    assert_eq!(
        absent.language_declaration(),
        Err(LanguageDeclarationError::Absent)
    );
}
