//! Native main-tier carriers, recovery retention and public fragment boundaries.
//! Controls own parsed/validated models; candidates borrow those immutable bytes.

use super::assert_fragment_rebases;
use std::ops::Range;
use talkbank_model::alignment::helpers::{WordItem, walk_words};
use talkbank_model::model::{
    CAElementType, ChatFile, SemanticEq, TranscriptName, Word, WordContent, WordStressMarkerType,
    WriteChat,
};
use talkbank_model::{ChatParser, ErrorCollector, ParseOutcome};
use talkbank_parser::TreeSitterParser;
use talkbank_parser::generated_traversal::{AsRawNode, MainTierNode, SourceSlotView};
use talkbank_parser_tests::repo_paths::workspace_root;
use talkbank_parser_tests::test_error::strict_parse;

const RECOVERY: &str =
    include_str!("../../../../corpus/reference/content/main-tier-recovery-controls.cha");
const STACKED: &str =
    include_str!("../../../../corpus/reference/word-features/stacked-marker-admission.cha");

#[path = "body_ending_recovery_corpus.rs"]
mod body_ending_contracts;

#[path = "prelude_integration_corpus.rs"]
mod prelude_contracts;

use admitted::{AdmittedControl, OwnedEdit, PrefixField};

// Only this module can mint edits from an admitted source/model.
mod admitted {
    use super::*;

    pub(super) struct AdmittedControl {
        source: &'static str,
        file: ChatFile,
    }

    impl AdmittedControl {
        pub(super) fn parse(source: &'static str) -> Self {
            let parser = TreeSitterParser::new().expect("parser");
            let mut file =
                strict_parse(parser.parse_chat_file(source)).expect("clean control parse");
            let errors = ErrorCollector::new();
            file.validate_with_alignment(&errors, TranscriptName::Anonymous);
            assert!(errors.is_empty(), "control validity: {:?}", errors.to_vec());
            assert_eq!(file.to_chat_string(), source, "control source spelling");
            Self { source, file }
        }

        pub(super) fn source(&self) -> &'static str {
            self.source
        }
        pub(super) fn file(&self) -> &ChatFile {
            &self.file
        }

        pub(super) fn word(&self, spelling: &str) -> &Word {
            let mut matches = Vec::new();
            for utterance in self.file.utterances() {
                walk_words(&utterance.main.content.content, None, &mut |item| {
                    let word = match item {
                        WordItem::Word(word) => word,
                        WordItem::ReplacedWord(word) => &word.word,
                        WordItem::Separator(_) => return,
                    };
                    if self
                        .source
                        .get(word.span.start as usize..word.span.end as usize)
                        == Some(spelling)
                    {
                        matches.push(word);
                    }
                });
            }
            assert_eq!(matches.len(), 1, "unique admitted word: {spelling}");
            matches[0]
        }

        pub(super) fn prefix_edit(&self, field: PrefixField) -> OwnedEdit<'_> {
            let parser = TreeSitterParser::new().expect("parser");
            let parsed = parser
                .parse_source_incremental(self.source, None)
                .expect("native owner");
            let root = parsed.root().expect("source owner");
            let expected = self
                .file
                .utterances()
                .nth(1)
                .expect("prefix target")
                .main
                .span;
            for node in root.descendants() {
                let node = node.expect("readable native range");
                let Some(main) = node.typed::<MainTierNode>() else {
                    continue;
                };
                if main.raw_node().start_byte() != expected.start as usize {
                    continue;
                }
                let associated = main.extract();
                macro_rules! token_range {
                    ($name:ident) => {
                        match associated.$name().slot().view() {
                            SourceSlotView::Present(token) => {
                                let token = token.read().expect("same native owner");
                                token.raw_node().byte_range()
                            }
                            other => panic!("control prefix recovery: {other:?}"),
                        }
                    };
                }
                let range = match field {
                    PrefixField::Star => token_range!(field_child_0),
                    PrefixField::Speaker => token_range!(field_speaker),
                    PrefixField::Colon => token_range!(field_child_2),
                    PrefixField::Tab => token_range!(field_child_3),
                };
                let text = match field {
                    PrefixField::Star => "*",
                    PrefixField::Speaker => "SPK",
                    PrefixField::Colon => ":",
                    PrefixField::Tab => "\t",
                };
                assert_eq!(self.source.get(range.clone()), Some(text));
                return self.own_edit(range);
            }
            panic!("admitted main tier has its native carrier");
        }

        pub(super) fn word_edit(&self, spelling: &str) -> OwnedEdit<'_> {
            let word = self.word(spelling);
            self.own_edit(word.span.start as usize..word.span.end as usize)
        }

        pub(super) fn prelude_edit(&self, word: &str, field: PreludeField) -> OwnedEdit<'_> {
            let word = self.word(word);
            let main = &self
                .file
                .utterances()
                .find(|utterance| {
                    let span = utterance.main.span;
                    span.start <= word.span.start && word.span.end <= span.end
                })
                .expect("word's admitted prelude")
                .main;
            let (span, spelling) = match field {
                PreludeField::Linker(index) => {
                    let token = main.content.linkers.get(index).expect("admitted linker");
                    (token.span, token.to_string())
                }
                PreludeField::LanguageCode => (
                    main.content
                        .language_code_span
                        .expect("admitted precode span"),
                    format!(
                        "[- {}]",
                        main.content
                            .language_code
                            .as_ref()
                            .expect("admitted language code")
                            .as_str()
                    ),
                ),
            };
            assert_eq!(
                self.source.get(span.start as usize..span.end as usize),
                Some(spelling.as_str()),
                "complete modeled prelude token"
            );
            self.own_edit(span.start as usize..span.end as usize)
        }

        pub(super) fn ending_edit(&self, word: &str, field: EndingField) -> OwnedEdit<'_> {
            let word = self.word(word);
            let main = &self
                .file
                .utterances()
                .find(|utterance| {
                    let span = utterance.main.span;
                    span.start <= word.span.start && word.span.end <= span.end
                })
                .expect("word's admitted main tier")
                .main;
            let (span, spelling) = match field {
                EndingField::Terminator => {
                    let token = main
                        .content
                        .terminator
                        .as_ref()
                        .expect("admitted terminator");
                    (token.span(), token.to_chat_string())
                }
                EndingField::FirstPostcode => {
                    let token = main.content.postcodes.first().expect("admitted postcode");
                    (token.span, token.to_chat_string())
                }
                EndingField::Bullet => {
                    let token = main
                        .content
                        .bullet
                        .as_ref()
                        .expect("admitted terminal bullet");
                    (token.span, token.to_chat_string())
                }
            };
            assert_eq!(
                self.source.get(span.start as usize..span.end as usize),
                Some(spelling.as_str()),
                "complete modeled ending token"
            );
            self.own_edit(span.start as usize..span.end as usize)
        }

        fn own_edit(&self, range: Range<usize>) -> OwnedEdit<'_> {
            assert!(self.source.get(range.clone()).is_some(), "UTF-8 owned edit");
            let main = self
                .file
                .utterances()
                .find(|utterance| {
                    let span = utterance.main.span;
                    span.start as usize <= range.start && range.end <= span.end as usize
                })
                .expect("edit is inside one admitted main tier");
            OwnedEdit {
                control: self,
                range,
                tier: main.main.span.start as usize..main.main.span.end as usize,
            }
        }
    }

    #[derive(Clone, Copy)]
    pub(super) enum PrefixField {
        Star,
        Speaker,
        Colon,
        Tab,
    }

    #[derive(Clone, Copy)]
    pub(super) enum EndingField {
        Terminator,
        FirstPostcode,
        Bullet,
    }

    #[derive(Clone, Copy)]
    pub(super) enum PreludeField {
        Linker(usize),
        LanguageCode,
    }

    pub(super) struct OwnedEdit<'control> {
        control: &'control AdmittedControl,
        range: Range<usize>,
        tier: Range<usize>,
    }

    impl<'control> OwnedEdit<'control> {
        pub(super) fn into_parts(self) -> (&'control AdmittedControl, Range<usize>, Range<usize>) {
            (self.control, self.range, self.tier)
        }
    }
}

/// Only a control-owned token/word range can construct the next source revision.
struct Candidate<'control> {
    control: &'control AdmittedControl,
    source: String,
    edit: Range<usize>,
    replacement_len: usize,
    original_tier: Range<usize>,
    tier: Range<usize>,
}

impl<'control> Candidate<'control> {
    fn replace(edit: OwnedEdit<'control>, replacement: &str) -> Self {
        let (control, edit, original_tier) = edit.into_parts();
        let prefix = control.source().get(..edit.start).expect("UTF-8 prefix");
        let suffix = control.source().get(edit.end..).expect("UTF-8 suffix");
        let delta = replacement.len() as isize - edit.len() as isize;
        Self {
            control,
            source: format!("{prefix}{replacement}{suffix}"),
            edit,
            replacement_len: replacement.len(),
            tier: original_tier.start
                ..original_tier
                    .end
                    .checked_add_signed(delta)
                    .expect("translated tier end"),
            original_tier,
        }
    }

    fn word(control: &'control AdmittedControl, word: &str, replacement: &str) -> Self {
        Self::replace(control.word_edit(word), replacement)
    }

    fn fixture(&self, stem: &str, index: usize) -> std::path::PathBuf {
        let path = workspace_root().join(format!(
            "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/{stem}_{index}.cha"
        ));
        let source = std::fs::read_to_string(&path).expect("canonical generated example");
        assert_eq!(
            source, self.source,
            "source-bound spec mutation: {stem}_{index}"
        );
        assert_eq!(
            self.source.get(..self.edit.start),
            self.control.source().get(..self.edit.start)
        );
        assert_eq!(
            self.source.get(self.edit.start + self.replacement_len..),
            self.control.source().get(self.edit.end..)
        );
        path
    }

    fn translated(&self, span: talkbank_model::Span) -> Range<usize> {
        let range = span.start as usize..span.end as usize;
        if range.start >= self.edit.end {
            let delta = self.replacement_len as isize - self.edit.len() as isize;
            range
                .start
                .checked_add_signed(delta)
                .expect("translated start")
                ..range.end.checked_add_signed(delta).expect("translated end")
        } else {
            range
        }
    }
}

#[derive(Clone, Copy)]
enum Verdict {
    Parse(&'static str),
    /// Incomplete constructs may be diagnosed elsewhere within their owner.
    ParseInTier(&'static str),
    Validation(&'static str),
    /// Some file policies locate their finding at the owning main-tier start.
    ValidationInTier(&'static str),
}

#[derive(Clone, Copy)]
enum NativeNeighbors {
    Isolated,
    AbsorbsNext,
}

/// Healthy neighboring CST owners must survive lowering; recovery is not validity.
fn observe(
    candidate: &Candidate<'_>,
    verdict: Verdict,
    neighbors: NativeNeighbors,
    failures: &mut Vec<String>,
) -> ChatFile {
    let parser = TreeSitterParser::new().expect("parser");
    let native = parser
        .parse_source_incremental(&candidate.source, None)
        .expect("native producer");
    let root = native.root().expect("native source owner");
    let recovery: Vec<_> = root
        .descendants()
        .filter_map(|node| {
            let node = node.expect("native readable source").raw_node();
            node.is_error().then(|| node.byte_range())
        })
        .collect();
    let healthy: Vec<_> = root
        .descendants()
        .filter_map(|node| {
            let node = node.expect("native readable source");
            let main = node.typed::<MainTierNode>()?;
            (!main.raw_node().has_error()).then(|| main.raw_node().byte_range())
        })
        .collect();
    let errors = ErrorCollector::new();
    let mut file = parser.parse_chat_file_streaming(&candidate.source, &errors);
    let diagnostics = errors.to_vec();
    let validation = ErrorCollector::new();
    file.validate_with_alignment(&validation, TranscriptName::Anonymous);
    let validation_diagnostics = validation.to_vec();
    let code = match verdict {
        Verdict::Parse(code) | Verdict::ParseInTier(code) => {
            if !diagnostics.iter().any(|error| error.code.as_str() == code) {
                failures.push(format!("missing parse {code}: {diagnostics:?}"));
            }
            code
        }
        Verdict::Validation(code) | Verdict::ValidationInTier(code) => {
            if !diagnostics.is_empty() {
                failures.push(format!("unexpected parse recovery: {diagnostics:?}"));
            }
            if !validation_diagnostics
                .iter()
                .any(|error| error.code.as_str() == code)
            {
                failures.push(format!(
                    "missing validation {code}: {:?}",
                    validation_diagnostics
                ));
            }
            code
        }
    };
    let focus = match verdict {
        Verdict::ParseInTier(_) | Verdict::ValidationInTier(_) => candidate.tier.clone(),
        Verdict::Parse(_) | Verdict::Validation(_) => {
            candidate.edit.start..candidate.edit.start + candidate.replacement_len
        }
    };
    if !diagnostics
        .iter()
        .chain(&validation_diagnostics)
        .any(|error| {
            let span = error.location.span;
            error.code.as_str() == code
                && span.start as usize <= focus.end
                && focus.start <= span.end as usize
        })
    {
        failures.push(format!("{code}: no diagnostic names the owned mutation"));
    }
    for error in diagnostics.iter().chain(&validation_diagnostics) {
        let span = error.location.span;
        if candidate
            .source
            .get(span.start as usize..span.end as usize)
            .is_none()
        {
            failures.push(format!(
                "{code}: diagnostic escapes UTF-8 source: {error:?}"
            ));
        }
        for label in &error.labels {
            let span = label.span;
            if candidate
                .source
                .get(span.start as usize..span.end as usize)
                .is_none()
            {
                failures.push(format!("{code}: label escapes UTF-8 source: {label:?}"));
            }
        }
    }
    let mut absorbed = 0;
    for expected in candidate.control.file().utterances() {
        if expected.main.span.start as usize == candidate.tier.start {
            continue;
        }
        let range = candidate.translated(expected.main.span);
        if matches!(neighbors, NativeNeighbors::AbsorbsNext)
            && expected.main.span.start as usize == candidate.original_tier.end
        {
            absorbed += 1;
            let body_end = range.start
                + candidate
                    .source
                    .get(range.clone())
                    .expect("translated next tier")
                    .trim_end_matches('\n')
                    .len();
            if healthy.contains(&range)
                || !recovery
                    .iter()
                    .any(|error| error.start <= candidate.tier.start && body_end <= error.end)
                || file
                    .utterances()
                    .any(|utterance| utterance.main.span.start as usize == range.start)
            {
                failures.push(format!(
                    "{code}: expected next-turn absorption changed: {range:?}"
                ));
            }
            continue;
        }
        if !healthy.contains(&range) {
            failures.push(format!(
                "{code}: unchanged neighboring native tier is not healthy: {range:?}"
            ));
            continue;
        }
        let retained: Vec<_> = file
            .utterances()
            .filter(|utterance| {
                utterance.main.span.start as usize == range.start
                    && utterance.main.span.end as usize == range.end
            })
            .collect();
        if retained.len() != 1 || !expected.semantic_eq(retained[0]) {
            failures.push(format!(
                "{code}: healthy source-owned neighbor lost or changed: {range:?}"
            ));
        }
    }
    if matches!(neighbors, NativeNeighbors::AbsorbsNext) && absorbed != 1 {
        failures.push(format!(
            "{code}: expected exactly one absorbed neighboring turn"
        ));
    }
    file
}

#[test]
fn prefix_mutations_keep_native_neighbors_and_reject_public_fragments() {
    let control = AdmittedControl::parse(RECOVERY);
    use PrefixField::{Colon, Speaker, Star, Tab};
    let mut failures = Vec::new();
    for (index, field, replacement) in [
        (2, Star, ""),
        (3, Speaker, ""),
        (4, Colon, ""),
        (5, Tab, ""),
        (6, Colon, "::"),
        (7, Speaker, "SP K"),
        (8, Tab, " "),
    ] {
        let candidate = Candidate::replace(control.prefix_edit(field), replacement);
        let path = candidate.fixture("E316_main_tier_recovery", index);
        let code = match index {
            2 => "E326", // Without a star this is an unprefixed line, not a main tier.
            _ => "E316",
        };
        observe(
            &candidate,
            Verdict::Parse(code),
            NativeNeighbors::Isolated,
            &mut failures,
        );
        let parser = TreeSitterParser::new().expect("parser");
        let input = candidate
            .source
            .get(candidate.tier.clone())
            .expect("mutated tier range");
        for input in [input, input.trim_end_matches('\n')] {
            let errors = ErrorCollector::new();
            if !matches!(
                ChatParser::parse_main_tier(&parser, input, 0, &errors),
                ParseOutcome::Rejected
            ) {
                failures.push(format!("prefix {index}: invalid fragment admitted"));
            }
            assert_fragment_rebases(&path, input, |offset, errors| {
                ChatParser::parse_main_tier(&parser, input, offset, errors)
            });
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn malformed_word_specs_preserve_healthy_source_owned_turns() {
    let control = AdmittedControl::parse(RECOVERY);
    let mut failures = Vec::new();
    for (index, word, replacement) in [
        (9, "&-hm", "&"),
        (10, "&-hm", "&-"),
        (11, "&-hm", "&~"),
        (12, "willow", "]"),
        (13, "ash", "&"),
    ] {
        let candidate = Candidate::word(&control, word, replacement);
        let path = candidate.fixture("E316_main_tier_recovery", index);
        // A bare ampersand inside a group lets native other_spoken_event recovery
        // consume the next prefix; that turn is not a healthy main-tier carrier.
        let neighbors = if index == 13 {
            NativeNeighbors::AbsorbsNext
        } else {
            NativeNeighbors::Isolated
        };
        observe(&candidate, Verdict::Parse("E316"), neighbors, &mut failures);
        let parser = TreeSitterParser::new().expect("parser");
        let input = candidate
            .source
            .get(candidate.tier.clone())
            .expect("mutated tier range");
        for input in [input, input.trim_end_matches('\n')] {
            assert_fragment_rebases(&path, input, |offset, errors| {
                ChatParser::parse_main_tier(&parser, input, offset, errors)
            });
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn marker_only_words_are_rejected_without_erasing_their_native_tier() {
    let control = AdmittedControl::parse(RECOVERY);
    let mut failures = Vec::new();
    for (index, replacement) in [(2, "ˈ"), (3, "ˈˌ")] {
        let candidate = Candidate::word(&control, "ˈbirch", replacement);
        let path = candidate.fixture("E245_main_tier_recovery", index);
        let file = observe(
            &candidate,
            Verdict::Parse("E245"),
            NativeNeighbors::Isolated,
            &mut failures,
        );
        let retained = file
            .utterances()
            .find(|utterance| utterance.main.span.start as usize == candidate.tier.start)
            .expect("marker rejection retains its terminated tier");
        let mut words = Vec::new();
        walk_words(
            &retained.main.content.content,
            None,
            &mut |item| match item {
                WordItem::Word(word) => words.push(word),
                WordItem::ReplacedWord(word) => words.push(&word.word),
                WordItem::Separator(_) => {}
            },
        );
        if !words.is_empty() || retained.main.content.terminator.is_none() {
            failures.push(format!(
                "{index}: invented spoken word or lost native terminator"
            ));
        }
        let parser = TreeSitterParser::new().expect("parser");
        let input = candidate
            .source
            .get(candidate.tier.clone())
            .expect("mutated tier range");
        for input in [input, input.trim_end_matches('\n')] {
            assert_fragment_rebases(&path, input, |offset, errors| {
                ChatParser::parse_main_tier(&parser, input, offset, errors)
            });
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn stacked_pause_specs_preserve_the_word_and_the_validation_boundary() {
    let control = AdmittedControl::parse(STACKED);
    let mut failures = Vec::new();
    for (index, word, replacement) in [
        (2, "⌈⌊orchid⌋⌉", "^⌈⌊orchid⌋⌉"),
        (3, "⌈⌊orchid⌋⌉", "⌈^⌊orchid⌋⌉"),
        (4, "⌈⌊orchid⌋⌉", "⌈⌊orchid⌋⌉^"),
        (
            5,
            "⌈\u{2}\u{1}thistle\u{2}\u{2}⌉",
            "⌈\u{2}\u{1}^thistle\u{2}\u{2}⌉",
        ),
    ] {
        let candidate = Candidate::word(&control, word, replacement);
        let path = candidate.fixture("E252_stacked_pauses", index);
        let file = observe(
            &candidate,
            Verdict::Validation("E252"),
            NativeNeighbors::Isolated,
            &mut failures,
        );
        if file.to_chat_string() != candidate.source {
            failures.push(format!(
                "pause {index}: structured word source spelling lost"
            ));
        }
        let parser = TreeSitterParser::new().expect("parser");
        let input = candidate
            .source
            .get(candidate.tier.clone())
            .expect("mutated tier range");
        for input in [input, input.trim_end_matches('\n')] {
            let errors = ErrorCollector::new();
            if !matches!(
                ChatParser::parse_main_tier(&parser, input, 0, &errors),
                ParseOutcome::Parsed(_)
            ) || !errors.is_empty()
            {
                failures.push(format!(
                    "pause {index}: model policy incorrectly became parse rejection"
                ));
            }
            assert_fragment_rebases(&path, input, |offset, errors| {
                ChatParser::parse_main_tier(&parser, input, offset, errors)
            });
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[derive(Debug, PartialEq, Eq)]
enum Leaf<'a> {
    Text(&'a str),
    Overlap,
    Begin,
    End,
    PitchUp,
    Clitic,
    Compound,
    Primary,
    Pause,
}

#[test]
fn stacked_marker_controls_travel_the_admitted_typed_word_choices() {
    use Leaf::{
        Begin as B, Clitic as C, Compound as M, End as E, Overlap as O, Pause as S, PitchUp as U,
        Primary as P, Text as T,
    };
    let control = AdmittedControl::parse(STACKED);
    for (spelling, expected) in [
        ("⌈⌊orchid⌋⌉", vec![O, O, T("orchid"), O, O]),
        (
            "⌈\u{2}\u{1}thistle\u{2}\u{2}⌉",
            vec![O, B, T("thistle"), E, O],
        ),
        ("⌈clo↑ver⌉", vec![O, T("clo"), U, T("ver"), O]),
        (
            "⌈la~ven+der⌉",
            vec![O, T("la"), C, T("ven"), M, T("der"), O],
        ),
        ("⌈peˈtal⌉", vec![O, T("pe"), P, T("tal"), O]),
        ("⌈ir^is⌉", vec![O, T("ir"), S, T("is"), O]),
    ] {
        let word = control.word(spelling);
        let leaves: Vec<_> = word
            .content()
            .iter()
            .map(|item| match item {
                WordContent::Text(text) => T(text.as_ref()),
                WordContent::OverlapPoint(_) => O,
                WordContent::UnderlineBegin(_) => B,
                WordContent::UnderlineEnd(_) => E,
                WordContent::CAElement(value) if value.element_type == CAElementType::PitchUp => U,
                WordContent::CliticBoundary(_) => C,
                WordContent::CompoundMarker(_) => M,
                WordContent::StressMarker(value)
                    if value.marker_type == WordStressMarkerType::Primary =>
                {
                    P
                }
                WordContent::SyllablePause(_) => S,
                other => panic!("unexpected admitted leaf: {spelling} {other:?}"),
            })
            .collect();
        assert_eq!(leaves, expected, "{spelling}");
        assert_eq!(word.to_chat_string(), spelling);
    }
}

#[test]
fn admitted_recovery_controls_preserve_file_and_fragment_main_tiers() {
    let parser = TreeSitterParser::new().expect("parser");
    for (source, path) in [
        (RECOVERY, "content/main-tier-recovery-controls.cha"),
        (STACKED, "word-features/stacked-marker-admission.cha"),
    ] {
        let control = AdmittedControl::parse(source);
        let path = workspace_root().join("corpus/reference").join(path);
        for utterance in control.file().utterances() {
            let span = utterance.main.span;
            let input = source
                .get(span.start as usize..span.end as usize)
                .expect("owned main tier");
            for input in [input, input.trim_end_matches('\n')] {
                let errors = ErrorCollector::new();
                let actual = ChatParser::parse_main_tier(&parser, input, 0, &errors)
                    .into_option()
                    .expect("valid fragment");
                assert!(errors.is_empty(), "{input:?}: {:?}", errors.to_vec());
                assert!(utterance.main.semantic_eq(&actual));
                assert_fragment_rebases(&path, input, |offset, errors| {
                    ChatParser::parse_main_tier(&parser, input, offset, errors)
                });
            }
        }
    }
}
