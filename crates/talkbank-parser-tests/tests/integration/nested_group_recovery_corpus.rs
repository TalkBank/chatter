//! Missing inner annotations retain typed groups inside enclosing constructs.

use talkbank_model::model::{
    BracketedContent, BracketedItem, ChatFile, ContentAnnotation, Group, SemanticEq,
    TranscriptName, UtteranceContent, WriteChat,
};
use talkbank_model::{
    ChatParser, DiagnosticLocation, ErrorCode, ErrorCollector, Location, ParseError, ParseOutcome,
    Span,
};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_re2c::Re2cParser;

#[derive(Clone, Copy, Debug)]
enum Enclosure {
    Quotation,
    Phonological,
    Sign,
}

#[derive(Clone, Copy)]
enum Example {
    Control,
    MissingAnnotation,
}

impl Enclosure {
    fn fixture(self, example: Example) -> &'static str {
        match (self, example) {
            (Self::Quotation, Example::Control) => "E342_nested_group_3.cha",
            (Self::Quotation, Example::MissingAnnotation) => "E342_nested_group_4.cha",
            (Self::Phonological, Example::Control) => "E342_nested_group_5.cha",
            (Self::Phonological, Example::MissingAnnotation) => "E342_nested_group_6.cha",
            (Self::Sign, Example::Control) => "E342_nested_group_7.cha",
            (Self::Sign, Example::MissingAnnotation) => "E342_nested_group_8.cha",
        }
    }

    fn contents(self, item: &UtteranceContent) -> &BracketedContent {
        match self {
            Self::Quotation => {
                let UtteranceContent::Quotation(value) = item else {
                    panic!("expected quotation, got {item:?}");
                };
                &value.content
            }
            Self::Phonological => {
                let UtteranceContent::PhoGroup(value) = item else {
                    panic!("expected phonological group, got {item:?}");
                };
                &value.content
            }
            Self::Sign => {
                let UtteranceContent::SinGroup(value) = item else {
                    panic!("expected sign group, got {item:?}");
                };
                &value.content
            }
        }
    }

    fn delimiters(self) -> (&'static str, &'static str) {
        match self {
            Self::Quotation => ("“", "”"),
            Self::Phonological => ("‹", "›"),
            Self::Sign => ("〔", "〕"),
        }
    }
}

/// Retain the parser-produced model beside its exact source and diagnostics.
struct Observation<'source> {
    source: &'source str,
    file: ChatFile,
    diagnostics: Vec<ParseError>,
}

impl<'source> Observation<'source> {
    fn parse(parser: &impl ChatParser, source: &'source str) -> Self {
        let errors = ErrorCollector::new();
        let ParseOutcome::Parsed(file) = parser.parse_chat_file(source, 0, &errors) else {
            panic!(
                "the authored file must retain a model: {:?}",
                errors.to_vec()
            );
        };
        Self {
            source,
            file,
            diagnostics: errors.into_vec(),
        }
    }

    fn span(&self, spelling: &str) -> Span {
        let start = self
            .source
            .find(spelling)
            .expect("unique authored spelling");
        Span::new(start, start + spelling.len())
    }

    fn check_group(&self, group: &Group) {
        assert_eq!(group.location, Location::Located(self.span("<hello>")));
        let [BracketedItem::Word(word)] = group.content.content.as_slice() else {
            panic!("the inner group must retain exactly its spoken word");
        };
        assert_eq!(word.cleaned_text(), "hello");
        assert_eq!(word.location, Location::Located(self.span("hello")));
        assert_eq!(group.to_chat_string(), "<hello>");
    }

    fn outer(&self) -> &UtteranceContent {
        let utterance = self.file.utterances().next().expect("one utterance");
        let [outer] = utterance.main.content.content.as_slice() else {
            panic!("the enclosing construct must survive whole");
        };
        outer
    }

    fn check_control(&self, enclosure: Enclosure) {
        let inner = enclosure.contents(self.outer()).content.as_slice();
        let [BracketedItem::AnnotatedGroup(wrapper)] = inner else {
            panic!("expected annotated inner group, got {inner:?}");
        };
        self.check_group(&wrapper.inner);
        assert_eq!(
            wrapper.scoped_annotations.iter().collect::<Vec<_>>(),
            [&ContentAnnotation::Uncertain]
        );
        assert_eq!(
            wrapper.location,
            Location::Located(self.span("<hello> [?]"))
        );
        assert_eq!(self.file.to_chat_string(), self.source);
    }
}

#[test]
fn missing_inner_annotation_retains_bare_group_and_exact_recovery_position() {
    let parser = TreeSitterParser::new().expect("grammar");
    let native_parser = Re2cParser::new();
    for enclosure in [
        Enclosure::Quotation,
        Enclosure::Phonological,
        Enclosure::Sign,
    ] {
        for example in [Example::Control, Example::MissingAnnotation] {
            let source =
                std::fs::read_to_string(super::corpus_dir().join(enclosure.fixture(example)))
                    .expect("generated authored spec fixture");
            let mut observed = Observation::parse(&parser, &source);
            match example {
                Example::Control => assert!(observed.diagnostics.is_empty()),
                Example::MissingAnnotation => {
                    let [diagnostic] = observed.diagnostics.as_slice() else {
                        panic!(
                            "expected only missing-annotation recovery: {:?}",
                            observed.diagnostics
                        );
                    };
                    assert_eq!(diagnostic.code, ErrorCode::MissingRequiredElement);
                    let point = observed.span("<hello> ").end();
                    assert_eq!(
                        diagnostic.location,
                        DiagnosticLocation::new(Span::new(point, point)),
                        "the missing marker is before the outer closing delimiter"
                    );
                }
            }
            let validation = ErrorCollector::new();
            observed
                .file
                .validate_with_alignment(&validation, TranscriptName::Anonymous);
            match example {
                Example::Control => assert!(validation.is_empty(), "{:?}", validation.to_vec()),
                Example::MissingAnnotation => {
                    let diagnostics = validation.into_vec();
                    let [diagnostic] = diagnostics.as_slice() else {
                        panic!("expected retained missing-annotation invalidity: {diagnostics:?}");
                    };
                    assert_eq!(diagnostic.code, ErrorCode::MissingRequiredElement);
                    assert_eq!(
                        diagnostic.location,
                        DiagnosticLocation::new(observed.span("<hello>"))
                    );
                }
            }
            let outer = observed.outer();
            let inner = enclosure.contents(outer).content.as_slice();
            match example {
                Example::Control => {
                    observed.check_control(enclosure);
                    let mut native = Observation::parse(&native_parser, &source);
                    assert!(native.diagnostics.is_empty(), "{:?}", native.diagnostics);
                    let native_validation = ErrorCollector::new();
                    native
                        .file
                        .validate_with_alignment(&native_validation, TranscriptName::Anonymous);
                    assert!(
                        native_validation.is_empty(),
                        "{:?}",
                        native_validation.to_vec()
                    );
                    native.check_control(enclosure);
                    assert!(observed.file.semantic_eq(&native.file));
                }
                Example::MissingAnnotation => {
                    let [BracketedItem::Group(group)] = inner else {
                        panic!(
                            "expected bare group, without invented marker or retrace: {inner:?}"
                        );
                    };
                    observed.check_group(group);
                    let (open, close) = enclosure.delimiters();
                    let mut rendered = String::new();
                    outer.write_chat(&mut rendered).expect("CHAT rendering");
                    assert_eq!(rendered, format!("{open}<hello>{close}"));
                }
            }
        }
    }
}
