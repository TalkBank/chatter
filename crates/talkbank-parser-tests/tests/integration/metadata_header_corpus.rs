//! Structured metadata contracts through real document, fragment and wire boundaries.

use talkbank_model::model::{ChatFile, Header, Line, SemanticEq, TranscriptName, WriteChat};
use talkbank_model::{
    ChatParser, CompletedDiagnostics, ErrorCode, ErrorCollector, ErrorSink, Severity,
};
use talkbank_parser::TreeSitterParser;
use talkbank_parser::generated_traversal::{AsRawNode, SourceSlotView, TypesHeaderNode};
use talkbank_parser_tests::{repo_paths::workspace_root, test_error::strict_parse};

fn specimen(name: &str) -> String {
    std::fs::read_to_string(workspace_root().join(format!(
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/{name}.cha"
    )))
    .expect("authored metadata specimen")
}

fn metadata(header: &Header) -> bool {
    matches!(
        header,
        Header::Languages { .. } | Header::Types(_) | Header::Situation { .. }
    )
}

#[test]
fn metadata_headers_legal_payloads_preserve_order_unicode_and_wire_trust() {
    let parser = TreeSitterParser::new().expect("parser");
    for name in ["E316_metadata_headers_1", "E316_metadata_headers_2"] {
        for newline in ["\n", "\r\n"] {
            let source = specimen(name).replace('\n', newline);
            let file = strict_parse(parser.parse_chat_file(&source)).expect("legal metadata");
            let mut seen = Vec::new();
            for header in file.headers() {
                match header {
                    Header::Languages { codes } => {
                        assert_eq!(
                            codes.iter().map(|code| code.as_str()).collect::<Vec<_>>(),
                            ["eng", "fra", "spa"]
                        );
                        seen.push("languages");
                    }
                    Header::Types(types) => {
                        assert_eq!(
                            [
                                types.design.as_str(),
                                types.activity.as_str(),
                                types.group.as_str()
                            ],
                            ["long", "toyplay", "TD"]
                        );
                        seen.push("types");
                    }
                    Header::Situation { text } => {
                        assert_eq!(text.as_ref(), "café α — jeu partagé");
                        seen.push("situation");
                    }
                    _ => {}
                }
            }
            assert_eq!(seen, ["languages", "types", "situation"]);
            let wire: ChatFile = serde_json::from_str(&serde_json::to_string(&file).expect("wire"))
                .expect("model admission");
            assert!(file.semantic_eq(&wire));
            let errors = ErrorCollector::new();
            let refusal = wire
                .validate_into(&errors, TranscriptName::Anonymous)
                .expect_err("wire payloads cannot restore parse-health evidence");
            assert!(refusal.has_incomplete_parse());
            assert!(errors.is_empty(), "legal payloads remain legal");
            let errors = ErrorCollector::new();
            let valid = file
                .validate_into(&errors, TranscriptName::Anonymous)
                .expect("valid metadata");
            assert!(errors.is_empty());
            let normalized = valid.document().to_chat_string();
            let reparsed = strict_parse(parser.parse_chat_file(&normalized)).expect("normal form");
            assert!(valid.document().semantic_eq(&reparsed));
            assert_eq!(
                reparsed.to_chat_string(),
                normalized,
                "normalization fixed point"
            );
        }
    }
}

#[test]
fn metadata_headers_fragments_agree_with_document_payloads_and_rebasing() {
    let parser = TreeSitterParser::new().expect("parser");
    for name in ["E316_metadata_headers_1", "E316_metadata_headers_2"] {
        for newline in ["\n", "\r\n"] {
            let source = specimen(name).replace('\n', newline);
            let file = strict_parse(parser.parse_chat_file(&source)).expect("document");
            for origin in [0, 25] {
                let errors = ErrorCollector::new();
                let shifted = ChatParser::parse_chat_file(&parser, &source, origin, &errors)
                    .into_option()
                    .expect("rebased document");
                assert!(errors.is_empty());
                assert!(file.semantic_eq(&shifted));
                let mut compared = 0;
                for (original, rebased) in file.lines.iter().zip(shifted.lines.iter()) {
                    let (
                        Line::Header { header, span, .. },
                        Line::Header {
                            span: shifted_span, ..
                        },
                    ) = (original, rebased)
                    else {
                        continue;
                    };
                    if !metadata(header.as_ref()) {
                        continue;
                    }
                    assert_eq!(shifted_span.start as usize, span.start as usize + origin);
                    assert_eq!(shifted_span.end as usize, span.end as usize + origin);
                    let fragment = &source[span.start as usize..span.end as usize];
                    let errors = ErrorCollector::new();
                    let parsed = ChatParser::parse_header(&parser, fragment, origin, &errors)
                        .into_option()
                        .expect("native header fragment");
                    assert!(errors.is_empty(), "{name}: {:?}", errors.to_vec());
                    assert!(header.as_ref().semantic_eq(&parsed));
                    compared += 1;
                }
                assert_eq!(compared, 3, "all three metadata families actually compared");
            }
        }
    }
}

#[test]
fn metadata_headers_types_truncation_is_source_owned_and_not_fabricated() {
    let parser = TreeSitterParser::new().expect("parser");
    let source = specimen("E316_metadata_headers_1");
    let errors = ErrorCollector::new();
    let (file, owner) = parser.parse_chat_file_with_source(&source, &errors);
    assert!(errors.is_empty());
    assert!(
        file.headers()
            .any(|header| matches!(header, Header::Types(_)))
    );
    let owner = owner.expect("source owner");
    let mut candidates = Vec::new();
    for node in owner.root().expect("root").descendants() {
        let node = node.expect("readable source");
        let Some(types) = node.typed::<TypesHeaderNode>() else {
            continue;
        };
        let children = types.extract();
        let SourceSlotView::Present(design) = children.field_child_2().slot().view() else {
            panic!("legal control has a present design")
        };
        let SourceSlotView::Present(group) = children.field_child_10().slot().view() else {
            panic!("legal control has a present group")
        };
        let design = design.read().expect("design read");
        let group = group.read().expect("group read");
        assert_eq!(design.source(), source);
        assert_eq!(design.text(), "long");
        assert_eq!(group.text(), "TD");
        let mut candidate = source.clone();
        candidate.replace_range(
            design.raw_node().end_byte()..group.raw_node().end_byte(),
            "",
        );
        candidates.push(candidate);
    }
    assert_eq!(
        candidates,
        [specimen("E316_metadata_headers_3")],
        "authored truncation deletes only the source-bound trailing Types fields"
    );
    let errors = ErrorCollector::new();
    let (recovered, owner) = parser.parse_chat_file_with_source(&candidates[0], &errors);
    assert!(
        !recovered
            .headers()
            .any(|header| matches!(header, Header::Types(_))),
        "document recovery cannot invent a complete Types header"
    );
    assert!(owner.expect("recovery source").root_node().has_error());
    assert!(
        errors
            .to_vec()
            .iter()
            .any(|error| error.code == ErrorCode::UnparsableContent)
    );
    assert!(strict_parse(parser.parse_chat_file(&candidates[0])).is_err());
    // Model checks do not recertify the original bytes. ParseProduct/strict
    // admission above owns the document-level recovery diagnostic.
    recovered.validate(&errors, TranscriptName::Anonymous);
    CompletedDiagnostics::admit(errors.into_vec()).expect("authored fault is not producer failure");
}

#[test]
fn metadata_headers_language_faults_survive_wire_without_false_validity() {
    enum FaultStage {
        Parse,
        Model,
    }
    let parser = TreeSitterParser::new().expect("parser");
    for (name, code, stage) in [
        ("E316_13", ErrorCode::UnparsableContent, FaultStage::Parse),
        (
            "E316_14",
            ErrorCode::EmptyLanguagesHeader,
            FaultStage::Parse,
        ),
        ("E507_2", ErrorCode::EmptyLanguagesHeader, FaultStage::Parse),
        ("E519_1", ErrorCode::InvalidLanguageCode, FaultStage::Model),
    ] {
        let source = specimen(name);
        let product = parser.parse_chat_file(&source);
        match stage {
            FaultStage::Parse => assert!(
                product.has_error_diagnostics(),
                "{name}: source admission must retain the parse fault"
            ),
            FaultStage::Model => assert!(
                !product.has_error_diagnostics(),
                "{name}: code membership is a model policy"
            ),
        }
        let errors = ErrorCollector::new();
        let talkbank_parser::ParseProduct::Built { file, diagnostics } = product else {
            panic!("these authored faults retain a model")
        };
        for diagnostic in diagnostics {
            errors.report(diagnostic);
        }
        let wire: ChatFile = serde_json::from_str(&serde_json::to_string(&file).expect("wire"))
            .expect("wire payload admission");
        assert!(file.semantic_eq(&wire));
        file.validate(&errors, TranscriptName::Anonymous);
        let completed = CompletedDiagnostics::admit(errors.into_vec()).expect("data fault");
        assert!(
            completed
                .diagnostics()
                .iter()
                .any(|error| error.code == code && error.severity == Severity::Error),
            "{name}: original fault remains explicit"
        );
        let errors = ErrorCollector::new();
        let refused = wire
            .validate_into(&errors, TranscriptName::Anonymous)
            .expect_err("no wire validity proof");
        assert!(
            refused.has_incomplete_parse(),
            "wire does not certify salvaged language values"
        );
        assert!(!refused.has_internal_failure());
        CompletedDiagnostics::admit(errors.into_vec())
            .expect("wire refusal is not producer failure");
    }
}

#[test]
fn metadata_headers_malformed_types_keep_source_refusal_and_noninternal_diagnostics() {
    let parser = TreeSitterParser::new().expect("parser");
    for name in [
        "E316_metadata_headers_4",
        "E316_metadata_headers_5",
        "E316_metadata_headers_6",
    ] {
        let source = specimen(name);
        let product = parser.parse_chat_file(&source);
        assert!(
            product.has_error_diagnostics(),
            "{name}: source admission refuses malformed atoms"
        );
        let completed = CompletedDiagnostics::admit(product.diagnostics().to_vec())
            .expect("authored invalidity, not internal failure");
        assert!(
            completed
                .diagnostics()
                .iter()
                .any(|error| error.code == ErrorCode::UnparsableContent)
        );
        assert!(strict_parse(product).is_err());
    }
}
