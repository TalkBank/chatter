//! Rejected text retains tier identity but cannot escape transform recovery policy.

use talkbank_model::{ChatParser, ErrorCollector};

#[test]
fn sanitizer_refuses_orphan_annotation_source_at_any_depth() {
    let specs = talkbank_parser_tests::error_specs::load(
        talkbank_parser_tests::repo_paths::workspace_root(),
    )
    .unwrap();
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E759.md")
        .unwrap();
    let parser = talkbank_parser_re2c::Re2cParser::new();
    for (index, malformed) in [
        (0, true),
        (1, true),
        (2, true),
        (6, false),
        (7, true),
        (8, false),
    ] {
        let errors = ErrorCollector::new();
        let file = parser
            .parse_chat_file(spec.examples()[index].chat.as_str(), 0, &errors)
            .into_option()
            .unwrap();
        let result = talkbank_transform::redact::sanitize(
            file,
            &talkbank_transform::redact::SanitizationPolicy::strict(),
        );
        if malformed {
            assert!(matches!(
                result,
                Err(talkbank_transform::redact::RedactError::OrphanAnnotation)
            ));
        } else {
            assert!(result.is_ok());
        }
    }
}
