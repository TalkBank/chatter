//! One-speaker CHAT fixtures for parse-backed tests of model METHODS.
//!
//! A test of a model method used to build its input by hand:
//! `Word::new_unchecked("hello@s", "hello")` beside a comment saying
//! `// *CHI:\thello@s .`. The struct and the comment were two statements of one
//! fact held together by nothing. Here a test writes the CHAT lines it means,
//! and the model is whatever the parser builds from them.
//!
//! # Every fixture states its own validity
//!
//! The first draft of these tests refused only PARSE diagnostics, and a review
//! found two fixtures that were invalid CHAT the helper accepted (an `@ID`
//! language absent from `@Languages`, E519; bullets with no `@Media`, E752).
//! Running validation and refusing everything would be wrong too: some
//! fixtures exist to reach a validation code. So [`SingleSpeaker::parsed`]
//! takes the codes the fixture is EXPECTED to carry, `&[]` for valid CHAT,
//! and refuses any other verdict. A fixture cannot be invalid without the test
//! saying so, WITHIN THE VERDICT'S SCOPE, which is narrower than `chatter
//! validate`'s: the default rule selection (so the opt-in linker-pairing rules
//! do not run unless a `Dialogue` asks for them), and no transcript name (so
//! the rules about a file's name, E531 among them, never run).
//!
//! # The `@ID` language is derived; `@Media` is stated
//!
//! The `@ID` language is a function of `@Languages` (its first code, E519
//! otherwise) and is derived. `@Media` is stated, not derived: whether a
//! fixture means to declare media is the test's claim, and the verdict
//! check refuses a wrong statement either way (a bullet, main-tier or
//! `%wor`, without `@Media` is E752; `@Media` with no bullet is E544). A
//! first draft derived it from the presence of a bullet and hid exactly the
//! defect that became the 2026-09-08 ruling: until then a bullet INSIDE the
//! main tier counted for neither rule.

use talkbank_model::model::{ChatFile, Line, MainTier, TranscriptName, Utterance};
use talkbank_model::{ErrorCollector, ParseError, RuleSelection};
use talkbank_parser::TreeSitterParser;

use crate::test_error::TestError;

/// Whether the header declares the media that the transcript's timing
/// indexes.
///
/// `@Media` beside any bullet, main-tier (final or internal) or `%wor`, is
/// required (E752 without it); `@Media` beside no timing evidence is refused
/// (E544). Internal main-tier bullets joined that union on 2026-09-08.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Media {
    /// No `@Media` header.
    Undeclared,
    /// `@Media:` followed by a tab and `corpus, audio`.
    Declared,
}

/// A one-speaker transcript built around the lines a test writes.
#[derive(Debug, Clone, Copy)]
pub struct SingleSpeaker<'a> {
    /// The `@Languages` value, comma-separated; the first is the `@ID`
    /// language.
    pub languages: &'a str,
    /// An `@Options` value, placed between `@Participants` and `@ID`, which
    /// is its one legal position (E551 after `@ID`, E543 before
    /// `@Participants`).
    pub options: Option<&'a str>,
    /// Whether an `@Media` header is present.
    pub media: Media,
    /// The speaker's lines: a `*CHI:` main tier and any dependent tiers,
    /// newline-separated, with no trailing newline.
    pub lines: &'a str,
}

impl<'a> SingleSpeaker<'a> {
    /// English, no options, no media: the common case.
    pub const fn english(lines: &'a str) -> Self {
        Self {
            languages: "eng",
            options: None,
            media: Media::Undeclared,
            lines,
        }
    }

    /// The whole file, header derived from the lines.
    pub fn source(&self) -> String {
        let id_language = self
            .languages
            .split(',')
            .next()
            .map(str::trim)
            .unwrap_or(self.languages);
        let options = match self.options {
            Some(options) => format!("@Options:\t{options}\n"),
            None => String::new(),
        };
        let media = match self.media {
            Media::Declared => "@Media:\tcorpus, audio\n",
            Media::Undeclared => "",
        };
        format!(
            "@UTF8\n@Begin\n@Languages:\t{languages}\n@Participants:\tCHI Target_Child\n\
             {options}@ID:\t{id_language}|corpus|CHI|||||Target_Child|||\n{media}{lines}\n@End\n",
            languages = self.languages,
            lines = self.lines,
        )
    }

    /// Parse the fixture, having first checked that parsing AND validating it
    /// under the default rule selection reports exactly `expected_codes` (as a
    /// set; `&[]` means valid CHAT).
    ///
    /// The verdict is taken from a separate parse, so the returned file has
    /// had no validator run over it and carries no computed state a method
    /// under test might otherwise inherit.
    pub fn parsed(&self, expected_codes: &[&str]) -> Result<ChatFile, TestError> {
        parsed_with_verdict(&self.source(), self.lines, expected_codes, Rules::Default)
    }

    /// The first utterance of the parsed fixture.
    pub fn utterance(&self, expected_codes: &[&str]) -> Result<Utterance, TestError> {
        let file = self.parsed(expected_codes)?;
        file.lines
            .into_iter()
            .find_map(|line| match line {
                Line::Utterance(utterance) => Some(*utterance),
                _ => None,
            })
            .ok_or_else(|| TestError::Failure(format!("{:?} built no utterance", self.lines)))
    }

    /// The first utterance's main tier.
    pub fn main_tier(&self, expected_codes: &[&str]) -> Result<MainTier, TestError> {
        Ok(self.utterance(expected_codes)?.main)
    }
}

/// The rule selection a fixture is validated under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rules {
    /// What `chatter validate` runs with no flags.
    Default,
    /// `--strict-linkers`: the opt-in cross-utterance pairing rules for
    /// quotation and completion linkers.
    StrictLinkers,
}

impl Rules {
    fn selection(self) -> RuleSelection {
        match self {
            Rules::Default => RuleSelection::new(),
            Rules::StrictLinkers => RuleSelection::new().with_strict_linkers(),
        }
    }
}

/// A three-participant transcript (`CHI`, `MOT`, `EXP`) around the lines a
/// test writes, for rules about SEQUENCES of utterances by different
/// speakers.
#[derive(Debug, Clone, Copy)]
pub struct Dialogue<'a> {
    /// `(speaker code, body)` per line, in order; each body is the text after
    /// the tab, terminator included.
    pub lines: &'a [(&'a str, &'a str)],
}

impl Dialogue<'_> {
    /// The whole file.
    pub fn source(&self) -> String {
        let mut source = String::from(
            "@UTF8\n@Begin\n@Languages:\teng\n\
             @Participants:\tCHI Target_Child, MOT Mother, EXP Investigator\n\
             @ID:\teng|corpus|CHI|||||Target_Child|||\n\
             @ID:\teng|corpus|MOT|||||Mother|||\n\
             @ID:\teng|corpus|EXP|||||Investigator|||\n",
        );
        for (speaker, body) in self.lines {
            source.push('*');
            source.push_str(speaker);
            source.push_str(":\t");
            source.push_str(body);
            source.push('\n');
        }
        source.push_str("@End\n");
        source
    }

    /// Every diagnostic parsing and validating the fixture reports under
    /// `rules`: the table's own verdict, when the table is about diagnostics.
    pub fn diagnostics(&self, rules: Rules) -> Result<Vec<ParseError>, TestError> {
        diagnostics_of(&self.source(), rules)
    }

    /// Parse the fixture, having first checked that it reports exactly
    /// `expected_codes` under `rules`; see [`SingleSpeaker::parsed`].
    pub fn parsed(&self, expected_codes: &[&str], rules: Rules) -> Result<ChatFile, TestError> {
        let source = self.source();
        let described = self
            .lines
            .iter()
            .map(|(speaker, body)| format!("*{speaker}: {body}"))
            .collect::<Vec<_>>()
            .join(" / ");
        parsed_with_verdict(&source, &described, expected_codes, rules)
    }
}

/// Parse and validate `source` under `rules`, returning every diagnostic.
pub fn diagnostics_of(source: &str, rules: Rules) -> Result<Vec<ParseError>, TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let verdict = ErrorCollector::new();
    let mut checked = parser.parse_chat_file_streaming(source, &verdict);
    checked.validate_with_alignment_and_rules(
        rules.selection(),
        &verdict,
        TranscriptName::Anonymous,
    );
    Ok(verdict.into_vec())
}

/// The verdict check behind `parsed`: refuse any code set but `expected_codes`,
/// then return a fresh parse that no validator has touched.
/// Parse any CHAT source, having first checked that parsing AND validating
/// it under `rules` reports exactly `expected_codes` (as a set; `&[]` means
/// valid CHAT), like [`SingleSpeaker::parsed`] for a document a test wrote
/// itself.
pub fn parsed_document(
    source: &str,
    expected_codes: &[&str],
    rules: Rules,
) -> Result<ChatFile, TestError> {
    parsed_with_verdict(source, source, expected_codes, rules)
}

fn parsed_with_verdict(
    source: &str,
    described_as: &str,
    expected_codes: &[&str],
    rules: Rules,
) -> Result<ChatFile, TestError> {
    let reported = diagnostics_of(source, rules)?;
    let mut got: Vec<&str> = reported.iter().map(|e| e.code.as_str()).collect();
    got.sort_unstable();
    got.dedup();
    let mut want: Vec<&str> = expected_codes.to_vec();
    want.sort_unstable();
    want.dedup();
    if got != want {
        return Err(TestError::Failure(format!(
            "fixture {described_as:?} was expected to report {want:?} and reported {got:?}: {}",
            reported
                .iter()
                .map(|e| format!("{} {}", e.code.as_str(), e.message))
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let fresh = ErrorCollector::new();
    Ok(parser.parse_chat_file_streaming(source, &fresh))
}

/// One code a fixture must report exactly once, with a fragment its message
/// must contain.
#[derive(Debug, Clone, Copy)]
pub struct Fired {
    /// The code.
    pub code: &'static str,
    /// A fragment of the message, as the original test asserted it.
    pub message: &'static str,
}

/// The verdict a diagnostic table asserts per row: `reported` is EXACTLY the
/// codes in `fired` (an empty `fired` means the fixture is valid), each
/// exactly once, each message containing its fragment.
///
/// Returns the mismatches as sentences prefixed with `what`, so a table can
/// collect every row's failures into one report rather than stop at the
/// first.
pub fn fired_mismatches(what: &str, reported: &[ParseError], fired: &[Fired]) -> Vec<String> {
    let mut wrong = Vec::new();
    let mut got: Vec<&str> = reported.iter().map(|e| e.code.as_str()).collect();
    got.sort_unstable();
    let mut want: Vec<&str> = fired.iter().map(|f| f.code).collect();
    want.sort_unstable();
    if got != want {
        wrong.push(format!(
            "{what}: expected exactly {want:?} and got {got:?}: {}",
            reported
                .iter()
                .map(|e| format!("{} {}", e.code.as_str(), e.message))
                .collect::<Vec<_>>()
                .join("; ")
        ));
        return wrong;
    }
    for f in fired {
        let matching: Vec<_> = reported
            .iter()
            .filter(|e| e.code.as_str() == f.code)
            .collect();
        match matching.as_slice() {
            [one] if one.message.contains(f.message) => {}
            [one] => wrong.push(format!(
                "{what}: {}'s message {:?} does not contain {:?}",
                f.code, one.message, f.message
            )),
            many => wrong.push(format!(
                "{what}: expected exactly one {} and got {}",
                f.code,
                many.len()
            )),
        }
    }
    wrong
}
