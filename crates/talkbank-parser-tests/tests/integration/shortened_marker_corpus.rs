//! Typed shortening structure, not cleaned spelling, owns spoken-content validity.

use talkbank_model::model::{ChatFile, TranscriptName};
use talkbank_model::{ChatParser, ErrorCode, ErrorCollector};
use talkbank_parser_tests::{error_specs, repo_paths::workspace_root};

#[derive(Clone, Copy)]
enum Control {
    Shortened,
    Bare,
    CaOmission,
}

impl Control {
    fn title(self) -> &'static str {
        match self {
            Self::Shortened => "Entirely shortened untranscribed spellings have no spoken content",
            Self::Bare => "Bare untranscribed markers retain spoken-content eligibility",
            Self::CaOmission => "CA normalizes parenthesized marker spellings as omissions",
        }
    }
    fn codes(self) -> Vec<ErrorCode> {
        match self {
            Self::Shortened => vec![ErrorCode::EmptySpokenContent; 3],
            Self::Bare | Self::CaOmission => vec![],
        }
    }
}

#[test]
fn shortening_only_markers_cannot_bypass_spoken_content_validation() {
    let specs = error_specs::load(workspace_root()).expect("specs");
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E209.md")
        .expect("E209");
    let tree = talkbank_parser::TreeSitterParser::new().expect("parser");
    let native = talkbank_parser_re2c::Re2cParser::new();
    for control in [Control::Shortened, Control::Bare, Control::CaOmission] {
        let example = spec
            .examples()
            .iter()
            .find(|example| example.title.as_deref() == Some(control.title()))
            .expect("authored control");
        fn check(parser: &impl ChatParser, source: &str, control: Control) {
            let errors = ErrorCollector::new();
            let file = parser
                .parse_chat_file(source, 0, &errors)
                .into_option()
                .expect("file");
            assert!(errors.is_empty(), "{:?}", errors.into_vec());
            let decoded: ChatFile =
                serde_json::from_str(&serde_json::to_string(&file).expect("wire")).expect("model");
            for document in [&file, &decoded] {
                let errors = ErrorCollector::new();
                document.validate(&errors, TranscriptName::Anonymous);
                assert_eq!(
                    errors
                        .into_vec()
                        .iter()
                        .map(|error| error.code)
                        .collect::<Vec<_>>(),
                    control.codes(),
                    "{}: {}",
                    parser.parser_name(),
                    control.title()
                );
            }
        }
        check(&tree, example.chat.as_str(), control);
        check(&native, example.chat.as_str(), control);
    }
}
