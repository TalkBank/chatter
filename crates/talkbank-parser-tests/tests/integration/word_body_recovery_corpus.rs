//! Prefixes and suffixes cannot substitute for a missing word body.

use talkbank_model::model::{
    ChatFile, ParseHealthTier, SemanticEq, TranscriptName, UtteranceContent, Word, WordContent,
    WriteChat,
};
use talkbank_model::{
    ChatParser, DiagnosticLocation, ErrorCode, ErrorCollector, Location, ParseError, ParseOutcome,
    Span,
};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_re2c::Re2cParser;

#[derive(Clone, Copy)]
enum Case {
    MissingReplacementBody,
    MissingSuffixBody,
    CompleteOmission,
    CompleteReplacement,
    CompleteSuffix,
}

impl Case {
    fn fixture(self) -> &'static str {
        match self {
            Self::MissingReplacementBody => "E342_2.cha",
            Self::MissingSuffixBody => "E342_3.cha",
            Self::CompleteOmission => "E390_1.cha",
            Self::CompleteReplacement => "E390_3.cha",
            Self::CompleteSuffix => "E342_8.cha",
        }
    }
}

struct Observation<'source> {
    source: &'source str,
    file: ChatFile,
    diagnostics: Vec<ParseError>,
}

impl<'source> Observation<'source> {
    fn parse(parser: &impl ChatParser, source: &'source str) -> Self {
        let sink = ErrorCollector::new();
        let ParseOutcome::Parsed(file) = parser.parse_chat_file(source, 0, &sink) else {
            panic!("spec file must retain a model: {:?}", sink.to_vec());
        };
        Self {
            source,
            file,
            diagnostics: sink.into_vec(),
        }
    }

    fn span(&self, spelling: &str) -> Span {
        let start = self.source.find(spelling).expect("authored spelling");
        Span::new(start, start + spelling.len())
    }

    fn content(&self) -> &[UtteranceContent] {
        self.file
            .utterances()
            .next()
            .expect("one utterance")
            .main
            .content
            .content
            .as_slice()
    }

    fn check_word(&self, word: &Word, spelling: &str, body: &str) {
        assert_eq!(word.location, Location::Located(self.span(spelling)));
        assert_eq!(word.raw_text(), spelling);
        assert_eq!(word.cleaned_text(), body);
        let [WordContent::Text(text)] = word.content().as_slice() else {
            panic!(
                "expected only the actual word body, got {:?}",
                word.content()
            );
        };
        assert_eq!(text.as_ref(), body);
    }
}

#[test]
fn recovered_affixes_never_become_spoken_word_bodies() {
    let parser = TreeSitterParser::new().expect("grammar");
    let native = Re2cParser::new();
    for case in [
        Case::MissingReplacementBody,
        Case::MissingSuffixBody,
        Case::CompleteOmission,
        Case::CompleteReplacement,
        Case::CompleteSuffix,
    ] {
        let source = std::fs::read_to_string(super::corpus_dir().join(case.fixture()))
            .expect("generated spec fixture");
        let mut observed = Observation::parse(&parser, &source);
        let health = observed
            .file
            .utterances()
            .next()
            .expect("one utterance")
            .parse_health();
        match case {
            Case::MissingReplacementBody | Case::MissingSuffixBody => {
                assert!(health.is_tier_tainted(ParseHealthTier::Main));
            }
            Case::CompleteOmission | Case::CompleteReplacement | Case::CompleteSuffix => {
                assert!(health.is_tier_clean(ParseHealthTier::Main));
            }
        }
        match case {
            Case::MissingReplacementBody => {
                let mut codes: Vec<_> = observed.diagnostics.iter().map(|e| e.code).collect();
                codes.sort_by_key(|code| code.as_str());
                assert_eq!(
                    codes,
                    [
                        ErrorCode::MissingRequiredElement,
                        ErrorCode::ReplacementContainsOmission
                    ]
                );
                let omission = observed
                    .diagnostics
                    .iter()
                    .find(|e| e.code == ErrorCode::ReplacementContainsOmission)
                    .unwrap();
                assert_eq!(
                    omission.location,
                    DiagnosticLocation::new(observed.span("0"))
                );
                let [
                    UtteranceContent::Word(base),
                    UtteranceContent::Word(following),
                ] = observed.content()
                else {
                    panic!(
                        "no replacement Word may be invented: {:?}",
                        observed.content()
                    );
                };
                observed.check_word(base, "helo", "helo");
                observed.check_word(following, "world", "world");
            }
            Case::MissingSuffixBody => {
                let [error] = observed.diagnostics.as_slice() else {
                    panic!("expected missing-body recovery: {:?}", observed.diagnostics);
                };
                assert_eq!(error.code, ErrorCode::MissingRequiredElement);
                assert!(
                    observed.content().is_empty(),
                    "a suffix is not a spoken word"
                );
            }
            Case::CompleteOmission | Case::CompleteReplacement => {
                assert!(
                    observed.diagnostics.is_empty(),
                    "{:?}",
                    observed.diagnostics
                );
                let [UtteranceContent::ReplacedWord(replaced)] = observed.content() else {
                    panic!("expected complete replacement");
                };
                observed.check_word(&replaced.word, "went", "went");
                let [word] = replaced.replacement.words.as_slice() else {
                    panic!("expected one replacement word");
                };
                let spelling = match case {
                    Case::CompleteOmission => "0go",
                    Case::CompleteReplacement => "go",
                    Case::MissingReplacementBody
                    | Case::MissingSuffixBody
                    | Case::CompleteSuffix => unreachable!(),
                };
                observed.check_word(word, spelling, "go");
            }
            Case::CompleteSuffix => {
                assert!(
                    observed.diagnostics.is_empty(),
                    "{:?}",
                    observed.diagnostics
                );
                let [
                    UtteranceContent::Word(word),
                    UtteranceContent::Word(following),
                ] = observed.content()
                else {
                    panic!("expected complete suffixed word");
                };
                observed.check_word(word, "hello@s:eng", "hello");
                observed.check_word(following, "world", "world");
                let sink = ErrorCollector::new();
                let ParseOutcome::Parsed(fragment) = ChatParser::parse_word(
                    &parser,
                    "hello@s:eng",
                    observed.span("hello@s:eng").start(),
                    &sink,
                ) else {
                    panic!("complete word fragment: {:?}", sink.to_vec());
                };
                assert!(sink.is_empty());
                observed.check_word(&fragment, "hello@s:eng", "hello");
            }
        }
        let validation = ErrorCollector::new();
        observed
            .file
            .validate_with_alignment(&validation, TranscriptName::Anonymous);
        match case {
            Case::CompleteOmission => {
                let errors = validation.into_vec();
                let [error] = errors.as_slice() else {
                    panic!("{:?}", errors);
                };
                assert_eq!(error.code, ErrorCode::ReplacementContainsOmission);
                assert_eq!(
                    error.location,
                    DiagnosticLocation::new(observed.span("0go"))
                );
            }
            Case::MissingReplacementBody
            | Case::MissingSuffixBody
            | Case::CompleteReplacement
            | Case::CompleteSuffix => assert!(validation.is_empty(), "{:?}", validation.to_vec()),
        }
        match case {
            Case::CompleteOmission | Case::CompleteReplacement | Case::CompleteSuffix => {
                assert_eq!(observed.file.to_chat_string(), source);
                let native = Observation::parse(&native, &source);
                assert!(native.diagnostics.is_empty(), "{:?}", native.diagnostics);
                assert!(observed.file.semantic_eq(&native.file));
            }
            // Recovery models need not match backends or reproduce rejected syntax.
            Case::MissingReplacementBody | Case::MissingSuffixBody => {}
        }
    }
}
