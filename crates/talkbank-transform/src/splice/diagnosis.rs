//! One actual diagnosis owns every input used by the repair application path.

use super::catalog::{CatalogResolution, catalog_fix};
use super::{FixKind, SpliceEdit};
use std::collections::HashSet;
use talkbank_model::model::TranscriptName;
use talkbank_model::validation::ValidationPolicy;
use talkbank_model::{ChatFile, ErrorCode, ErrorCollector, ParseError};
use talkbank_parser::{TreeSitterParser, generated_traversal::ParsedSource};

mod phases;
pub use phases::{AdmittedRepairs, SelectedRepairs, VerifiedRepairs};

enum DiagnosisSource<'source> {
    Parsed(ParsedSource<'source>),
    Unavailable(&'source str),
}

impl<'source> DiagnosisSource<'source> {
    fn source(&self) -> &'source str {
        match self {
            Self::Parsed(parsed) => parsed.source(),
            Self::Unavailable(source) => source,
        }
    }
    fn parsed(&self) -> Option<&ParsedSource<'source>> {
        match self {
            Self::Parsed(parsed) => Some(parsed),
            Self::Unavailable(_) => None,
        }
    }
    fn resolve(&self, diagnostic: &ParseError) -> CatalogResolution {
        match self.parsed() {
            Some(parsed) => catalog_fix(diagnostic, parsed),
            None => CatalogResolution::SourceUnavailable,
        }
    }
}

/// A produced model and complete findings associated with their actual source.
/// Diagnosis is not validity; no errors, warnings or recovery are discarded.
///
/// ```compile_fail
/// use talkbank_transform::splice::FixDiagnosis;
/// let forged = FixDiagnosis { diagnostics: Vec::new() };
/// ```
///
/// An independent diagnostic/CST resolver is no longer public.
/// ```compile_fail
/// use talkbank_transform::splice::catalog_fix;
/// ```
pub struct FixDiagnosis<'source> {
    source: DiagnosisSource<'source>,
    document: ChatFile,
    diagnostics: Vec<ParseError>,
    policy: ValidationPolicy,
    name: TranscriptName<'source>,
}

impl std::fmt::Debug for FixDiagnosis<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FixDiagnosis")
            .field("source_bytes", &self.source().len())
            .field(
                "source_available",
                &matches!(self.source, DiagnosisSource::Parsed(_)),
            )
            .field("diagnostic_count", &self.diagnostics.len())
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

impl<'source> FixDiagnosis<'source> {
    /// Parse once, validate that produced model and retain the complete context.
    pub fn observe(
        parser: &TreeSitterParser,
        source: &'source str,
        policy: ValidationPolicy,
        name: TranscriptName<'source>,
    ) -> Self {
        let sink = ErrorCollector::new();
        let (mut document, parsed) = parser.parse_chat_file_with_source(source, &sink);
        document.validate_at(policy, &sink, name);
        Self {
            source: match parsed {
                Some(parsed) => DiagnosisSource::Parsed(parsed),
                None => DiagnosisSource::Unavailable(source),
            },
            document,
            diagnostics: sink.into_vec(),
            policy,
            name,
        }
    }

    /// The exact original input; the source reference survives moving this owner.
    pub fn source(&self) -> &'source str {
        self.source.source()
    }
    /// Read-only produced model; cloning it does not issue diagnosis authority.
    pub fn document(&self) -> &ChatFile {
        &self.document
    }
    /// Optional readonly CST from the actual producer; absence is not fabricated.
    pub fn parsed_source(&self) -> Option<&ParsedSource<'source>> {
        self.source.parsed()
    }
    /// Complete immutable findings, including unselected codes and warnings.
    pub fn diagnostics(&self) -> &[ParseError] {
        &self.diagnostics
    }
    /// The policy actually used before selection; reused by output verification.
    pub fn policy(&self) -> ValidationPolicy {
        self.policy
    }
    /// The original validation name, not inferred from rewritten content.
    pub fn name(&self) -> TranscriptName<'source> {
        self.name
    }
    /// Issue associated finding views; external diagnostics cannot enter here.
    pub fn findings(&self) -> impl Iterator<Item = FixFinding<'_, 'source>> {
        self.diagnostics.iter().map(|diagnostic| FixFinding {
            diagnosis: self,
            diagnostic,
        })
    }

    /// Select only this owner's findings, consuming the diagnosis into its phases.
    /// No public append accepts tokens from a different diagnosis.
    pub fn select(self, selection: &FixCodeSelection) -> SelectedRepairs<'source> {
        let mut candidates = Vec::new();
        let mut skipped = Vec::new();
        for finding in self
            .findings()
            .filter(|finding| selection.admits(finding.code))
        {
            let code = finding.code;
            let reason = match finding.catalog() {
                CatalogResolution::Offered(fix) => match fix.into_kind() {
                    FixKind::Mechanical(edits) => {
                        candidates.extend(edits);
                        continue;
                    }
                    FixKind::Semantic(edits) if selection.names(code) => {
                        candidates.extend(edits);
                        continue;
                    }
                    FixKind::Semantic(_) => SelectionSkip::NeedsCode,
                    FixKind::Alternatives(choices) => SelectionSkip::Alternatives(
                        choices.into_iter().map(|choice| choice.label).collect(),
                    ),
                },
                CatalogResolution::Unsupported => SelectionSkip::Unsupported,
                CatalogResolution::Ineligible(reason) => SelectionSkip::Ineligible(reason),
                CatalogResolution::SourceUnavailable => SelectionSkip::SourceUnavailable,
                CatalogResolution::Fault(fault) => SelectionSkip::Fault(fault),
            };
            skipped.push(SelectionSkipped { code, reason });
        }
        SelectedRepairs {
            diagnosis: self,
            candidates,
            skipped,
        }
    }
}

/// A read-only view issued only by its diagnosis's iteration.
/// A shared lifetime alone is not identity; selection never accepts outside views.
///
/// ```compile_fail
/// use talkbank_transform::splice::FixFinding;
/// let forged = FixFinding { diagnostic: todo!(), diagnosis: todo!() };
/// ```
#[derive(Clone, Copy)]
pub struct FixFinding<'diagnosis, 'source> {
    diagnosis: &'diagnosis FixDiagnosis<'source>,
    diagnostic: &'diagnosis ParseError,
}

impl FixFinding<'_, '_> {
    /// Inspect the original immutable diagnostic.
    pub fn diagnostic(&self) -> &ParseError {
        self.diagnostic
    }
    /// Resolve against the producing diagnosis's source, never another CST.
    pub fn catalog(&self) -> CatalogResolution {
        self.diagnosis.source.resolve(self.diagnostic)
    }
}
impl std::ops::Deref for FixFinding<'_, '_> {
    type Target = ParseError;
    fn deref(&self) -> &Self::Target {
        self.diagnostic
    }
}

/// Explicit current code-selection policy, distinct from naming every code.
#[derive(Debug)]
pub enum FixCodeSelection {
    /// Consider all findings but name no semantic code.
    All,
    /// Consider and explicitly name only this set of real codes.
    Only(HashSet<ErrorCode>),
}
impl FixCodeSelection {
    fn admits(&self, code: ErrorCode) -> bool {
        match self {
            Self::All => true,
            Self::Only(codes) => codes.contains(&code),
        }
    }
    fn names(&self, code: ErrorCode) -> bool {
        matches!(self, Self::Only(codes) if codes.contains(&code))
    }
}

/// One actual finding refused during owner-local selection.
#[derive(Debug)]
pub struct SelectionSkipped {
    /// The original finding's code.
    pub code: ErrorCode,
    /// Why it was not selected.
    pub reason: SelectionSkip,
}

/// Full selection verdict retained separately from health-admission refusals.
#[derive(Debug)]
pub enum SelectionSkip {
    /// No existing catalog repair policy.
    Unsupported,
    /// Existing policy is unsupported at this actual carrier.
    Ineligible(super::catalog::CatalogIneligibility),
    /// The parse did not retain a CST.
    SourceUnavailable,
    /// Generated source evidence failed.
    Fault(super::catalog::CatalogFault),
    /// A semantic code was not explicitly named.
    NeedsCode,
    /// Several valid choices are retained but never selected.
    Alternatives(Vec<String>),
}

impl std::fmt::Display for SelectionSkip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => f.write_str("no catalog entry"),
            Self::Ineligible(reason) => write!(f, "catalog proposal declined: {reason}"),
            Self::SourceUnavailable => f.write_str("source-bound parse unavailable"),
            Self::Fault(fault) => write!(f, "source capability refused: {fault}"),
            Self::NeedsCode => f.write_str("semantic fix, name it with --code to apply"),
            Self::Alternatives(labels) => {
                write!(f, "ambiguous, several valid fixes ({})", labels.join("; "))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CatalogResolution, DiagnosisSource, FixDiagnosis};

    #[test]
    fn unavailable_source_retains_bytes_without_issuing_a_cst() {
        // Native absence-state boundary only: no fabricated diagnostic/model,
        // and no claim of witnessing the parser's external failure path.
        let source = include_str!(
            "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E241_illegal_untranscribed_marker_2.cha"
        );
        let parser = talkbank_parser::TreeSitterParser::new().expect("parser");
        let diagnosis = FixDiagnosis::observe(
            &parser,
            source,
            talkbank_model::validation::ValidationPolicy::new(
                talkbank_model::RuleSelection::new(),
                talkbank_model::validation::AlignmentValidation::Structure,
            ),
            talkbank_model::model::TranscriptName::Anonymous,
        );
        let finding = diagnosis
            .findings()
            .find(|finding| finding.code.as_str() == "E241")
            .expect("actual finding");
        let unavailable = DiagnosisSource::Unavailable(source);
        assert_eq!(unavailable.source(), source);
        assert!(unavailable.parsed().is_none());
        assert!(matches!(
            unavailable.resolve(finding.diagnostic()),
            CatalogResolution::SourceUnavailable
        ));
    }
}
