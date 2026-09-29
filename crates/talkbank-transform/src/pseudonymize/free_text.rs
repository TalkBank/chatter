//! Unicode matching inside typed prose payloads, never raw CHAT structure.

use talkbank_model::{DependentTier, Header, Span, WordText};
use talkbank_parser::generated_traversal::{
    AsRawNode, RestOfLineNode, SourceSlice, TextSegmentNode,
};
use unicode_segmentation::UnicodeSegmentation;

use super::{NameDecision, PseudonymizationInput, TranscriptNames};

/// The typed owner of a private prose finding.
pub enum FreeTextOwner<'input> {
    /// A name-bearing prose header, not a speaker code or configuration header.
    Header(&'input Header),
    /// A prose dependent tier, not morphology, timing or pronunciation.
    DependentTier(&'input DependentTier),
}

/// Typed prose position. Indices are zero-based in the owning AST collections;
/// a dependent tier has an utterance, while a document header does not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProseLocation {
    /// Original header, including structural headers in the index count.
    Header(usize),
    /// Original dependent tier within an utterance (all tier kinds count).
    DependentTier {
        /// Zero-based utterance in document order.
        utterance: usize,
        /// Zero-based dependent tier in this utterance, including non-prose tiers.
        tier: usize,
    },
}

/// Review decisions never turn a possessive or case near miss into a rewrite.
pub enum FreeTextDecision<'input> {
    /// Exact whole Unicode word match.
    Replace(&'input WordText),
    /// Case-only match, retained for private review.
    CaseNearMiss,
    /// A mapped name followed by an apostrophe-s possessive, left intact.
    PossessiveReview,
}

/// Sensitive whole-word finding tied to a producer-owned prose field.
pub struct FreeTextFinding<'input> {
    location: ProseLocation,
    owner: FreeTextOwner<'input>,
    matched: TextMatch<'input>,
}

/// One matcher-owned decision and range, shared by prose and metadata owners.
pub(super) struct TextMatch<'input> {
    field: SourceSlice<'input, 'input>,
    offset: usize,
    text: &'input str,
    decision: FreeTextDecision<'input>,
}

impl FreeTextFinding<'_> {
    /// Position recorded by the typed owner traversal.
    pub fn location(&self) -> ProseLocation {
        self.location
    }
    /// Typed context for a private receipt.
    pub fn owner(&self) -> &FreeTextOwner<'_> {
        &self.owner
    }
    /// Exact protected spelling, including a possessive suffix when reported.
    pub fn original(&self) -> &str {
        self.matched.original()
    }
    /// Exact UTF-8 source range supplied by the field and Unicode segmentation.
    pub fn range(&self) -> std::ops::Range<usize> {
        self.matched.range()
    }
    /// Sensitive replacement or value-free report-only classification.
    pub fn decision(&self) -> &FreeTextDecision<'_> {
        self.matched.decision()
    }
}

impl TextMatch<'_> {
    pub(super) fn original(&self) -> &str {
        self.text
    }
    pub(super) fn range(&self) -> std::ops::Range<usize> {
        let start = self.field.raw_node().start_byte() + self.offset;
        start..start + self.text.len()
    }
    pub(super) fn decision(&self) -> &FreeTextDecision<'_> {
        &self.decision
    }
}

/// A typed prose owner could not be associated with its producing source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FreeTextRefusal;

pub(super) fn plan<'input>(
    input: &'input PseudonymizationInput<'_, '_>,
) -> Result<Vec<FreeTextFinding<'input>>, FreeTextRefusal> {
    let mut findings = Vec::new();
    for (index, (header, span)) in input.document().document().headers_with_spans().enumerate() {
        if header_is_prose(header) {
            inspect(
                input,
                span,
                ProseLocation::Header(index),
                || FreeTextOwner::Header(header),
                &mut findings,
            )?;
        }
    }
    for (utterance_index, utterance) in input.document().document().utterances().enumerate() {
        for (tier_index, entry) in utterance.dependent_tiers.iter().enumerate() {
            if tier_is_prose(&entry.tier) {
                inspect(
                    input,
                    entry.span(),
                    ProseLocation::DependentTier {
                        utterance: utterance_index,
                        tier: tier_index,
                    },
                    || FreeTextOwner::DependentTier(&entry.tier),
                    &mut findings,
                )?;
            }
        }
    }
    findings.sort_by_key(|finding| finding.range().start);
    Ok(findings)
}

fn inspect<'input>(
    input: &'input PseudonymizationInput<'_, '_>,
    span: Span,
    location: ProseLocation,
    owner: impl Fn() -> FreeTextOwner<'input>,
    findings: &mut Vec<FreeTextFinding<'input>>,
) -> Result<(), FreeTextRefusal> {
    let parsed = input.parsed_source();
    let node = parsed
        .root_node()
        .named_descendant_for_byte_range(span.start as usize, span.end as usize)
        .ok_or(FreeTextRefusal)?;
    let root = parsed.bind(node).map_err(|_| FreeTextRefusal)?;
    for descendant in root.descendants() {
        let field = descendant.map_err(|_| FreeTextRefusal)?;
        if field.typed::<RestOfLineNode>().is_none() && field.typed::<TextSegmentNode>().is_none() {
            continue;
        }
        for matched in matches(field, input.names())? {
            findings.push(FreeTextFinding {
                location,
                owner: owner(),
                matched,
            });
        }
    }
    Ok(())
}

/// The only whole-word matcher for both prose and generated metadata fields.
/// Its private construction ties each decision to its original field and token.
pub(super) fn matches<'input>(
    field: SourceSlice<'input, 'input>,
    names: &'input TranscriptNames,
) -> Result<impl Iterator<Item = TextMatch<'input>>, FreeTextRefusal> {
    Ok(matches_text(field.text(), names)?
        .into_iter()
        .map(move |(offset, text, decision)| TextMatch {
            field,
            offset,
            text,
            decision,
        }))
}

/// Match complete literal names between Unicode boundaries, including names
/// spanning several segments. Longest mapped spelling wins at each boundary;
/// a longer near miss or possessive cannot become a shorter partial rewrite.
/// Used both for source findings and placeholder admission, with one policy.
pub(super) fn matches_text<'text, 'map>(
    input: &'text str,
    names: &'map TranscriptNames,
) -> Result<Vec<(usize, &'text str, FreeTextDecision<'map>)>, FreeTextRefusal> {
    let max_parts = names
        .patterns()
        .map(|name| name.split_word_bounds().count())
        .max()
        .ok_or(FreeTextRefusal)?;
    let parts: Vec<_> = input.split_word_bound_indices().collect();
    let mut findings = Vec::new();
    let mut start = 0;
    while let Some(&(offset, _)) = parts.get(start) {
        let mut next = start + 1;
        // The extra boundary pieces allow the report-only possessive suffix.
        let limit = parts
            .len()
            .min(start.saturating_add(max_parts).saturating_add(2));
        for end in (start..limit).rev() {
            let (last_offset, last) = parts.get(end).ok_or(FreeTextRefusal)?;
            let text = input
                .get(offset..last_offset + last.len())
                .ok_or(FreeTextRefusal)?;
            let possessive = ["'s", "’s", "'S", "’S"]
                .into_iter()
                .find_map(|suffix| text.strip_suffix(suffix))
                .is_some_and(|base| !matches!(names.decide(base), NameDecision::Keep));
            let decision = if possessive {
                FreeTextDecision::PossessiveReview
            } else {
                match names.decide(text) {
                    NameDecision::Replace(replacement) => FreeTextDecision::Replace(replacement),
                    NameDecision::CaseNearMiss => FreeTextDecision::CaseNearMiss,
                    NameDecision::Keep => continue,
                }
            };
            findings.push((offset, text, decision));
            next = end + 1;
            break;
        }
        start = next;
    }
    Ok(findings)
}

fn header_is_prose(header: &Header) -> bool {
    match header {
        Header::Comment { .. }
        | Header::Situation { .. }
        | Header::TapeLocation { .. }
        | Header::Location { .. }
        | Header::RoomLayout { .. }
        | Header::Birthplace { .. }
        | Header::Transcriber { .. }
        | Header::Warning { .. }
        | Header::Activities { .. }
        | Header::Bck { .. }
        | Header::BeginGem { .. }
        | Header::EndGem { .. }
        | Header::LazyGem { .. } => true,
        Header::Utf8
        | Header::Begin
        | Header::End
        | Header::Languages { .. }
        | Header::Participants { .. }
        | Header::ID(_)
        | Header::Date { .. }
        | Header::Pid { .. }
        | Header::Media(_)
        | Header::Types(_)
        | Header::Font { .. }
        | Header::Window { .. }
        | Header::ColorWords { .. }
        | Header::Number { .. }
        | Header::RecordingQuality { .. }
        | Header::Transcription { .. }
        | Header::NewEpisode
        | Header::TimeDuration { .. }
        | Header::TimeStart { .. }
        | Header::Birth { .. }
        | Header::L1Of { .. }
        | Header::Blank
        | Header::Unknown { .. }
        | Header::Options { .. }
        | Header::Page { .. }
        | Header::Videos { .. }
        | Header::T { .. } => false,
    }
}

fn tier_is_prose(tier: &DependentTier) -> bool {
    match tier {
        DependentTier::Act(_)
        | DependentTier::Cod(_)
        | DependentTier::Add(_)
        | DependentTier::Com(_)
        | DependentTier::Exp(_)
        | DependentTier::Gpx(_)
        | DependentTier::Int(_)
        | DependentTier::Sit(_)
        | DependentTier::Spa(_)
        | DependentTier::Alt(_)
        | DependentTier::Coh(_)
        | DependentTier::Def(_)
        | DependentTier::Eng(_)
        | DependentTier::Err(_)
        | DependentTier::Fac(_)
        | DependentTier::Flo(_)
        | DependentTier::Gls(_)
        | DependentTier::Ort(_)
        | DependentTier::Par(_)
        | DependentTier::UserDefined(_) => true,
        DependentTier::Mor(_)
        | DependentTier::Gra(_)
        | DependentTier::Pho(_)
        | DependentTier::Mod(_)
        | DependentTier::Sin(_)
        | DependentTier::Modsyl(_)
        | DependentTier::Phosyl(_)
        | DependentTier::Phoaln(_)
        | DependentTier::Xphoint(_)
        | DependentTier::Tim(_)
        | DependentTier::Wor(_)
        | DependentTier::Unsupported(_) => false,
    }
}
