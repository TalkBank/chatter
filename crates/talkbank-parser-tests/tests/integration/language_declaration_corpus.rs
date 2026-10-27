//! Authored language-list recovery retains source and withholds semantic authority.

use talkbank_model::model::{
    ChatFile, Header, LanguageDeclarationError, TranscriptName, WriteChat,
};
use talkbank_model::{ChatParser, ErrorCode, ErrorCollector};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_re2c::Re2cParser;
use talkbank_parser_tests::{error_specs, repo_paths::workspace_root};

enum Expected {
    Codes(&'static [&'static str]),
    Rejected,
}

fn check(parser: &impl ChatParser, source: &str, expected: &Expected) {
    let diagnostics = ErrorCollector::new();
    let file = parser
        .parse_chat_file(source, 0, &diagnostics)
        .into_option()
        .expect("retained file");
    let wire = serde_json::to_value(&file).expect("wire");
    assert!(
        wire.get("languages").is_none(),
        "no independently forgeable language cache"
    );
    let decoded: ChatFile = serde_json::from_value(wire).expect("decoded model");
    for document in [&file, &decoded] {
        match expected {
            Expected::Codes(expected) => {
                let actual: Vec<_> = document
                    .language_declaration()
                    .expect("admitted")
                    .iter()
                    .map(|code| code.as_str())
                    .collect();
                assert_eq!(&actual, expected, "{}", parser.parser_name());
            }
            Expected::Rejected => {
                assert_eq!(
                    document.language_declaration(),
                    Err(LanguageDeclarationError::Rejected),
                    "{}: {source:?}",
                    parser.parser_name()
                );
                let raw = document
                    .headers()
                    .find_map(|header| match header {
                        Header::UnparsedLanguages(raw) => Some(raw),
                        _ => None,
                    })
                    .expect("typed rejected source");
                let original = source
                    .split(['\r', '\n'])
                    .find(|line| line.starts_with("@Languages:"))
                    .expect("authored header");
                assert_eq!(
                    raw.source().trim_end_matches(['\r', '\n']),
                    original.trim_end_matches(['\r', '\n'])
                );
                assert!(
                    document
                        .to_chat_string()
                        .contains(raw.source().trim_end_matches(['\r', '\n']))
                );
                assert!(
                    source.contains(raw.source()),
                    "raw source is observed, never reconstructed"
                );
                let context_errors = ErrorCollector::new();
                let context =
                    document.validate_headers_only(&context_errors, TranscriptName::Anonymous);
                assert!(context.shared.default_language.is_none());
                assert_eq!(
                    context.shared.declared_languages,
                    Err(LanguageDeclarationError::Rejected)
                );
                let findings = ErrorCollector::new();
                document.validate(&findings, TranscriptName::Anonymous);
                let findings = findings.into_vec();
                assert!(findings.iter().any(|e| matches!(
                    e.code,
                    ErrorCode::EmptyLanguagesHeader | ErrorCode::UnparsableContent
                )));
                assert!(
                    !findings
                        .iter()
                        .any(|e| e.code == ErrorCode::MissingRequiredHeader)
                );
                assert!(
                    !findings
                        .iter()
                        .any(|e| e.code == ErrorCode::InvalidLanguageCode)
                );
            }
        }
    }
    if matches!(expected, Expected::Rejected) {
        assert!(diagnostics.has_errors(), "{}", parser.parser_name());
    }
}

#[test]
fn authored_language_declarations_preserve_rejection_through_both_parsers_and_json() {
    let specs = error_specs::load(workspace_root()).expect("authored specs");
    let tree = TreeSitterParser::new().expect("parser");
    let native = Re2cParser::new();
    for (file, title, expected) in [
        ("E507.md", "One language", Expected::Codes(&["eng"])),
        (
            "E507.md",
            "Comma-separated multilingual control",
            Expected::Codes(&["eng", "spa"]),
        ),
        ("E507.md", "A tab and nothing", Expected::Rejected),
        (
            "E507.md",
            "Replace the sole language code with spaces",
            Expected::Rejected,
        ),
        (
            "E507.md",
            "Replace the sole language code with a tab",
            Expected::Rejected,
        ),
        (
            "E316.md",
            "Replace a language-list comma with a semicolon",
            Expected::Rejected,
        ),
        (
            "E316.md",
            "Duplicate a language-list comma",
            Expected::Rejected,
        ),
    ] {
        let spec = specs
            .iter()
            .find(|spec| spec.filename == file)
            .expect("spec");
        let example = spec
            .examples()
            .iter()
            .find(|example| example.title.as_deref() == Some(title))
            .expect("example");
        for ending in ["\n", "\r\n", "\r"] {
            let source = example.chat.as_str().replace('\n', ending);
            check(&tree, &source, &expected);
            check(&native, &source, &expected);
        }
    }
}

#[test]
fn duplicate_language_declarations_have_no_order_dependent_winner() {
    let source = std::fs::read_to_string(
        workspace_root().join("corpus/reference/core/headers-metadata.cha"),
    )
    .expect("reference");
    let tree = TreeSitterParser::new().expect("parser");
    let native = Re2cParser::new();
    let header = source
        .lines()
        .find(|line| line.starts_with("@Languages:"))
        .expect("reference declaration");
    let specs = error_specs::load(workspace_root()).expect("specs");
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E316.md")
        .expect("E316");
    let example = spec
        .examples()
        .iter()
        .find(|example| example.title.as_deref() == Some("Duplicate a language-list comma"))
        .expect("authored rejection");
    let rejected = example
        .chat
        .as_str()
        .lines()
        .find(|line| line.starts_with("@Languages:"))
        .expect("rejected declaration");
    for repeated in [
        format!("{header}\n{rejected}"),
        format!("{rejected}\n{header}"),
    ] {
        let source = source.replacen(header, &repeated, 1);
        fn check_duplicate(parser: &impl ChatParser, source: &str) {
            let errors = ErrorCollector::new();
            let file = parser
                .parse_chat_file(source, 0, &errors)
                .into_option()
                .expect("file");
            assert_eq!(
                file.language_declaration(),
                Err(LanguageDeclarationError::Repeated)
            );
            let decoded: ChatFile =
                serde_json::from_str(&serde_json::to_string(&file).expect("wire")).expect("model");
            assert_eq!(
                decoded.language_declaration(),
                Err(LanguageDeclarationError::Repeated)
            );
            let mut attempted = decoded.clone();
            assert_eq!(
                talkbank_transform::fix_s::rewrite_whole_utterance_language_switches(
                    &mut attempted
                ),
                Err(LanguageDeclarationError::Repeated)
            );
            assert_eq!(attempted, decoded, "refusal is atomic");
        }
        check_duplicate(&tree, &source);
        check_duplicate(&native, &source);
    }
}
