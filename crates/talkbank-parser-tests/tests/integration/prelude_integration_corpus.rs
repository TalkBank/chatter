//! Prelude integration consumes admitted token edits, never independent ranges.
//! File policies, lowering diagnostics and strict fragment admission stay distinct.

use super::admitted::PreludeField;
use super::*;
use talkbank_parser::generated_traversal::{
    LangcodeNode, NodeSlot, TierBodyNode, UtteranceEndNode,
};

const ORDINARY: &str = include_str!("../../../../corpus/reference/content/prelude-controls.cha");
const CA: &str = include_str!("../../../../corpus/reference/content/prelude-ca-controls.cha");

#[derive(Clone, Copy)]
enum ControlKind {
    Ordinary,
    CA,
}

enum Edit {
    Identity,
    Tab(&'static str),
    Word(&'static str, &'static str),
    Linker(&'static str, &'static str),
    Language(&'static str, &'static str),
}

#[derive(Clone, Copy)]
enum Expectation {
    Clean,
    Diagnostic(Verdict),
}

struct Case {
    index: usize,
    control: ControlKind,
    edit: Edit,
    expectation: Expectation,
}

impl Case {
    fn candidate<'a>(&self, control: &'a AdmittedControl) -> Candidate<'a> {
        match self.edit {
            Edit::Identity => Candidate::replace(control.prefix_edit(PrefixField::Tab), "\t"),
            Edit::Tab(text) => Candidate::replace(control.prefix_edit(PrefixField::Tab), text),
            Edit::Word(word, text) => Candidate::word(control, word, text),
            Edit::Linker(word, text) => {
                Candidate::replace(control.prelude_edit(word, PreludeField::Linker(0)), text)
            }
            Edit::Language(word, text) => {
                Candidate::replace(control.prelude_edit(word, PreludeField::LanguageCode), text)
            }
        }
    }
}

fn run_cases(stem: &str, cases: &[Case]) {
    let ordinary = AdmittedControl::parse(ORDINARY);
    let ca = AdmittedControl::parse(CA);
    let parser = TreeSitterParser::new().expect("parser");
    let mut failures = Vec::new();
    for case in cases {
        let control = match case.control {
            ControlKind::Ordinary => &ordinary,
            ControlKind::CA => &ca,
        };
        let candidate = case.candidate(control);
        let path = candidate.fixture(stem, case.index);
        let before = failures.len();
        let file = match case.expectation {
            Expectation::Diagnostic(verdict) => observe(
                &candidate,
                verdict,
                NativeNeighbors::Isolated,
                &mut failures,
            ),
            Expectation::Clean => {
                let errors = ErrorCollector::new();
                let mut file = parser.parse_chat_file_streaming(&candidate.source, &errors);
                file.validate_with_alignment(&errors, TranscriptName::Anonymous);
                if !errors.is_empty() {
                    failures.push(format!(
                        "{stem}_{}: clean case diagnostics: {:?}",
                        case.index,
                        errors.to_vec()
                    ));
                }
                for expected in control.file().utterances() {
                    if expected.main.span.start as usize == candidate.original_tier.start {
                        continue;
                    }
                    let range = candidate.translated(expected.main.span);
                    if !file.utterances().any(|actual| {
                        actual.main.span.start as usize == range.start
                            && actual.main.span.end as usize == range.end
                            && expected.semantic_eq(actual)
                    }) {
                        failures.push(format!(
                            "{stem}_{}: unchanged clean neighbor lost",
                            case.index
                        ));
                    }
                }
                file
            }
        };
        check_retention(&candidate, &file, &mut failures);
        for input in [
            candidate
                .source
                .get(candidate.tier.clone())
                .expect("owned main tier"),
            candidate
                .source
                .get(candidate.tier.clone())
                .expect("owned main tier")
                .trim_end_matches('\n'),
        ] {
            assert_fragment_rebases(&path, input, |offset, errors| {
                ChatParser::parse_main_tier(&parser, input, offset, errors)
            });
            let errors = ErrorCollector::new();
            let outcome = ChatParser::parse_main_tier(&parser, input, 0, &errors);
            match case.expectation {
                Expectation::Diagnostic(Verdict::Parse(code) | Verdict::ParseInTier(code)) => {
                    if !matches!(outcome, ParseOutcome::Rejected) || errors.is_empty() {
                        failures.push(format!(
                            "{stem}_{}: lowering/recovery accepted as a fragment",
                            case.index
                        ));
                    }
                    // A healthy misplaced-linker CST reaches semantic lowering in both APIs.
                    // Native recovery instead rejects before that deep classifier.
                    if code == "E766" && !errors.to_vec().iter().any(|e| e.code.as_str() == code) {
                        failures.push(format!(
                            "{stem}_{}: fragment lost typed linker diagnostic",
                            case.index
                        ));
                    }
                }
                Expectation::Clean
                | Expectation::Diagnostic(Verdict::Validation(_) | Verdict::ValidationInTier(_)) => {
                    if !matches!(outcome, ParseOutcome::Parsed(_)) || !errors.is_empty() {
                        failures.push(format!(
                            "{stem}_{}: a file-context policy became fragment rejection",
                            case.index
                        ));
                    }
                }
            }
        }
        for failure in &mut failures[before..] {
            *failure = format!("{stem}_{}: {failure}", case.index);
        }
    }
    assert!(
        failures.is_empty(),
        "complete prelude inventory:\n{}",
        failures.join("\n")
    );
}

fn check_retention(candidate: &Candidate<'_>, file: &ChatFile, failures: &mut Vec<String>) {
    let parser = TreeSitterParser::new().expect("parser");
    let native = parser
        .parse_source_incremental(&candidate.source, None)
        .expect("native owner");
    let root = native.root().expect("source owner");
    let expected = &candidate
        .control
        .file()
        .utterances()
        .find(|utterance| utterance.main.span.start as usize == candidate.original_tier.start)
        .expect("admitted target")
        .main;
    let Some(actual) = file
        .utterances()
        .find(|utterance| utterance.main.span.start as usize == candidate.tier.start)
        .map(|utterance| &utterance.main)
    else {
        failures.push("source-owned target main tier lost".to_owned());
        return;
    };
    let mut main = None;
    let mut body = None;
    let mut ending = None;
    let mut tokens = Vec::new();
    let mut recovery_ranges = Vec::new();
    for node in root.descendants() {
        let node = node.expect("native source bounds");
        let raw = node.raw_node();
        if raw.start_byte() < candidate.tier.start || candidate.tier.end < raw.end_byte() {
            continue;
        }
        if raw.is_error() {
            recovery_ranges.push(raw.byte_range());
        }
        if let Some(value) = node.typed::<MainTierNode>() {
            main = Some(value);
        }
        if let Some(value) = node.typed::<TierBodyNode>() {
            body = Some(value);
        }
        if let Some(value) = node.typed::<UtteranceEndNode>() {
            ending = Some(value);
        }
        if let Some(value) = node.typed::<LangcodeNode>() {
            tokens.push(value);
        }
    }
    if let Some(main) = main {
        let associated = main.extract();
        let slots = associated.children();
        let native_separator = match slots.child_4.slot() {
            Some(NodeSlot::Present(separator)) => match slots.child_3.slot().raw_node() {
                Some(tab) if tab.end_byte() == separator.raw_node().start_byte() => {
                    Some(separator.raw_node().byte_range())
                }
                Some(_) | None => None,
            },
            Some(NodeSlot::Missing(_) | NodeSlot::Error(_)) | None => None,
            Some(NodeSlot::Unexpected(never) | NodeSlot::Absent(never)) => match *never {},
        };
        let modeled_separator = actual
            .separator
            .trailing_space()
            .map(|span| span.start as usize..span.end as usize);
        if modeled_separator != native_separator {
            failures.push(
                "separator provenance lost or inferred from nonadjacent whitespace".to_owned(),
            );
        }
    } else {
        failures.push("native target main-tier carrier lost".to_owned());
    }
    let Some(body) = body else {
        failures.push("native prelude lost its body carrier".to_owned());
        return;
    };
    let associated = body.extract();
    if let Some(NodeSlot::Present(group)) = associated.children().language_code.slot()
        && let NodeSlot::Present(token) = group.child_0.slot()
    {
        let range = token.raw_node().byte_range();
        if actual
            .content
            .language_code_span
            .map(|span| span.start as usize..span.end as usize)
            != Some(range.clone())
        {
            failures.push(format!(
                "positioned language token lost its span: {range:?}"
            ));
        }
        if !token.raw_node().has_error() {
            let spelling = candidate.source.get(range).expect("native language source");
            if !actual
                .content
                .language_code
                .as_ref()
                .is_some_and(|code| spelling == format!("[- {}]", code.as_str()))
            {
                failures.push("healthy positioned language token lost its decoded code".to_owned());
            }
        }
    }
    if let Some(span) = actual.content.language_code_span {
        if !tokens
            .iter()
            .any(|token| token.raw_node().byte_range() == (span.start as usize..span.end as usize))
        {
            failures.push("model language span has no source-owned native token".to_owned());
        }
    } else if actual.content.language_code.is_some() {
        failures.push("decoded language has no token provenance".to_owned());
    }
    let Some(ending) = ending else {
        failures.push("native prelude lost its ending carrier".to_owned());
        return;
    };
    if ending.raw_node().has_error() {
        failures.push("this closed-prelude deck tainted its original healthy ending".to_owned());
    } else if !expected
        .content
        .terminator
        .semantic_eq(&actual.content.terminator)
        || !expected
            .content
            .postcodes
            .semantic_eq(&actual.content.postcodes)
        || !expected.content.bullet.semantic_eq(&actual.content.bullet)
    {
        failures.push("healthy ending payload changed during prelude lowering".to_owned());
    }
    let positioned_linkers = matches!(
        associated.children().linkers.slot(),
        Some(NodeSlot::Present(group)) if !group.raw_node().has_error()
    );
    if positioned_linkers {
        if actual.content.linkers.len() != expected.content.linkers.len() {
            failures.push("initial linker multiplicity changed during prelude lowering".to_owned());
        }
        for (original, token) in expected.content.linkers.iter().zip(&actual.content.linkers) {
            let original_range = original.span.start as usize..original.span.end as usize;
            let retained_range = if original_range == candidate.edit {
                candidate.edit.start..candidate.edit.start + candidate.replacement_len
            } else {
                candidate.translated(original.span)
            };
            let range = token.span.start as usize..token.span.end as usize;
            if range != retained_range
                || candidate.source.get(range) != Some(token.to_string().as_str())
            {
                failures.push("initial linker identity/order escaped its source owner".to_owned());
            }
        }
    } else {
        if !actual.content.linkers.is_empty() {
            failures.push("recovery was invented as positioned initial linkers".to_owned());
        }
        // These malformed-precode cases move original linkers under a native ERROR.
        // A non-Present slot is not evidence that the source contained no linkers.
        for original in &expected.content.linkers {
            let range = candidate.translated(original.span);
            if !recovery_ranges
                .iter()
                .any(|error| error.start <= range.start && range.end <= error.end)
            {
                failures
                    .push("unpositioned initial linker has no native recovery owner".to_owned());
            }
        }
    }
    let mut actual_words = Vec::new();
    walk_words(&actual.content.content, None, &mut |item| match item {
        WordItem::Word(word) => actual_words.push(word.to_chat_string()),
        WordItem::ReplacedWord(word) => actual_words.push(word.word.to_chat_string()),
        WordItem::Separator(_) => {}
    });
    let mut cursor = actual_words.iter();
    walk_words(&expected.content.content, None, &mut |item| {
        let word = match item {
            WordItem::Word(word) => word,
            WordItem::ReplacedWord(word) => &word.word,
            WordItem::Separator(_) => return,
        };
        if !cursor.any(|actual| *actual == word.to_chat_string()) {
            failures.push(format!(
                "healthy spoken word lost or reordered: {}",
                word.to_chat_string()
            ));
        }
    });
}

#[test]
fn admitted_controls_issue_complete_prelude_edit_capabilities() {
    for source in [ORDINARY, CA] {
        let control = AdmittedControl::parse(source);
        for (word, field, spelling) in [
            ("bamboo", PreludeField::Linker(0), "+<"),
            ("bamboo", PreludeField::Linker(1), "+^"),
            ("bamboo", PreludeField::LanguageCode, "[- zho]"),
            ("birch", PreludeField::LanguageCode, "[- eng]"),
            ("cedar", PreludeField::LanguageCode, "[- zho]"),
        ] {
            let (owner, range, tier) = control.prelude_edit(word, field).into_parts();
            assert!(std::ptr::eq(owner, &control));
            assert_eq!(owner.source().get(range.clone()), Some(spelling));
            assert!(tier.start <= range.start && range.end <= tier.end);
        }
    }
}

#[test]
fn canonical_preludes_preserve_order_language_and_ending_provenance() {
    for source in [ORDINARY, CA] {
        let control = AdmittedControl::parse(source);
        for word in ["bamboo", "birch", "cedar", "fern"] {
            let spelling = control.word(word).to_chat_string();
            let candidate = Candidate::word(&control, word, &spelling);
            let mut failures = Vec::new();
            check_retention(&candidate, control.file(), &mut failures);
            assert!(failures.is_empty(), "{word}: {failures:?}");
        }
    }
}

#[test]
fn separator_specs_distinguish_ca_and_ordinary_provenance() {
    run_cases(
        "E758_prelude",
        &[
            Case {
                index: 1,
                control: ControlKind::Ordinary,
                edit: Edit::Identity,
                expectation: Expectation::Clean,
            },
            Case {
                index: 2,
                control: ControlKind::Ordinary,
                edit: Edit::Tab("\t "),
                expectation: Expectation::Diagnostic(Verdict::Validation("E758")),
            },
            Case {
                index: 3,
                control: ControlKind::Ordinary,
                edit: Edit::Tab("\t  "),
                expectation: Expectation::Diagnostic(Verdict::Validation("E758")),
            },
            Case {
                index: 4,
                control: ControlKind::Ordinary,
                edit: Edit::Tab("\t\t"),
                expectation: Expectation::Diagnostic(Verdict::Parse("E370")),
            },
            Case {
                index: 5,
                control: ControlKind::CA,
                edit: Edit::Identity,
                expectation: Expectation::Clean,
            },
            Case {
                index: 6,
                control: ControlKind::CA,
                edit: Edit::Tab("\t "),
                expectation: Expectation::Clean,
            },
            Case {
                index: 7,
                control: ControlKind::CA,
                edit: Edit::Tab("\t  "),
                expectation: Expectation::Clean,
            },
            Case {
                index: 8,
                control: ControlKind::CA,
                edit: Edit::Tab("\t\t"),
                expectation: Expectation::Diagnostic(Verdict::Parse("E370")),
            },
        ],
    );
}

#[test]
fn misplaced_linker_specs_retain_prelude_and_surrounding_words() {
    run_cases(
        "E766_prelude",
        &[
            Case {
                index: 1,
                control: ControlKind::Ordinary,
                edit: Edit::Identity,
                expectation: Expectation::Clean,
            },
            Case {
                index: 2,
                control: ControlKind::Ordinary,
                edit: Edit::Word("bamboo", "bamboo +<"),
                expectation: Expectation::Diagnostic(Verdict::Parse("E766")),
            },
            Case {
                index: 3,
                control: ControlKind::Ordinary,
                edit: Edit::Word("birch", "birch +<"),
                expectation: Expectation::Diagnostic(Verdict::Parse("E766")),
            },
            Case {
                index: 4,
                control: ControlKind::Ordinary,
                edit: Edit::Word("cedar", "cedar +<"),
                expectation: Expectation::Diagnostic(Verdict::Parse("E766")),
            },
            Case {
                index: 5,
                control: ControlKind::Ordinary,
                edit: Edit::Word("fern", "fern +<"),
                expectation: Expectation::Diagnostic(Verdict::Parse("E766")),
            },
            Case {
                index: 6,
                control: ControlKind::Ordinary,
                edit: Edit::Language("bamboo", "[- zho] +<"),
                expectation: Expectation::Diagnostic(Verdict::Parse("E766")),
            },
            Case {
                index: 7,
                control: ControlKind::Ordinary,
                edit: Edit::Linker("bamboo", "+^"),
                expectation: Expectation::Clean,
            },
            Case {
                index: 8,
                control: ControlKind::CA,
                edit: Edit::Identity,
                expectation: Expectation::Clean,
            },
            Case {
                index: 9,
                control: ControlKind::CA,
                edit: Edit::Word("bamboo", "bamboo +<"),
                expectation: Expectation::Diagnostic(Verdict::Parse("E766")),
            },
            Case {
                index: 10,
                control: ControlKind::CA,
                edit: Edit::Word("birch", "birch +<"),
                expectation: Expectation::Diagnostic(Verdict::Parse("E766")),
            },
            Case {
                index: 11,
                control: ControlKind::CA,
                edit: Edit::Word("cedar", "cedar +<"),
                expectation: Expectation::Diagnostic(Verdict::Parse("E766")),
            },
            Case {
                index: 12,
                control: ControlKind::CA,
                edit: Edit::Word("fern", "fern +<"),
                expectation: Expectation::Diagnostic(Verdict::Parse("E766")),
            },
            Case {
                index: 13,
                control: ControlKind::CA,
                edit: Edit::Language("bamboo", "[- zho] +<"),
                expectation: Expectation::Diagnostic(Verdict::Parse("E766")),
            },
            Case {
                index: 14,
                control: ControlKind::CA,
                edit: Edit::Linker("bamboo", "+^"),
                expectation: Expectation::Clean,
            },
        ],
    );
}

#[test]
fn language_declaration_specs_keep_precode_and_ending_identity() {
    run_cases(
        "E755_prelude",
        &[
            Case {
                index: 1,
                control: ControlKind::Ordinary,
                edit: Edit::Identity,
                expectation: Expectation::Clean,
            },
            Case {
                index: 2,
                control: ControlKind::Ordinary,
                edit: Edit::Language("bamboo", "[- fra]"),
                expectation: Expectation::Diagnostic(Verdict::ValidationInTier("E755")),
            },
            Case {
                index: 3,
                control: ControlKind::Ordinary,
                edit: Edit::Language("bamboo", "[- eng]"),
                expectation: Expectation::Clean,
            },
            Case {
                index: 4,
                control: ControlKind::Ordinary,
                edit: Edit::Language("birch", "[- fra]"),
                expectation: Expectation::Diagnostic(Verdict::ValidationInTier("E755")),
            },
            Case {
                index: 5,
                control: ControlKind::Ordinary,
                edit: Edit::Language("birch", "[- zho]"),
                expectation: Expectation::Clean,
            },
            Case {
                index: 6,
                control: ControlKind::Ordinary,
                edit: Edit::Language("cedar", "[- fra]"),
                expectation: Expectation::Diagnostic(Verdict::ValidationInTier("E755")),
            },
            Case {
                index: 7,
                control: ControlKind::Ordinary,
                edit: Edit::Language("cedar", "[- eng]"),
                expectation: Expectation::Clean,
            },
            Case {
                index: 8,
                control: ControlKind::CA,
                edit: Edit::Identity,
                expectation: Expectation::Clean,
            },
            Case {
                index: 9,
                control: ControlKind::CA,
                edit: Edit::Language("bamboo", "[- fra]"),
                expectation: Expectation::Diagnostic(Verdict::ValidationInTier("E755")),
            },
            Case {
                index: 10,
                control: ControlKind::CA,
                edit: Edit::Language("bamboo", "[- eng]"),
                expectation: Expectation::Clean,
            },
            Case {
                index: 11,
                control: ControlKind::CA,
                edit: Edit::Language("birch", "[- fra]"),
                expectation: Expectation::Diagnostic(Verdict::ValidationInTier("E755")),
            },
            Case {
                index: 12,
                control: ControlKind::CA,
                edit: Edit::Language("birch", "[- zho]"),
                expectation: Expectation::Clean,
            },
            Case {
                index: 13,
                control: ControlKind::CA,
                edit: Edit::Language("cedar", "[- fra]"),
                expectation: Expectation::Diagnostic(Verdict::ValidationInTier("E755")),
            },
            Case {
                index: 14,
                control: ControlKind::CA,
                edit: Edit::Language("cedar", "[- eng]"),
                expectation: Expectation::Clean,
            },
        ],
    );
}

#[test]
fn malformed_precode_specs_preserve_native_recovery_and_fragment_refusal() {
    run_cases(
        "E316_prelude",
        &[
            Case {
                index: 1,
                control: ControlKind::Ordinary,
                edit: Edit::Identity,
                expectation: Expectation::Clean,
            },
            Case {
                index: 2,
                control: ControlKind::Ordinary,
                edit: Edit::Language("bamboo", "[- ]"),
                expectation: Expectation::Diagnostic(Verdict::ParseInTier("E316")),
            },
            Case {
                index: 3,
                control: ControlKind::Ordinary,
                edit: Edit::Language("bamboo", "[- ZHO]"),
                expectation: Expectation::Diagnostic(Verdict::ParseInTier("E316")),
            },
            Case {
                index: 4,
                control: ControlKind::Ordinary,
                edit: Edit::Language("bamboo", "[- z ho]"),
                expectation: Expectation::Diagnostic(Verdict::ParseInTier("E316")),
            },
            Case {
                index: 5,
                control: ControlKind::Ordinary,
                edit: Edit::Language("bamboo", "[/]"),
                expectation: Expectation::Diagnostic(Verdict::ParseInTier("E316")),
            },
            Case {
                index: 6,
                control: ControlKind::CA,
                edit: Edit::Identity,
                expectation: Expectation::Clean,
            },
            Case {
                index: 7,
                control: ControlKind::CA,
                edit: Edit::Language("bamboo", "[- ]"),
                expectation: Expectation::Diagnostic(Verdict::ParseInTier("E316")),
            },
            Case {
                index: 8,
                control: ControlKind::CA,
                edit: Edit::Language("bamboo", "[- ZHO]"),
                expectation: Expectation::Diagnostic(Verdict::ParseInTier("E316")),
            },
            Case {
                index: 9,
                control: ControlKind::CA,
                edit: Edit::Language("bamboo", "[- z ho]"),
                expectation: Expectation::Diagnostic(Verdict::ParseInTier("E316")),
            },
            Case {
                index: 10,
                control: ControlKind::CA,
                edit: Edit::Language("bamboo", "[/]"),
                expectation: Expectation::Diagnostic(Verdict::ParseInTier("E316")),
            },
        ],
    );
}
