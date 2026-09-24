//! Deliberate source edits admitted from diagnostic-free CHAT seeds.
//!
//! Candidates are not golden expectations. Admission records the chosen rules
//! and transcript identity, and refuses warnings as well as errors. It does not
//! certify validity under rules that the caller did not select.

use std::ops::Range;
use talkbank_model::model::{ChatFile, TranscriptName, WriteChat};
use talkbank_model::{ErrorCollector, ParseError, RuleSelection};
use talkbank_parser::{ParseProduct, TreeSitterParser};

/// A seed that failed admission, retaining the responsible stage's diagnostics.
#[derive(Debug, thiserror::Error)]
pub enum SeedRejection {
    /// Parsing required recovery or emitted another diagnostic.
    #[error("seed parsing was not diagnostic-free: {0:?}")]
    Parse(Vec<ParseError>),
    /// Contextual validation emitted at least one diagnostic.
    #[error("seed validation was not diagnostic-free: {0:?}")]
    Validation(Vec<ParseError>),
}

/// Immutable source and its privately owned, validated model.
pub struct AdmittedSeed<'source, 'name> {
    source: &'source str,
    file: ChatFile,
    name: TranscriptName<'name>,
    rules: RuleSelection,
}

impl<'source, 'name> AdmittedSeed<'source, 'name> {
    /// Parse once and validate with alignment and explicit file identity.
    pub fn admit(
        parser: &TreeSitterParser,
        source: &'source str,
        name: TranscriptName<'name>,
        rules: RuleSelection,
    ) -> Result<Self, SeedRejection> {
        let mut file = match parser.parse_chat_file(source) {
            ParseProduct::Built { file, diagnostics } => {
                if !diagnostics.is_empty() {
                    return Err(SeedRejection::Parse(diagnostics));
                }
                file
            }
            ParseProduct::Unbuildable { diagnostics } => {
                return Err(SeedRejection::Parse(diagnostics));
            }
        };
        let sink = ErrorCollector::new();
        file.validate_with_alignment_and_rules(rules, &sink, name);
        let diagnostics = sink.into_vec();
        if !diagnostics.is_empty() {
            return Err(SeedRejection::Validation(diagnostics));
        }
        Ok(Self {
            source,
            file,
            name,
            rules,
        })
    }

    /// The exact immutable source admitted at construction.
    pub fn source(&self) -> &'source str {
        self.source
    }

    /// Identity under which contextual rules ran.
    pub fn name(&self) -> TranscriptName<'name> {
        self.name
    }

    /// Rules used for admission, not an implicit universal-validity claim.
    pub fn rules(&self) -> RuleSelection {
        self.rules
    }

    /// Delete one complete main-tier terminator per candidate.
    ///
    /// Optional absent terminators produce no candidate. No text is re-parsed
    /// or reserialized: source ranges come exclusively from this seed's model.
    pub fn terminator_deletions(
        &self,
    ) -> impl Iterator<Item = Result<DeletionCandidate<'_>, SpanMismatch>> {
        self.file.utterances().filter_map(|utterance| {
            utterance
                .main
                .content
                .terminator
                .as_ref()
                .map(|terminator| {
                    let span = terminator.span();
                    let range = span.start as usize..span.end as usize;
                    let expected = terminator.to_chat_string();
                    if range.is_empty() || self.source.get(range.clone()) != Some(expected.as_str())
                    {
                        return Err(SpanMismatch { range });
                    }
                    Ok(DeletionCandidate {
                        source: self.source,
                        range,
                    })
                })
        })
    }
}

/// A producer span did not identify the complete modeled terminator.
#[derive(Debug, thiserror::Error)]
#[error("terminator span {range:?} does not match the admitted source")]
pub struct SpanMismatch {
    /// The rejected byte range, retained for diagnosis rather than repaired.
    pub range: Range<usize>,
}

/// An unreviewed deletion tied to the exact source that admitted it.
///
/// No public constructor accepts independently supplied source and ranges.
pub struct DeletionCandidate<'source> {
    source: &'source str,
    range: Range<usize>,
}

impl DeletionCandidate<'_> {
    /// Half-open byte range removed from the original seed.
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    /// Exact deleted bytes, including multi-character terminators.
    pub fn removed(&self) -> &str {
        &self.source[self.range.clone()]
    }

    /// Preserve every source byte outside the admitted deletion.
    pub fn render(&self) -> String {
        let mut output = String::with_capacity(self.source.len() - self.range.len());
        output.push_str(&self.source[..self.range.start]);
        output.push_str(&self.source[self.range.end..]);
        output
    }
}
