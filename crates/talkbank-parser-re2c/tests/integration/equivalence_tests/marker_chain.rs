//! Valid reference semantics are distinct from invalid-input recovery choices.

use super::{both_parsers, compare_parses};
use talkbank_model::model::{
    BracketedItem, ChatFile, ContentStructure, GroupRef, UtteranceContent, WordRef,
};
use talkbank_model::{ChatParser, ErrorCollector, ParseOutcome, RuleSelection, TranscriptName};
use talkbank_parser_tests::chat_corpus::ChatCorpus;
use talkbank_parser_tests::test_error::strict_parse;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScopeControl {
    WordBefore,
    WordAfter,
    GroupBefore,
    ReplacementBefore,
}

/// Select authored controls through the parsed reference's original source.
/// The ordinal is shared across model comparisons; native spans are not an oracle.
fn controls(file: &ChatFile, source: &str) -> Vec<(usize, ScopeControl)> {
    let controls: Vec<_> = file
        .utterances()
        .enumerate()
        .filter_map(|(index, utterance)| {
            let span = utterance.main.location;
            let text = source
                .get(span.span().expect("located").to_range())
                .expect("reference main tier belongs to its source");
            let control = match text.trim_end() {
                "*CHI:\tdog [* p:w] [/] dog ." => ScopeControl::WordBefore,
                "*MOT:\tdog [/] [* p:w] dog ." => ScopeControl::WordAfter,
                "*CHI:\t<the dog> [* s:r] [/] the dog ." => ScopeControl::GroupBefore,
                "*CHI:\tmale [: female] [* s:r] [/] male [: female] [* s:r] ." => {
                    ScopeControl::ReplacementBefore
                }
                _ => return None,
            };
            Some((index, control))
        })
        .collect();
    for required in [
        ScopeControl::WordBefore,
        ScopeControl::WordAfter,
        ScopeControl::GroupBefore,
        ScopeControl::ReplacementBefore,
    ] {
        assert_eq!(
            controls
                .iter()
                .filter(|(_, actual)| *actual == required)
                .count(),
            1,
            "exactly one authored {required:?} control"
        );
    }
    controls
}

fn check_scopes(file: &ChatFile, controls: &[(usize, ScopeControl)], backend: &str) {
    let utterances: Vec<_> = file.utterances().collect();
    for &(index, expected) in controls {
        let first = utterances[index]
            .main
            .content
            .content
            .first()
            .expect("retraced material");
        let ContentStructure::Retrace(retrace) = first.structure() else {
            panic!("{backend} {expected:?}: first item must be the retrace");
        };
        let [material] = retrace.inner().content.content.as_slice() else {
            panic!("{backend} {expected:?}: keep the scoped seed intact");
        };
        let shape = material.structure();
        let expected_counts = match expected {
            ScopeControl::WordAfter => (1, 0),
            ScopeControl::WordBefore
            | ScopeControl::GroupBefore
            | ScopeControl::ReplacementBefore => (0, 1),
        };
        assert_eq!(
            (
                retrace.scoped_annotations().len(),
                shape.scoped_annotations().len()
            ),
            expected_counts,
            "{backend} {expected:?}: annotation ownership"
        );
        assert!(
            matches!(
                (expected, shape),
                (
                    ScopeControl::WordBefore,
                    ContentStructure::Word(WordRef::Annotated(_))
                ) | (
                    ScopeControl::WordAfter,
                    ContentStructure::Word(WordRef::Bare(_))
                ) | (
                    ScopeControl::GroupBefore,
                    ContentStructure::Group(GroupRef::Angle(_))
                ) | (
                    ScopeControl::ReplacementBefore,
                    ContentStructure::Word(WordRef::Replaced(_))
                )
            ),
            "{backend} {expected:?}: scoped material kind"
        );
    }
}

fn parse_and_validate<P: ChatParser>(
    parser: &P,
    source: &str,
    name: TranscriptName<'_>,
    controls: &[(usize, ScopeControl)],
    backend: &str,
    errors: &ErrorCollector,
) -> ParseOutcome<ChatFile> {
    let mut outcome = ChatParser::parse_chat_file(parser, source, 0, errors);
    if let ParseOutcome::Parsed(file) = &mut outcome {
        check_scopes(file, controls, backend);
        file.validate_with_alignment_and_rules(RuleSelection::new(), errors, name);
    }
    outcome
}

pub(super) fn valid_reference_scopes() -> Result<(), String> {
    let (ts, re2c) = both_parsers();
    let corpus = ChatCorpus::reference().map_err(|error| error.to_string())?;
    let fixture = corpus
        .fixtures()
        .iter()
        .find(|fixture| fixture.path().ends_with("annotation/retrace.cha"))
        .ok_or_else(|| "missing authored retrace reference".to_owned())?;
    let primary =
        strict_parse(ts.parse_chat_file(fixture.source())).map_err(|error| error.to_string())?;
    let controls = controls(&primary, fixture.source());
    compare_parses(
        |errors| {
            parse_and_validate(
                &ts,
                fixture.source(),
                TranscriptName::for_path(fixture.path()),
                &controls,
                "tree-sitter",
                errors,
            )
        },
        |errors| {
            parse_and_validate(
                &re2c,
                fixture.source(),
                TranscriptName::for_path(fixture.path()),
                &controls,
                "re2c",
                errors,
            )
        },
    )
}

/// Where a construct is, as both backends must agree on it: the construct
/// through its own extent only. Wrappers carry their inner value's location
/// beside their own, so a backend that stretched the inner value over the
/// wrapper's annotations would show it here; a retrace carries its marker.
#[derive(Debug, PartialEq, Eq)]
struct Placed {
    kind: &'static str,
    location: talkbank_model::Location,
    inner: Option<talkbank_model::Location>,
    marker: Option<talkbank_model::Location>,
    children: Vec<Placed>,
}

impl Placed {
    fn leaf(kind: &'static str, location: talkbank_model::Location) -> Self {
        Self {
            kind,
            location,
            inner: None,
            marker: None,
            children: Vec::new(),
        }
    }
    fn wrapped(
        kind: &'static str,
        location: talkbank_model::Location,
        inner: talkbank_model::Location,
    ) -> Self {
        Self {
            inner: Some(inner),
            ..Self::leaf(kind, location)
        }
    }
    fn container(
        kind: &'static str,
        location: talkbank_model::Location,
        inner: Option<talkbank_model::Location>,
        marker: Option<talkbank_model::Location>,
        children: &[talkbank_model::model::BracketedItem],
    ) -> Self {
        Self {
            kind,
            location,
            inner,
            marker,
            children: placed_bracketed(children),
        }
    }
}

/// The two content enums share the variants that carry a location; one
/// mapping serves both. The arm list is deliberately complete for the
/// located variants; the leaves without a location of their own (pauses,
/// separators, markers, bullets, phonological and sign groups) are the
/// catch-all, which this test does not place.
macro_rules! placed_walker {
    ($name:ident, $content:ident) => {
        fn $name(content: &[$content]) -> Vec<Placed> {
            content
                .iter()
                .filter_map(|item| {
                    Some(match item {
                        $content::Word(w) => Placed::leaf("word", w.location),
                        $content::AnnotatedWord(a) => {
                            Placed::wrapped("annotated word", a.location, a.inner.location)
                        }
                        $content::ReplacedWord(r) => {
                            Placed::wrapped("replaced word", r.location, r.word.location)
                        }
                        $content::Event(e) => Placed::leaf("event", e.location),
                        $content::AnnotatedEvent(a) => {
                            Placed::wrapped("annotated event", a.location, a.inner.location)
                        }
                        $content::Action(a) => Placed::leaf("action", a.location),
                        $content::AnnotatedAction(a) => {
                            Placed::wrapped("annotated action", a.location, a.inner.location)
                        }
                        $content::Group(g) => {
                            Placed::container("group", g.location, None, None, &g.content.content)
                        }
                        $content::AnnotatedGroup(a) => Placed::container(
                            "annotated group",
                            a.location,
                            Some(a.inner.location),
                            None,
                            &a.inner.content.content,
                        ),
                        $content::Quotation(q) => Placed::container(
                            "quotation",
                            q.location,
                            None,
                            None,
                            &q.content.content,
                        ),
                        $content::AnnotatedQuotation(a) => Placed::container(
                            "annotated quotation",
                            a.location,
                            Some(a.inner.location),
                            None,
                            &a.inner.content.content,
                        ),
                        $content::Retrace(r) => Placed::container(
                            "retrace",
                            r.location,
                            None,
                            Some(r.marker_location),
                            &r.content.content,
                        ),
                        $content::AnnotatedRetrace(a) => Placed::container(
                            "annotated retrace",
                            a.location,
                            Some(a.inner.location),
                            Some(a.inner.marker_location),
                            &a.inner.content.content,
                        ),
                        _ => return None,
                    })
                })
                .collect()
        }
    };
}

placed_walker!(placed_items, UtteranceContent);
placed_walker!(placed_bracketed, BracketedItem);

/// The two backends place every container, wrapper and word identically, at
/// every nesting depth: a construct's location is exactly the construct (a
/// group its brackets, a quotation its marks, a retrace its content through
/// its marker, a replaced word its word through the replacement's `]` and
/// then through its last scoped annotation) and a wrapper's is the construct
/// through its own last annotation. The maintainer's ruling of 2026-10-19 made this
/// the contract; before it tree-sitter gave a group and a retrace the whole
/// chain and nothing compared the two.
pub(super) fn container_locations_agree() {
    let (ts, re2c) = both_parsers();
    for line in [
        "*CHI:\t<a b> [/] c .",
        "*CHI:\ta [?] [/] [!] b .",
        "*CHI:\ta [?] [!] b .",
        "*CHI:\t<a [!] b> [//] again .",
        "*CHI:\t<a b> [?] [/] c .",
        "*CHI:\t<a b> [?] [!] c .",
        "*CHI:\t<a b> [/] [!] c .",
        "*CHI:\t\u{201C}a b\u{201D} [=! x] c .",
        "*CHI:\tdat [: that] [!] again .",
        "*CHI:\tdat [: that] [?] [!] x .",
        "*CHI:\tdat [: that] [/] x .",
        "*CHI:\tdat [: that] [/] [!] x .",
        "*CHI:\t&=laugh [=! softly] x .",
        "*CHI:\t&=laugh [/] x .",
        "*CHI:\t0 [=! points] x .",
        "*CHI:\t0 [/] x .",
        "*CHI:\tdog [* p:w] [/] dog .",
        "*CHI:\t<<a> [/]> [//] b .",
    ] {
        let ts_errors = ErrorCollector::new();
        let re2c_errors = ErrorCollector::new();
        let (ParseOutcome::Parsed(ts_tier), ParseOutcome::Parsed(re2c_tier)) = (
            ChatParser::parse_main_tier(&ts, line, 0, &ts_errors),
            ChatParser::parse_main_tier(&re2c, line, 0, &re2c_errors),
        ) else {
            panic!(
                "{line:?}: both backends must parse; tree-sitter {:?}, re2c {:?}",
                ts_errors.into_vec(),
                re2c_errors.into_vec()
            );
        };
        let ts_placed = placed_items(&ts_tier.content.content);
        let re2c_placed = placed_items(&re2c_tier.content.content);
        assert!(!ts_placed.is_empty(), "{line:?}: nothing placed");
        assert_eq!(
            ts_placed, re2c_placed,
            "{line:?}: the backends place containers differently"
        );
    }
}
