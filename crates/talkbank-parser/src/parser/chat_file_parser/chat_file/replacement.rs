//! Admission of retained input after producer-owned dependent-tier removal.
//!
//! This never certifies the original bytes. The generated traversal selects
//! concrete tiers, removes them before lowering, and retains every other fault.
use super::tier_plan::{PlanEntry, TierRemoval, TierRouting, WordTimingDecision};
use super::word_timing_plan::{WordTimingCandidate, WordTimingLine};
use crate::TreeSitterParser;
use crate::generated_traversal::{AsRawNode, ParsedSource, SourceBound, WorDependentTierNode};
use crate::parser::CstNodeId;
use talkbank_model::model::dependent_tier::{WorTier, WorTimingEvidence};
use talkbank_model::model::{Header, ParseHealthTier, TranscriptName};
use talkbank_model::validation::{
    AlignmentValidation, PendingTimingChatFile, TimingRegenerationAdmission, ValidChatFile,
    ValidationFailure, ValidationPolicy,
};
use talkbank_model::{
    ChatFile, CompletedDiagnostics, ErrorCollector, InternalFailure, ParseError, RuleSelection,
    Span,
};

/// Tiers this operation will remove, not merely forgive or preserve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplacementTiers {
    /// Remove morphology and grammatical relations together.
    Morphosyntax,
    /// Remove word-level timing; retain morphology and grammatical relations.
    WordTiming,
}

/// Word-timing disposition chosen from the producing document's headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordTimingPlan {
    /// Preserve every tier; all original content must pass complete admission.
    Preserve,
    /// Retain valid word timing, including partial timing. If complete original
    /// admission fails, discard only the concrete word tiers the evidence names
    /// (their own lowering, or a validation error inside their source span),
    /// each with the diagnostics that cost it, and admit all retained content,
    /// remaining word tiers included, before permitting regeneration. Errors
    /// located in no word tier discard every word tier and are recorded as
    /// unattributed; retained faults still refuse. Internal failures never
    /// permit regeneration.
    PreferRetained,
}

/// Where one concrete tier sits in the producing parse. Only the source-bound
/// producer can build one, from the tier's own CST node.
#[derive(Debug, Clone, Copy)]
pub(super) struct SourceTier {
    tier: ParseHealthTier,
    span: Span,
    syntax_recovery: bool,
    node_id: CstNodeId,
}

impl SourceTier {
    pub(super) fn from_word<'tree>(
        node: SourceBound<'tree, '_, WorDependentTierNode<'tree>>,
    ) -> Self {
        Self::of(ParseHealthTier::Wor, node.raw_node())
    }

    pub(super) fn of(tier: ParseHealthTier, node: tree_sitter::Node<'_>) -> Self {
        Self {
            tier,
            span: crate::parser::node_span::span_of(node),
            syntax_recovery: node.has_error(),
            node_id: CstNodeId::of(node),
        }
    }

    pub(super) fn span(&self) -> Span {
        self.span
    }

    pub(super) fn node_id(&self) -> CstNodeId {
        self.node_id
    }
}

/// Why one concrete tier was removed, with the evidence that cost it.
///
/// Only the adaptive word-timing plan removes a tier because of what it
/// contains; a fixed selection removes a whole domain by request.
#[derive(Debug)]
pub enum RemovalCause {
    /// The caller's fixed selection named this tier's domain. This says
    /// nothing about whether its content was valid.
    Selected,
    /// The tier's own lowering recovered or reported an error, so it never
    /// entered the candidate document. These are its own lowering diagnostics.
    OwnLowering {
        /// Diagnostics from lowering this tier alone.
        diagnostics: Vec<ParseError>,
    },
    /// Complete validation of the candidate rejected it with at least one
    /// error located inside this tier's source span. These are every
    /// diagnostic of that attempt located inside the tier.
    LocatedValidation {
        /// Diagnostics located inside this tier, at least one an error.
        diagnostics: Vec<ParseError>,
    },
    /// Complete validation rejected the candidate with errors located inside
    /// no retained word tier, so the errors could not be bound to one tier
    /// and every word tier still retained was removed together. Every tier
    /// removed in that step shares the same diagnostics. A retained fault
    /// among them still refuses admission afterwards.
    UnattributedValidation {
        /// The whole rejecting diagnostic set of that attempt.
        diagnostics: std::sync::Arc<[ParseError]>,
    },
}

impl RemovalCause {
    /// The diagnostics that cost the tier; empty for a fixed selection.
    pub fn diagnostics(&self) -> &[ParseError] {
        match self {
            Self::Selected => &[],
            Self::OwnLowering { diagnostics } | Self::LocatedValidation { diagnostics } => {
                diagnostics
            }
            Self::UnattributedValidation { diagnostics } => diagnostics,
        }
    }
}

/// Timing content of a word tier, read once from its own lowered entry. The
/// regeneration disposition is derived from removed tiers' values, never
/// from a flag accumulated beside them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TierTiming {
    /// The lowered tier carries at least one word bullet.
    Recorded,
    /// The tier was not lowered (a fixed selection, or its own lowering
    /// failed), or lowered without a word bullet.
    NotRecorded,
}

impl TierTiming {
    pub(super) fn of(tier: Option<&WorTier>) -> Self {
        match tier.map(WorTier::timing_evidence) {
            Some(WorTimingEvidence::Recorded(_)) => Self::Recorded,
            Some(WorTimingEvidence::Absent) | None => Self::NotRecorded,
        }
    }
}

/// One concrete tier removed by the source-bound producer, and why.
#[derive(Debug)]
pub struct RemovedTier {
    source: SourceTier,
    cause: RemovalCause,
    timing: TierTiming,
}

impl RemovedTier {
    /// A tier removed because the caller's fixed selection named its domain.
    /// It was never lowered, so it records no timing.
    pub(super) fn selected(source: SourceTier) -> Self {
        Self {
            source,
            cause: RemovalCause::Selected,
            timing: TierTiming::NotRecorded,
        }
    }
    /// A word tier refused by the adaptive plan, with the timing its own
    /// lowered entry carried.
    pub(super) fn word(source: SourceTier, cause: RemovalCause, timing: TierTiming) -> Self {
        Self {
            source,
            cause,
            timing,
        }
    }
    /// The discarded tier's alignment domain.
    pub fn tier(&self) -> ParseHealthTier {
        self.source.tier
    }
    /// Original source coordinates, not offsets into the retained rendering.
    pub fn span(&self) -> Span {
        self.source.span
    }
    /// Whether tree-sitter recovered inside this discarded concrete tier.
    /// This does not assert semantic validity when false.
    pub fn had_syntax_recovery(&self) -> bool {
        self.source.syntax_recovery
    }
    /// Why the tier was removed, with the diagnostics that cost it.
    pub fn cause(&self) -> &RemovalCause {
        &self.cause
    }
    pub(super) fn discarded_recorded_timing(&self) -> bool {
        self.timing == TierTiming::Recorded
    }
    /// Exact CST identity of the removed tier.
    pub(super) fn node_id(&self) -> CstNodeId {
        self.source.node_id
    }
}

/// Exact source and retained model admitted together. Fields cannot be forged.
///
/// Editing consumes the model's validity through `ValidChatFile::into_unchecked`.
/// No method marks the original source valid or restores its discarded tiers.
///
/// ```compile_fail,E0594
/// # fn edit(admitted: &mut talkbank_parser::AdmittedReplacement<'_>) {
/// admitted.document().lines = Vec::new().into();
/// # }
/// ```
#[derive(Debug)]
pub struct AdmittedReplacement<'source> {
    file: ValidChatFile,
    source: ParsedSource<'source>,
    removal: TierRemoval,
    parse_diagnostics: Vec<ParseError>,
}

/// The producer's actual disposition, retaining the appropriate admission.
#[derive(Debug)]
pub enum AdmittedDisposition<'source> {
    /// No replacement was selected and the complete original source was admitted.
    Preserved(AdmittedPreservation<'source>),
    /// Only the retained document is admitted; original bytes are not certified.
    Replaced(AdmittedReplacement<'source>),
    /// Source-bound removal left a timing requirement; not complete CHAT.
    Regenerating(AdmittedTimingRegeneration<'source>),
}

/// The adaptive plan's outcome. Only the complete branch contains validity.
#[derive(Debug)]
pub enum WordTimingAdmission<'source> {
    /// Complete retention or replacement admission.
    Complete(AdmittedReplacement<'source>),
    /// Concrete source timing was discarded and must now be regenerated.
    Regenerating(AdmittedTimingRegeneration<'source>),
}

/// Source and retained working model stay bound to the actual removed tiers.
/// Neither unchanged-byte admission nor complete validity is available here.
///
/// ```compile_fail,E0599
/// # fn certify(pending: talkbank_parser::AdmittedTimingRegeneration<'_>) {
/// let _ = pending.into_valid_file();
/// # }
/// ```
#[derive(Debug)]
pub struct AdmittedTimingRegeneration<'source> {
    file: PendingTimingChatFile,
    source: ParsedSource<'source>,
    removed: Vec<RemovedTier>,
    parse_diagnostics: Vec<ParseError>,
}

impl<'source> AdmittedTimingRegeneration<'source> {
    /// Working structure, not a complete-validity claim.
    pub fn document(&self) -> &ChatFile {
        self.file.document()
    }
    /// Exact producing source of the removed word timing.
    pub fn parsed_source(&self) -> &ParsedSource<'source> {
        &self.source
    }
    /// Concrete source tiers whose timing must be regenerated, each with the
    /// [`RemovalCause`] that cost it.
    pub fn removed_tiers(&self) -> &[RemovedTier] {
        &self.removed
    }
    /// Inspect the outstanding linked-media obligation.
    pub fn pending_file(&self) -> &PendingTimingChatFile {
        &self.file
    }
    /// Consume source ownership for a checked, explicitly incomplete working phase.
    pub fn into_pending_file(self) -> PendingTimingChatFile {
        self.file
    }
}

impl<'source> WordTimingAdmission<'source> {
    /// Inspect the retained structure; use disposition to learn its proof level.
    pub fn document(&self) -> &ChatFile {
        match self {
            Self::Complete(value) => value.document(),
            Self::Regenerating(value) => value.document(),
        }
    }
    /// Exact producing CST/source, including discarded content.
    pub fn parsed_source(&self) -> &ParsedSource<'source> {
        match self {
            Self::Complete(value) => value.parsed_source(),
            Self::Regenerating(value) => value.parsed_source(),
        }
    }
    /// Selection actually performed by the producer.
    pub fn selection(&self) -> Option<ReplacementTiers> {
        match self {
            Self::Complete(value) => value.selection(),
            Self::Regenerating(_) => Some(ReplacementTiers::WordTiming),
        }
    }
    /// Concrete source-bound removal receipts.
    pub fn removed_tiers(&self) -> &[RemovedTier] {
        match self {
            Self::Complete(value) => value.removed_tiers(),
            Self::Regenerating(value) => value.removed_tiers(),
        }
    }
    /// Parser warnings, never a filtered diagnostic population.
    pub fn parse_diagnostics(&self) -> &[ParseError] {
        match self {
            Self::Complete(value) => value.parse_diagnostics(),
            Self::Regenerating(value) => &value.parse_diagnostics,
        }
    }
    /// Consume the actual complete/pending disposition without reparsing.
    pub fn into_disposition(self) -> AdmittedDisposition<'source> {
        match self {
            Self::Complete(value) => value.into_disposition(),
            Self::Regenerating(value) => AdmittedDisposition::Regenerating(value),
        }
    }
}

/// Complete original-source admission from a producer that selected preservation.
/// A separately supplied model and source cannot construct this capability.
///
/// ```compile_fail,E0451
/// # fn forge(file: talkbank_model::validation::ValidChatFile, source: &str) {
/// let _ = talkbank_parser::AdmittedPreservation { file, source };
/// # }
/// ```
#[derive(Debug)]
pub struct AdmittedPreservation<'source> {
    file: ValidChatFile,
    source: &'source str,
}

impl<'source> AdmittedPreservation<'source> {
    /// Original bytes associated with the admitted producing parse.
    pub fn source(&self) -> &'source str {
        self.source
    }

    /// Immutable admission of the complete original document.
    pub fn document(&self) -> &ChatFile {
        self.file.document()
    }

    /// Consume the capability for storage in an unchanged-output proof.
    pub fn into_parts(self) -> (ValidChatFile, &'source str) {
        (self.file, self.source)
    }
}

impl<'source> AdmittedReplacement<'source> {
    /// Retained document accepted by normal rules and tier alignment checks.
    pub fn document(&self) -> &ChatFile {
        self.file.document()
    }
    /// Immutable validity evidence for the retained document only.
    pub fn valid_file(&self) -> &ValidChatFile {
        &self.file
    }
    /// Exact producing CST/source, including the discarded original content.
    pub fn parsed_source(&self) -> &ParsedSource<'source> {
        &self.source
    }
    /// Removal operation that actually produced this payload.
    pub fn selection(&self) -> Option<ReplacementTiers> {
        self.removal.selection()
    }
    /// Source-bound removal receipt; no entry means no matching tier existed.
    /// Each entry carries its [`RemovalCause`].
    pub fn removed_tiers(&self) -> &[RemovedTier] {
        self.removal.removed()
    }
    /// Retained parser warnings. Validation warnings live on `valid_file()`.
    pub fn parse_diagnostics(&self) -> &[ParseError] {
        &self.parse_diagnostics
    }
    /// Consume the source/removal association and keep retained-model validity.
    pub fn into_valid_file(self) -> ValidChatFile {
        self.file
    }

    /// Consume the plan and distinguish complete preservation from replacement.
    /// A selected replacement cannot certify original bytes, even when no tier
    /// happened to match it. Neither branch reparses or changes the document.
    pub fn into_disposition(self) -> AdmittedDisposition<'source> {
        match self.removal {
            TierRemoval::Nothing => AdmittedDisposition::Preserved(AdmittedPreservation {
                source: self.source.source(),
                file: self.file,
            }),
            TierRemoval::Selected { .. } => AdmittedDisposition::Replaced(self),
        }
    }
}

/// Admission refused without declaring an internal/tool failure CHAT invalid.
#[derive(Debug, thiserror::Error)]
pub enum ReplacementFailure {
    /// Tool failure is not a determination that CHAT input is invalid.
    #[error(transparent)]
    Internal(#[from] InternalFailure),
    /// Parsing could not retain a producing CST/source association.
    #[error("no producing source is available for replacement admission")]
    SourceUnavailable {
        /// Producer diagnostics, retained without asserting CHAT invalidity.
        diagnostics: Vec<ParseError>,
    },
    /// Retained parsing reported errors, including producer failures.
    #[error("retained input has parser errors")]
    RetainedParse {
        /// All retained parser diagnostics, including internal failures.
        diagnostics: Vec<ParseError>,
    },
    /// Normal retained-model validation or alignment admission failed.
    #[error(transparent)]
    Validation(#[from] ValidationFailure),
}

impl ReplacementFailure {
    /// All retained findings, with internal failure distinct from invalidity.
    pub fn diagnostics(&self) -> &[ParseError] {
        match self {
            Self::Internal(failure) => failure.diagnostics(),
            Self::Validation(failure) => failure.diagnostics(),
            Self::SourceUnavailable { diagnostics } | Self::RetainedParse { diagnostics } => {
                diagnostics
            }
        }
    }

    /// Whether tool failure prevented a validity determination.
    pub fn has_internal_failure(&self) -> bool {
        match self {
            Self::Internal(_) => true,
            Self::Validation(failure) => failure.has_internal_failure(),
            Self::SourceUnavailable { .. } | Self::RetainedParse { .. } => false,
        }
    }
}

/// Retain every word tier that survives complete validation; refuse only
/// the tiers the evidence names.
///
/// Tiers whose own lowering failed are removed first. The rest move into
/// the document, which is validated; on rejection
/// [`WordTimingCandidate::reduce`] removes the word tiers the errors lie
/// in, or refuses, and the reduced document is validated again. Retained
/// faults are never exempt. Removing a tier with recorded timing switches
/// validation to the regeneration phase, which may leave a pending timing
/// obligation.
fn admit_word_candidates<'source>(
    candidates: Vec<WordTimingLine>,
    lowered: LoweredAdmission<'source>,
    name: TranscriptName<'_>,
) -> Result<WordTimingAdmission<'source>, ReplacementFailure> {
    let LoweredAdmission {
        mut file,
        source,
        diagnostics,
    } = lowered;
    let mut candidate = WordTimingCandidate::assemble(&mut file, candidates)?;
    loop {
        let attempt = if candidate.discarded_recorded_timing() {
            file.validate_for_timing_regeneration(&ErrorCollector::new(), name)
        } else {
            file.validate_with_policy(complete_policy(), &ErrorCollector::new(), name)
                .map(TimingRegenerationAdmission::Ready)
        };
        match attempt {
            Ok(admitted) => return Ok(finish(candidate, admitted, source, diagnostics)),
            Err(failure) => file = candidate.reduce(failure)?,
        }
    }
}

/// The retained document, its producing source and its parser diagnostics,
/// after every parser-stage refusal has been checked and before validation.
struct LoweredAdmission<'source> {
    file: ChatFile,
    source: ParsedSource<'source>,
    diagnostics: Vec<ParseError>,
}

impl<'source> LoweredAdmission<'source> {
    /// Complete validation of the retained document, bound to its removal.
    fn admit_complete(
        self,
        removal: TierRemoval,
        name: TranscriptName<'_>,
    ) -> Result<AdmittedReplacement<'source>, ReplacementFailure> {
        let file =
            self.file
                .validate_with_policy(complete_policy(), &ErrorCollector::new(), name)?;
        Ok(AdmittedReplacement {
            file,
            source: self.source,
            removal,
            parse_diagnostics: self.diagnostics,
        })
    }
}

/// Complete admission: default rules and tier alignment checks.
fn complete_policy() -> ValidationPolicy {
    ValidationPolicy::new(
        RuleSelection::new(),
        AlignmentValidation::IncludeTierAlignment,
    )
}

/// Bind an admitted word-timing candidate to its source and receipts. Any
/// removal makes it a replacement; it never certifies the original bytes.
fn finish<'source>(
    candidate: WordTimingCandidate,
    admitted: TimingRegenerationAdmission,
    source: ParsedSource<'source>,
    mut parse_diagnostics: Vec<ParseError>,
) -> WordTimingAdmission<'source> {
    let (removed, retained_diagnostics) = candidate.into_parts();
    parse_diagnostics.extend(retained_diagnostics);
    match admitted {
        TimingRegenerationAdmission::Ready(file) => {
            WordTimingAdmission::Complete(AdmittedReplacement {
                file,
                source,
                removal: TierRemoval::adaptive(removed),
                parse_diagnostics,
            })
        }
        TimingRegenerationAdmission::Pending(file) => {
            WordTimingAdmission::Regenerating(AdmittedTimingRegeneration {
                file,
                source,
                removed,
                parse_diagnostics,
            })
        }
    }
}

impl TreeSitterParser {
    /// Parse once, remove only concrete selected dependent tiers, then validate
    /// all retained input with normal rules and tier alignment checks.
    ///
    /// Headers, main tiers, retained dependents and unclassified recovery remain
    /// subject to refusal. Global control-character checks are also retained.
    /// A pass-through or preservation workflow must instead admit the full file.
    pub fn admit_replacing_tiers<'source>(
        &self,
        input: &'source str,
        selection: ReplacementTiers,
        name: TranscriptName<'_>,
    ) -> Result<AdmittedReplacement<'source>, ReplacementFailure> {
        let (lowered, removal) = self.lower_for_admission(
            input,
            PlanEntry::Decided(TierRemoval::from_selection(Some(selection))),
        )?;
        lowered.admit_complete(removal, name)
    }

    /// Decide which tiers are actually removed from this document using its
    /// typed headers. `None` preserves and validates every dependent tier.
    ///
    /// The callback sees all headers, including misplaced ones; normal header
    /// validation still applies. It cannot modify the producing source or model.
    /// Headers and utterances each lower once from the same CST. This is suitable
    /// for header-dependent pass-through policies, not selective tier reuse.
    pub fn admit_planned_tiers<'source>(
        &self,
        input: &'source str,
        name: TranscriptName<'_>,
        select: impl FnOnce(&[&Header]) -> Option<ReplacementTiers>,
    ) -> Result<AdmittedReplacement<'source>, ReplacementFailure> {
        let (lowered, removal) = self.lower_for_admission(
            input,
            PlanEntry::AfterHeaders(Box::new(move |headers| {
                TierRemoval::from_selection(select(headers))
            })),
        )?;
        lowered.admit_complete(removal, name)
    }

    /// Select preservation or adaptive word-tier retention from the same parse.
    /// Concrete word candidates lower once, separately from retained content.
    /// Full original admission retains even partially reusable valid timing;
    /// otherwise only the word tiers the evidence names are removed, each with
    /// the diagnostics that cost it, and the reduced document is admitted.
    /// Discarding concrete recorded timing may leave a linked-media obligation;
    /// that returns a distinct incomplete regeneration state, never ValidChatFile.
    /// There is no second parse or diagnostic-code filtering.
    pub fn admit_word_timing_plan<'source>(
        &self,
        input: &'source str,
        name: TranscriptName<'_>,
        select: impl FnOnce(&[&Header]) -> WordTimingPlan,
    ) -> Result<WordTimingAdmission<'source>, ReplacementFailure> {
        let (lowered, decision) = self.lower_for_admission(
            input,
            PlanEntry::AfterHeaders(Box::new(move |headers| {
                WordTimingDecision::from(select(headers))
            })),
        )?;
        match decision {
            WordTimingDecision::Preserve => lowered
                .admit_complete(TierRemoval::Nothing, name)
                .map(WordTimingAdmission::Complete),
            WordTimingDecision::PreferRetained(candidates) => {
                admit_word_candidates(candidates, lowered, name)
            }
        }
    }

    /// Parse once and lower under `entry`, then apply every parser-stage
    /// refusal. The decided plan comes back beside the retained document.
    fn lower_for_admission<'source, P: TierRouting>(
        &self,
        input: &'source str,
        entry: PlanEntry<'_, P>,
    ) -> Result<(LoweredAdmission<'source>, P), ReplacementFailure> {
        let errors = ErrorCollector::new();
        talkbank_model::validation::report_control_characters(input, &errors);
        let (file, lowered) = self.parse_chat_file_bound_with_removal(input, None, &errors, entry);
        let has_errors = errors.has_errors();
        let diagnostics = CompletedDiagnostics::admit(errors.into_vec())?.into_diagnostics();
        let Some((source, mut plan)) = lowered else {
            return Err(ReplacementFailure::SourceUnavailable { diagnostics });
        };
        plan.admit_deferred_diagnostics()?;
        if has_errors {
            return Err(ReplacementFailure::RetainedParse { diagnostics });
        }
        Ok((
            LoweredAdmission {
                file,
                source,
                diagnostics,
            },
            plan,
        ))
    }
}
