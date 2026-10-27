//! Missing separator recovery must not manufacture punctuation in the model.

use talkbank_model::alignment::helpers::{ContentItem, walk_content};
use talkbank_model::{ErrorCode, ErrorCollector, TranscriptName, WriteChat};

enum Expected {
    MissingContent,
    AuthoredSeparatorOnly,
    Clean,
}

#[test]
fn authored_empty_tiers_do_not_render_missing_separators() {
    let parser = talkbank_parser::TreeSitterParser::new().expect("parser");
    for (name, original, turns, expected_separators, expected) in [
        (
            "empty",
            include_str!("../error_corpus/validation_errors/E306_1.cha"),
            1,
            0,
            Expected::MissingContent,
        ),
        (
            "empty-word",
            include_str!("../error_corpus/validation_errors/E253_1.cha"),
            1,
            0,
            Expected::MissingContent,
        ),
        (
            "comma",
            include_str!("../error_corpus/validation_errors/E306_separator_only_2.cha"),
            2,
            1,
            Expected::AuthoredSeparatorOnly,
        ),
        (
            "comma-with-dependent",
            include_str!("../error_corpus/validation_errors/E306_separator_only_3.cha"),
            2,
            1,
            Expected::AuthoredSeparatorOnly,
        ),
        (
            "word",
            include_str!("../error_corpus/validation_errors/E306_3.cha"),
            1,
            0,
            Expected::Clean,
        ),
        (
            "empty-retrace",
            include_str!("../error_corpus/validation_errors/E342_empty_scoped_content_2.cha"),
            1,
            0,
            Expected::MissingContent,
        ),
        (
            "empty-explanation",
            include_str!("../error_corpus/validation_errors/E342_empty_scoped_content_4.cha"),
            1,
            0,
            Expected::MissingContent,
        ),
    ] {
        for ending in ["\n", "\r\n", "\r"] {
            let source = original.replace('\n', ending);
            let errors = ErrorCollector::new();
            let file = parser.parse_chat_file_streaming(&source, &errors);
            file.validate(&errors, TranscriptName::Anonymous);
            let diagnostics = errors.to_vec();
            match expected {
                Expected::MissingContent => {
                    assert!(
                        diagnostics
                            .iter()
                            .any(|error| error.code == ErrorCode::MissingRequiredElement),
                        "{name}: {diagnostics:?}"
                    );
                    assert!(
                        diagnostics
                            .iter()
                            .all(|error| error.code != ErrorCode::EmptyWordContent
                                && error.code != ErrorCode::EmptyUtterance),
                        "{name}: no fabricated-content cascade: {diagnostics:?}"
                    );
                }
                Expected::AuthoredSeparatorOnly => {
                    for code in [ErrorCode::EmptyWordContent, ErrorCode::EmptyUtterance] {
                        assert!(
                            diagnostics.iter().any(|error| error.code == code),
                            "{name}: {diagnostics:?}"
                        );
                    }
                }
                Expected::Clean => assert!(diagnostics.is_empty(), "{name}: {diagnostics:?}"),
            }
            assert_eq!(
                file.utterances().count(),
                turns,
                "{name}: preserve turn ownership"
            );
            let mut separators = 0;
            for utterance in file.utterances() {
                walk_content(&utterance.main.content.content, None, &mut |item| {
                    if let ContentItem::Separator(separator) = item {
                        separators += 1;
                        let span = separator
                            .location()
                            .span()
                            .expect("parsed separator location");
                        assert_eq!(
                            source.get(span.start()..span.end()),
                            Some(separator.to_chat_string().as_str()),
                            "{name}: a model separator must own observed punctuation, not a MISSING token; output={:?}; errors={:?}",
                            file.to_chat_string(),
                            errors.to_vec()
                        );
                    }
                });
            }
            assert_eq!(
                separators, expected_separators,
                "{name}: retain only authored separators"
            );
        }
    }
}
