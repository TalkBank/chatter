//! Ordered annotation wrappers through canonical parsing and JSON conversion.
#![allow(clippy::expect_used, clippy::panic)]

use talkbank_model::model::{
    Annotated, BracketedItem, ContentAnnotation, UtteranceContent, WriteChat,
};
use talkbank_model::{Location, Span};

#[derive(Clone, Copy, Debug)]
enum Host {
    Quotation,
    Event,
    Action,
    Retrace,
}

impl Host {
    fn spelling(self) -> &'static str {
        match self {
            Self::Quotation => "“hello” [?] [!]",
            Self::Event => "&=laughs [?] [!]",
            Self::Action => "0 [?] [!]",
            Self::Retrace => "hello [/] [?] [!]",
        }
    }

    fn check(self, item: &UtteranceContent, source: &str) {
        match self {
            Self::Quotation => {
                let UtteranceContent::AnnotatedQuotation(wrapper) = item else {
                    panic!("expected annotated quotation, got {item:?}");
                };
                check_wrapper(wrapper, self.spelling(), source);
            }
            Self::Event => {
                let UtteranceContent::AnnotatedEvent(wrapper) = item else {
                    panic!("expected annotated event, got {item:?}");
                };
                check_wrapper(wrapper, self.spelling(), source);
            }
            Self::Action => {
                let UtteranceContent::AnnotatedAction(wrapper) = item else {
                    panic!("expected annotated action, got {item:?}");
                };
                check_wrapper(wrapper, self.spelling(), source);
            }
            Self::Retrace => {
                let UtteranceContent::AnnotatedRetrace(wrapper) = item else {
                    panic!("expected annotated retrace, got {item:?}");
                };
                check_wrapper(wrapper.as_ref(), self.spelling(), source);
                assert!(
                    !wrapper.inner.is_group,
                    "no invented group around the retrace"
                );
                assert!(matches!(
                    wrapper.inner.content.content.as_slice(),
                    [BracketedItem::Word(_)]
                ));
            }
        }
    }
}

fn check_wrapper<T: WriteChat>(wrapper: &Annotated<T>, spelling: &str, source: &str) {
    assert_eq!(
        wrapper.scoped_annotations.iter().collect::<Vec<_>>(),
        vec![&ContentAnnotation::Uncertain, &ContentAnnotation::Stressing],
        "annotations remain ordered in one nonempty wrapper"
    );
    let start = source.find(spelling).expect("authored unique control");
    assert_eq!(
        wrapper.location,
        Location::Located(Span::new(start, start + spelling.len())),
        "wrapper grows through the second marker, including UTF-8 bytes"
    );
    assert_eq!(wrapper.to_chat_string(), spelling);
}

#[test]
fn reference_multiple_annotations_preserve_host_order_extent_and_wire() {
    use talkbank_model::model::{ChatFile, SemanticEq};
    use talkbank_model::{ErrorCollector, ParseValidateOptions, RuleSelection, TranscriptName};
    use talkbank_parser::TreeSitterParser;
    use talkbank_parser_tests::test_error::strict_parse;
    use talkbank_transform::JsonLayout;

    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/reference/annotation/scope-markers.cha"
    ));
    let parser = TreeSitterParser::new().expect("canonical grammar");
    let mut file = strict_parse(parser.parse_chat_file(source)).expect("reference parses");
    let errors = ErrorCollector::new();
    file.validate_with_alignment_and_rules(
        RuleSelection::new(),
        &errors,
        TranscriptName::for_path(std::path::Path::new("scope-markers.cha")),
    );
    assert!(
        errors.is_empty(),
        "legal annotation hosts: {:?}",
        errors.to_vec()
    );

    let controls = [Host::Quotation, Host::Event, Host::Action, Host::Retrace];
    let utterances: Vec<_> = file.utterances().collect();
    assert_eq!(utterances.len(), 3 + controls.len());
    for (host, utterance) in controls.into_iter().zip(&utterances[3..]) {
        host.check(
            utterance
                .main
                .content
                .content
                .first()
                .expect("annotated host"),
            source,
        );
    }

    let wire = talkbank_transform::chat_to_json(
        source,
        ParseValidateOptions::default(),
        JsonLayout::Compact,
    )
    .expect("validated JSON conversion");
    let restored: ChatFile = serde_json::from_str(&wire).expect("typed wire roundtrip");
    assert!(
        file.semantic_eq(&restored),
        "JSON preserves annotation ownership and order"
    );
    let rendered = file.to_chat();
    let reparsed = strict_parse(parser.parse_chat_file(&rendered)).expect("CHAT roundtrip");
    assert!(file.semantic_eq(&reparsed));
}
