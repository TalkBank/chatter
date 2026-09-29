//! All-or-refused source application followed by independent output admission.

use std::{fmt, ops::Range};
use talkbank_model::validation::ValidChatFile;
use talkbank_parser::TreeSitterParser;

use super::{
    FreeTextDecision, HeaderNameField, InputRefusal, LexicalPlan, MorphologyOutcome,
    PseudonymizationInput, TimingOutcome,
};

/// Typed origin of an applied private receipt entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKind {
    /// Main-tier lexical field, including a shortening removed by replacement.
    MainWord,
    /// Aligned main morphology lemma, not a post-clitic.
    Morphology,
    /// Corroborated timing-tier lexical field, not its bullet.
    Timing,
    /// Generated participant or ID name field.
    Header(HeaderNameField),
    /// Whole Unicode word in a typed prose field.
    Prose,
}

/// Decision owner of an applied edit. Word-bearing variants always retain the
/// main-tier location that authorized the change, never a guessed tier index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditOrigin {
    /// Main-tier lexical decision.
    MainWord(super::WordLocation),
    /// Morphology change driven by this main-tier decision.
    Morphology(super::WordLocation),
    /// Timing change driven by this main-tier decision.
    Timing(super::WordLocation),
    /// Generated metadata field.
    Header(super::HeaderFieldLocation),
    /// Typed prose field and its original owner position.
    Prose(super::ProseLocation),
}

impl EditOrigin {
    /// Derive the category from its owner, avoiding separately stored labels.
    pub fn kind(self) -> EditKind {
        match self {
            Self::MainWord(_) => EditKind::MainWord,
            Self::Morphology(_) => EditKind::Morphology,
            Self::Timing(_) => EditKind::Timing,
            Self::Header(location) => EditKind::Header(location.field()),
            Self::Prose(_) => EditKind::Prose,
        }
    }
}

/// Sensitive receipt entry built by the exact application that produced output.
/// No implicit debug or serialization exposes names.
pub struct AppliedEdit {
    origin: EditOrigin,
    original_range: Range<usize>,
    output_range: Range<usize>,
    original: String,
    replacement: String,
}

impl AppliedEdit {
    /// Typed source category.
    pub fn kind(&self) -> EditKind {
        self.origin.kind()
    }
    /// Decision origin carried through the actual source application.
    pub fn origin(&self) -> EditOrigin {
        self.origin
    }
    /// Coordinates in the original protected input.
    pub fn original_range(&self) -> Range<usize> {
        self.original_range.clone()
    }
    /// Coordinates in the admitted output, empty for a removed lexical piece.
    pub fn output_range(&self) -> Range<usize> {
        self.output_range.clone()
    }
    /// Protected original spelling, only for explicitly private receipts.
    pub fn original(&self) -> &str {
        &self.original
    }
    /// Actual replacement written at the corresponding output range.
    pub fn replacement(&self) -> &str {
        &self.replacement
    }
}

/// A value-free reason why no output was issued.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputRefusalReason {
    /// A lexical, morphology, timing or pronunciation review blocks output.
    UnsafePlan,
    /// A metadata or prose owner could not bind to its source.
    SourceBinding,
    /// Proposed source ranges overlap, are invalid or disagree with their source.
    EditCorrespondence,
    /// Rewritten CHAT failed the original input admission policy.
    Admission(InputRefusal),
    /// A second plan would change the output again or refuse its safety.
    NotIdempotent,
}

/// Refusal retains the review findings but has no proposed output text.
pub struct OutputRefusal<'input> {
    reason: OutputRefusalReason,
    plan: Box<LexicalPlan<'input>>,
}

impl OutputRefusal<'_> {
    /// Value-free failure classification.
    pub fn reason(&self) -> OutputRefusalReason {
        self.reason
    }
    /// Sensitive input findings for an explicitly private review.
    pub fn plan(&self) -> &LexicalPlan<'_> {
        &self.plan
    }
}

/// In-memory output admitted under the input's exact validation context.
///
/// Construction proves source-edit correspondence, validation and that another
/// plan produces no changes. It does not certify complete de-identification.
/// Case near misses, possessives and unmatched lemmas remain in the review.
/// No automatic file writer or public receipt serialization is provided.
pub struct PseudonymizedDocument<'input> {
    text: String,
    document: ValidChatFile,
    edits: Vec<AppliedEdit>,
    plan: LexicalPlan<'input>,
}

impl PseudonymizedDocument<'_> {
    /// Admitted output, still potentially identifying and not for default logging.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Independent output validity evidence, not the input model.
    pub fn document(&self) -> &ValidChatFile {
        &self.document
    }
    /// Exact applied changes, in source order.
    pub fn edits(&self) -> &[AppliedEdit] {
        &self.edits
    }
    /// Original sensitive review, including report-only findings.
    pub fn plan(&self) -> &LexicalPlan<'_> {
        &self.plan
    }
}

impl PseudonymizationInput<'_, '_> {
    /// Apply reviewed source fields atomically in memory, then admit the result.
    /// Refusals expose only original review evidence, never partial output.
    pub fn prepare_output(
        &self,
        parser: &TreeSitterParser,
    ) -> Result<PseudonymizedDocument<'_>, OutputRefusal<'_>> {
        let plan = self.plan_words();
        let prepared = (|| {
            let edits = collect_edits(&plan)?;
            let (text, edits) = apply(self.source(), edits)?;
            let output = self
                .names()
                .admit_document(
                    &text,
                    self.document().name(),
                    self.document().policy().rules(),
                    parser,
                )
                .map_err(OutputRefusalReason::Admission)?;
            // Reuse this output parse for the stability check; do not parse it
            // again or manufacture validity from an unchanged input proof.
            let follow_up = output.plan_words();
            let remaining =
                collect_edits(&follow_up).map_err(|_| OutputRefusalReason::NotIdempotent)?;
            if !remaining.is_empty() {
                return Err(OutputRefusalReason::NotIdempotent);
            }
            drop(remaining);
            drop(follow_up);
            let document = output.into_document();
            Ok((text, document, edits))
        })();
        match prepared {
            Ok((text, document, edits)) => Ok(PseudonymizedDocument {
                text,
                document,
                edits,
                plan,
            }),
            Err(reason) => Err(OutputRefusal {
                reason,
                plan: Box::new(plan),
            }),
        }
    }
}

struct Edit<'plan> {
    origin: EditOrigin,
    range: Range<usize>,
    original: &'plan str,
    replacement: &'plan str,
}

fn collect_edits<'plan>(
    plan: &'plan LexicalPlan<'_>,
) -> Result<Vec<Edit<'plan>>, OutputRefusalReason> {
    if !plan.refusals().is_empty() || !plan.pronunciation().is_empty() {
        return Err(OutputRefusalReason::UnsafePlan);
    }
    let mut edits = Vec::new();
    for word in plan.previews() {
        for edit in word.edits() {
            edits.push(Edit {
                origin: EditOrigin::MainWord(word.location()),
                range: edit.range(),
                original: edit.original(),
                replacement: edit.replacement(),
            });
        }
    }
    for review in plan.morphology() {
        match review.outcome() {
            MorphologyOutcome::Proposed {
                source, proposed, ..
            } => edits.push(Edit {
                origin: EditOrigin::Morphology(review.location()),
                range: source.range(),
                original: source.original(),
                replacement: proposed.main.lemma.as_ref(),
            }),
            MorphologyOutcome::Refused { .. } => return Err(OutputRefusalReason::UnsafePlan),
        }
    }
    for review in plan.timing() {
        match review.outcome() {
            TimingOutcome::Corroborated(words) => {
                for word in words {
                    for edit in word.edits() {
                        edits.push(Edit {
                            origin: EditOrigin::Timing(word.location()),
                            range: edit.range(),
                            original: edit.original(),
                            replacement: edit.replacement(),
                        });
                    }
                }
            }
            TimingOutcome::Refused(_) => return Err(OutputRefusalReason::UnsafePlan),
        }
    }
    for field in plan
        .header_fields()
        .map_err(|_| OutputRefusalReason::SourceBinding)?
    {
        if let FreeTextDecision::Replace(text) = field.decision() {
            edits.push(Edit {
                origin: EditOrigin::Header(field.location()),
                range: field.range(),
                original: field.original(),
                replacement: text.as_ref(),
            });
        }
    }
    for field in plan
        .free_text()
        .map_err(|_| OutputRefusalReason::SourceBinding)?
    {
        if let FreeTextDecision::Replace(text) = field.decision() {
            edits.push(Edit {
                origin: EditOrigin::Prose(field.location()),
                range: field.range(),
                original: field.original(),
                replacement: text.as_ref(),
            });
        }
    }
    edits.sort_by_key(|edit| edit.range.start);
    Ok(edits)
}

fn apply(
    source: &str,
    edits: Vec<Edit<'_>>,
) -> Result<(String, Vec<AppliedEdit>), OutputRefusalReason> {
    let mut text = String::with_capacity(source.len());
    let mut applied = Vec::with_capacity(edits.len());
    let mut cursor = 0;
    for edit in edits {
        let gap = source
            .get(cursor..edit.range.start)
            .ok_or(OutputRefusalReason::EditCorrespondence)?;
        if source.get(edit.range.clone()) != Some(edit.original) {
            return Err(OutputRefusalReason::EditCorrespondence);
        }
        text.push_str(gap);
        let start = text.len();
        text.push_str(edit.replacement);
        cursor = edit.range.end;
        applied.push(AppliedEdit {
            origin: edit.origin,
            original_range: edit.range,
            output_range: start..text.len(),
            original: edit.original.to_owned(),
            replacement: edit.replacement.to_owned(),
        });
    }
    text.push_str(
        source
            .get(cursor..)
            .ok_or(OutputRefusalReason::EditCorrespondence)?,
    );
    Ok((text, applied))
}

impl fmt::Debug for PseudonymizedDocument<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PseudonymizedDocument(<private>)")
    }
}
impl fmt::Debug for OutputRefusal<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OutputRefusal")
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}
