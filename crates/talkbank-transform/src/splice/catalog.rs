//! The fix catalog: one answer to "what fix does error code X get".
//!
//! Before this module, "what fix does error code X get" had two unshared
//! answers: the LSP's `code_action_fixes.rs` (~21 codes, a human accepts
//! each action individually in an editor) and `chatter lint`'s own
//! three-code copy (deleted 2026-07-31, a live unguarded byte writer). This
//! module supplies the answer exposed by [`crate::splice::FixFinding::catalog`].
//! The producing diagnosis owns the source and finding; callers cannot resolve
//! an arbitrary diagnostic against an unrelated parse. [`BatchSafety`] records
//! the additional judgment needed before a proposal can be batch-selected.
//!
//! # Why a tier, not just a fix
//!
//! The LSP catalog can afford to be liberal because a human reviews and
//! accepts each action individually. A batch CLI applying the same catalog
//! over a corpus turns twenty reviewed decisions into twenty unreviewed
//! ones. A rewrite can remain structurally valid while changing meaning, so
//! revalidation alone cannot establish preservation. [`BatchSafety`] makes
//! that risk explicit for each proposal:
//!
//! - [`BatchSafety::Mechanical`]: one right answer, no semantic judgment.
//!   Safe for a bare `--apply` to write unattended.
//! - [`BatchSafety::Semantic`]: deterministic, but consequential enough
//!   (deletes content or changes what a speaker
//!   said) that a batch run should require the caller to name the code.
//! - [`BatchSafety::Ambiguous`]: several valid answers exist and no
//!   evidence in the file picks one. Never batch-applied; only reported.
//!
//! Missing participant, role and language facts have no catalog proposal:
//! E308/E504/E507 require actual user-supplied values, not placeholders.
//!
//! # Porting from the LSP catalog: verified, not copied
//!
//! The observations below record the original porting rationale, not today's
//! diagnostic reachability. The current E301/E501/E507 specs assign their
//! malformed-header/speaker specimens to E316 grammar rejection; the catalog
//! refuses those specimens rather than inventing the historical diagnostics.
//!
//! The catalog exposed through [`crate::splice::FixFinding::catalog`] is not a mechanical port of
//! `crates/talkbank-lsp/src/backend/features/code_action_fixes.rs`. Every
//! ported code was checked against what the CURRENT `ErrorCode` variant and
//! a real `chatter validate` run actually produce (`chatter validate` is
//! this repo's authority on CHAT validity; see the root `AGENTS.md`
//! "CHAT-validity authority" section). Several of the LSP's ~21 entries
//! turned out to be stale, most likely surviving an `ErrorCode` renumbering
//! that the string-keyed LSP match arms never tracked:
//!
//! - **E301** (`"E301" | "E305" => missing_terminator_actions`, LSP):
//!   `ErrorCode::MissingMainTier` (E301) is currently "Empty speaker code"
//!   (verified: `*:\thello world .` produces `error[E301]: Empty speaker
//!   code in main tier`), unrelated to terminators. There is no
//!   discoverable correct speaker code to insert, so E301 gets no catalog
//!   entry; only E305 carries the terminator fix.
//! - **E242** (LSP: inserts `" +..."`, "trailing off marker"):
//!   `ErrorCode::UnbalancedQuotation` (E242) is actually "Unbalanced
//!   quotation in word content" (verified against
//!   `spec/errors/E242.md` and a live run). Appending a trailing-off
//!   marker does not balance a quotation mark. No confident single-answer
//!   fix exists (a missing open and a missing close both produce the same
//!   diagnostic), so E242 gets no catalog entry.
//! - **E501** (LSP: `insert_after_utf8(..., "@Begin\n", "Insert '@Begin'
//!   after @UTF8")`): `ErrorCode::DuplicateHeader` (E501) is a DUPLICATE
//!   `@Begin` (verified: two `@Begin` lines produce `error[E501]:
//!   Duplicate @Begin header: only one @Begin is allowed per file`), the
//!   opposite condition from "missing @Begin". This module gives E501 a
//!   correct fix instead (delete the flagged duplicate line).
//! - **E362** (LSP: swaps the two numbers inside the flagged bullet):
//!   verified the real diagnostic ("Media bullet timestamp Nms comes before
//!   previous timestamp") is a CROSS-utterance monotonicity check, not a
//!   within-bullet backwards range. Swapping this bullet's own two numbers
//!   does not fix cross-utterance ordering and can introduce a new
//!   within-bullet backwards range. No catalog entry.
//! - **E322** (LSP: `delete_diagnostic_line`, "Delete empty colon line"):
//!   `ErrorCode::EmptyColon` describes a missing colon TOKEN, not a line
//!   worth deleting wholesale; deleting the entire utterance to fix a
//!   missing punctuation mark is disproportionate. Also currently
//!   unreachable (`spec/errors/E322.md`: `Status: not_implemented`).
//!   No catalog entry.
//! - **E506** (LSP: `replace_diagnostic_range` with a participant
//!   template): every real E506 diagnostic observed here was unlocated
//!   (formerly the `0..0` sentinel), regardless of where the empty
//!   `@Participants` header actually is, and the catalog refuses an unlocated
//!   diagnosis (`CatalogIneligibility::Unlocated`) rather than guessing at
//!   file start. This is a `chatter` diagnostic-emission defect (E506 should
//!   carry a real location), not a catalog design gap; fixing it is out of
//!   this module's scope. No catalog entry until it is fixed upstream.
//! - **E312 / E313 / E323** (LSP: append `]` / `)` / `:`): the fixes
//!   themselves are sound in intent, but all three codes are currently
//!   unreachable via the tree-sitter parser (`spec/errors/E312.md`,
//!   `E313.md`, `E323.md`: `Status: not_implemented`; the
//!   grammar produces a different code, usually E304/E316/E375, for every
//!   input tried). A catalog entry with no way to construct a real
//!   `ParseError` to test it against is speculative, not verified, so
//!   these get no entry either. Add them once the grammar can reach them.
//!
//! Two more ports needed a span-precision correction, not an exclusion:
//!
//! - **E244**: the LSP replaces the WHOLE diagnostic span (which covers
//!   the entire word, e.g. `"ˈˈhello"`) with a single `"ˈ"`, which would
//!   delete `"hello"`. This module locates the run of consecutive stress
//!   marks WITHIN the span and replaces only that run.
//! - **E258**: the diagnostic span here covers exactly ONE of the two
//!   commas (one byte), not both. Replacing that one byte with `","` (the
//!   LSP's literal action) is a no-op. This module deletes the flagged
//!   byte instead, which collapses `",,"` to `","`, the same intent the
//!   LSP's title states.
//!
//! Every ported entry additionally verifies the text actually at its span
//! matches what the fix assumes before building an edit, and reports a
//! non-offer [`CatalogResolution`] rather than guessing when it does not.
//! E241 is the entry that shows what
//! that check should look like: it asks the model's vocabulary owner whether
//! the span reads as a misspelled marker, rather than comparing it to a
//! literal, which is what the rest of this catalog still does. The retained
//! diagnosis supplies its own parse capability to the private resolver behind
//! [`crate::splice::FixFinding::catalog`]
//! for exactly this: a diagnostic's span is trusted data about WHERE, never
//! about WHAT is there.
//!
//! # `DiagnosticKind` was checked and found orthogonal
//!
//! `talkbank_model::errors::diagnostic_kind` classifies every code into
//! `Invalidity` / `Unmodeled` / `Deprecation` / `Style`, an axis about the
//! RULE's nature. [`BatchSafety`] is a different axis, about how safe an
//! unattended REWRITE is, and the two do not line up (E501, E502, E503,
//! E506, E507 are all `DiagnosticKind::Invalidity` despite needing very
//! different `BatchSafety` treatment here). There was nothing to derive
//! from that registry for this module.
//!
//! # Header-scoped codes need a recovery-free parse, not utterance gating
//!
//! [`crate::splice::admit_edits`] admits an edit only when
//! `ChatFile::utterance_containing` finds an enclosing utterance whose
//! parse health is `Clean`. E501, E502 and E503 proposals target headers
//! outside utterances, so `utterance_containing`
//! will never find an enclosing utterance for them and `admit_edits` will
//! always report `SkipReason::OutsideAnyUtterance`. That is a real,
//! documented limitation for those entries. W109 has a separate, narrow
//! capability: its catalog resolves a filename token from a recovery-free,
//! source-bound generated media header. Only that token is admitted outside
//! utterances; no generic header-edit bypass is exposed.
//! E501 additionally requires proven identical source-bound declarations;
//! conflicting information is never a mechanical deletion proposal.

use talkbank_model::model::content::word::MarkerSpelling;
use talkbank_model::{DiagnosticLocation, ErrorCode, ParseError, Span};

use super::engine::{EditProvenance, EditTarget, Replacement, SpliceEdit};
mod proposal;
mod refusal;
pub use proposal::{BatchSafety, CatalogFix, FixKind, NamedAlternative};
use refusal::CatalogRefusal;
pub use refusal::{CatalogFault, CatalogIneligibility, CatalogResolution};
type CatalogResult = Result<CatalogFix, CatalogRefusal>;

/// The located part of a finding that a source-reading fix consumes: its code
/// and the span it diagnoses. Built once in [`catalog_fix`], so no fix reads a
/// diagnostic location that might be unlocated.
#[derive(Clone, Copy)]
struct Diagnosis {
    code: ErrorCode,
    span: Span,
}

/// Private resolver: only an owner-issued finding supplies this associated pair.
pub(super) fn catalog_fix(
    error: &ParseError,
    parsed: &talkbank_parser::generated_traversal::ParsedSource<'_>,
) -> CatalogResolution {
    let source = parsed.source();
    // Every fix except the whole-file header insertions reads the source at
    // the diagnosis; an unlocated diagnostic gives it nowhere to look.
    let located = || match error.location {
        DiagnosticLocation::Known { span, .. } => Ok(Diagnosis {
            code: error.code,
            span,
        }),
        DiagnosticLocation::Unlocated => Err(CatalogIneligibility::Unlocated.into()),
    };
    let result = match error.code {
        ErrorCode::IllegalUntranscribed => {
            located().and_then(|d| e241_illegal_untranscribed(d, parsed))
        }
        ErrorCode::ConsecutiveStressMarkers => {
            located().and_then(|d| e244_consecutive_stress_markers(d, parsed))
        }
        ErrorCode::ConsecutiveCommas => located().and_then(|d| e258_consecutive_commas(d, parsed)),
        ErrorCode::CommaAfterNonSpokenContent => {
            located().and_then(|d| e259_comma_after_non_spoken_content(d, parsed))
        }
        ErrorCode::MissingTerminator => located().and_then(|d| e305_missing_terminator(d, parsed)),
        ErrorCode::EmptyUtterance => located().and_then(|d| e306_empty_utterance(d, parsed)),
        ErrorCode::DuplicateHeader => located().and_then(|d| e501_duplicate_header(d, parsed)),
        ErrorCode::MissingEndHeader => e502_missing_end_header(error.code, source),
        ErrorCode::MissingUTF8Header => e503_missing_utf8_header(error.code),
        ErrorCode::GraWithoutMor => located().and_then(|d| e604_gra_without_mor(d, parsed)),
        ErrorCode::SpaceInsideAngleGroup => {
            located().and_then(|d| e750_space_inside_angle_group(d, parsed))
        }
        ErrorCode::MediaFilenameNonCanonicalUnicode => {
            located().and_then(|d| w109_media_name(d, parsed))
        }
        _ => return CatalogResolution::Unsupported,
    };
    match result {
        Ok(fix) => CatalogResolution::Offered(fix),
        Err(CatalogRefusal::Ineligible(reason)) => CatalogResolution::Ineligible(reason),
        Err(CatalogRefusal::Fault(fault)) => CatalogResolution::Fault(fault),
    }
}

/// Locate a typed enclosing node without reconstructing CHAT from source text.
fn enclosing_node<'tree, 'source, T>(
    parsed: &'tree talkbank_parser::generated_traversal::ParsedSource<'source>,
    span: Span,
) -> Result<talkbank_parser::generated_traversal::SourceBound<'tree, 'source, T>, CatalogRefusal>
where
    T: talkbank_parser::generated_traversal::SourceBoundKind<'tree>
        + talkbank_parser::generated_traversal::FromNodeKind<'tree>,
{
    let mut node = parsed
        .root_node()
        .descendant_for_byte_range(span.start(), span.end())
        .ok_or(CatalogIneligibility::MissingCarrier)?;
    loop {
        if let Some(typed) = parsed.bind(node)?.typed::<T>() {
            return Ok(typed);
        }
        node = node.parent().ok_or(CatalogIneligibility::MissingCarrier)?;
    }
}

/// Normalize only the generated CST's filename token in a clean media header.
/// URL tokens and already-canonical names never produce edits.
fn w109_media_name(
    diagnosis: Diagnosis,
    parsed: &talkbank_parser::generated_traversal::ParsedSource<'_>,
) -> CatalogResult {
    use talkbank_parser::generated_traversal::{AsRawNode, MediaHeaderNode, SourceSlotView};
    use unicode_normalization::UnicodeNormalization;
    let header = enclosing_node::<MediaHeaderNode>(parsed, diagnosis.span)?;
    if header.raw_node().has_error() {
        return Err(CatalogIneligibility::RecoveredCarrier.into());
    }
    let fields = header.extract();
    let SourceSlotView::Present(contents) = fields.field_child_2().slot().view() else {
        return Err(CatalogIneligibility::MissingCarrier.into());
    };
    let contents = contents.read()?;
    let fields = contents.extract();
    let SourceSlotView::Present(filename) = fields.field_child_0().slot().view() else {
        return Err(CatalogIneligibility::MissingCarrier.into());
    };
    let filename = filename.read()?;
    let text = filename.text();
    if talkbank_model::model::MediaFilename::parse(text)
        .map_err(|_| CatalogIneligibility::MediaToken)?
        .is_remote_url()
    {
        return Err(CatalogIneligibility::MediaToken.into());
    }
    let canonical: String = text.nfc().collect();
    if canonical == text {
        return Err(CatalogIneligibility::MediaToken.into());
    }
    let range = filename.raw_node().byte_range();
    let edit = SpliceEdit::new_header_token(
        EditTarget::Replace(Span::new(range.start, range.end)),
        Replacement::new(canonical),
        diagnosis.code,
    );
    Ok(CatalogFix::mechanical(vec![edit]))
}

/// E241 replaces a complete source-bound word only when the model-owned
/// [`MarkerSpelling::of`] classifies its spelling as a misspelled marker.
/// Do not duplicate that vocabulary or erase omission/shortening notation.
fn e241_illegal_untranscribed(
    diagnosis: Diagnosis,
    parsed: &talkbank_parser::generated_traversal::ParsedSource<'_>,
) -> CatalogResult {
    use talkbank_parser::generated_traversal::{AsRawNode, StandaloneWordNode};
    let span = diagnosis.span;
    let word = enclosing_node::<StandaloneWordNode>(parsed, span)?;
    if word.raw_node().byte_range() != span.to_range() {
        return Err(CatalogIneligibility::TokenMismatch.into());
    }
    if word.raw_node().has_error() {
        return Err(CatalogIneligibility::RecoveredCarrier.into());
    }
    // The token itself must be a marker, not merely its cleaned spelling.
    let intended = MarkerSpelling::of(word.text())
        .misspelled()
        .ok_or(CatalogIneligibility::NoRepairableToken)?;
    let edit = SpliceEdit::new(
        EditTarget::Replace(span),
        Replacement::new(intended.canonical()),
        EditProvenance::Diagnostic(diagnosis.code),
    );
    Ok(CatalogFix::mechanical(vec![edit]))
}

/// E244 `ConsecutiveStressMarkers`: collapse runs of consecutive primary
/// stress marks (`ˈ`, U+02C8) to one, without touching the word content the
/// diagnostic span also covers (see the module-level doc for why the naive
/// whole-span replace this was ported from is unsafe).
fn e244_consecutive_stress_markers(
    diagnosis: Diagnosis,
    parsed: &talkbank_parser::generated_traversal::ParsedSource<'_>,
) -> CatalogResult {
    use talkbank_parser::generated_traversal::{
        AsRawNode, SourceBound, StandaloneWordNode, StressMarkerNode,
    };
    // Only a checked primary token can become the previous primary witness.
    struct PrimaryStress<'tree, 'source>(SourceBound<'tree, 'source, StressMarkerNode<'tree>>);
    let word = enclosing_node::<StandaloneWordNode>(parsed, diagnosis.span)?;
    if word.raw_node().has_error() {
        return Err(CatalogIneligibility::RecoveredCarrier.into());
    }
    let mut previous: Option<PrimaryStress<'_, '_>> = None;
    let mut edits = Vec::new();
    for node in word.source_slice().descendants() {
        let Some(mark) = node?.typed::<StressMarkerNode>() else {
            continue;
        };
        if mark.text() != "ˈ" {
            previous = None;
            continue;
        }
        let current = PrimaryStress(mark);
        if let Some(previous) = previous.as_ref()
            && previous.0.raw_node().end_byte() == current.0.raw_node().start_byte()
        {
            edits.push(SpliceEdit::new(
                EditTarget::Replace(Span::new(
                    current.0.raw_node().start_byte(),
                    current.0.raw_node().end_byte(),
                )),
                Replacement::new(""),
                EditProvenance::Diagnostic(diagnosis.code),
            ));
        }
        previous = Some(current);
    }
    if edits.is_empty() {
        Err(CatalogIneligibility::NoRepairableToken.into())
    } else {
        Ok(CatalogFix::mechanical(edits))
    }
}

/// E258 `ConsecutiveCommas`: the diagnostic span covers exactly one of the
/// pair; deleting it collapses `",,"` to `","`.
fn e258_consecutive_commas(
    diagnosis: Diagnosis,
    parsed: &talkbank_parser::generated_traversal::ParsedSource<'_>,
) -> CatalogResult {
    use talkbank_parser::generated_traversal::{AsRawNode, TierBodyNode};
    let comma = diagnostic_comma(parsed, diagnosis.span)?;
    let body = enclosing_node::<TierBodyNode>(parsed, diagnosis.span)?;
    if body.raw_node().has_error() {
        return Err(CatalogIneligibility::RecoveredCarrier.into());
    }
    let edit = SpliceEdit::new(
        EditTarget::Replace(Span::new(
            comma.raw_node().start_byte(),
            comma.raw_node().end_byte(),
        )),
        Replacement::new(""),
        EditProvenance::Diagnostic(diagnosis.code),
    );
    Ok(CatalogFix::mechanical(vec![edit]))
}

/// Admit only a complete grammar comma at the diagnostic's exact source range.
fn diagnostic_comma<'tree, 'source>(
    parsed: &'tree talkbank_parser::generated_traversal::ParsedSource<'source>,
    span: Span,
) -> Result<
    talkbank_parser::generated_traversal::SourceBound<
        'tree,
        'source,
        talkbank_parser::generated_traversal::CommaNode<'tree>,
    >,
    CatalogRefusal,
> {
    use talkbank_parser::generated_traversal::{AsRawNode, CommaNode};
    let comma = enclosing_node::<CommaNode>(parsed, span)?;
    if comma.raw_node().byte_range() == span.to_range() {
        Ok(comma)
    } else {
        Err(CatalogIneligibility::TokenMismatch.into())
    }
}

/// E259 `CommaAfterNonSpokenContent`: delete the comma that has no
/// preceding spoken word to attach to. Semantic, not mechanical: unlike
/// E258's redundant comma, this comma is the ONLY one at its position, so
/// deleting it is a real content change rather than de-duplication.
fn e259_comma_after_non_spoken_content(
    diagnosis: Diagnosis,
    parsed: &talkbank_parser::generated_traversal::ParsedSource<'_>,
) -> CatalogResult {
    use talkbank_parser::generated_traversal::{AsRawNode, TierBodyNode, WhitespacesNode};
    let span = diagnosis.span;
    let comma = diagnostic_comma(parsed, span)?;
    let body = enclosing_node::<TierBodyNode>(parsed, span)?;
    if body.raw_node().has_error() {
        return Err(CatalogIneligibility::RecoveredCarrier.into());
    }
    let mut delete_span = span;
    if comma.raw_node().start_byte() == body.raw_node().start_byte() {
        // Only a body-initial comma owns the following separator: deleting it
        // alone would introduce leading whitespace. Interior commas retain it.
        for node in body.source_slice().descendants() {
            if let Some(space) = node?.typed::<WhitespacesNode>()
                && space.raw_node().start_byte() == comma.raw_node().end_byte()
            {
                delete_span = Span::new(comma.raw_node().start_byte(), space.raw_node().end_byte());
                break;
            }
        }
    }
    let edit = SpliceEdit::new(
        EditTarget::Replace(delete_span),
        Replacement::new(""),
        EditProvenance::Diagnostic(diagnosis.code),
    );
    Ok(CatalogFix::semantic(vec![edit]))
}

/// E305 `MissingTerminator`: fires both for a main-tier utterance and a
/// `%mor` tier missing its own terminator; either way there are three
/// equally valid answers (`.`, `?`, `!`) and no evidence in the file picks
/// one, so this is never a single deterministic fix.
///
/// Main tiers insert before the grammar-owned ending (including final codes);
/// MOR inserts before its terminal newline. Neither splits CRLF or guesses
/// structure from diagnostic text. Recovered tiers decline a proposal.
fn e305_missing_terminator(
    diagnosis: Diagnosis,
    parsed: &talkbank_parser::generated_traversal::ParsedSource<'_>,
) -> CatalogResult {
    use talkbank_parser::generated_traversal::{
        AsRawNode, MainTierNode, MorDependentTierNode, NewlineNode, UtteranceEndNode,
    };
    let anchor = match enclosing_node::<MainTierNode>(parsed, diagnosis.span) {
        Ok(main) => terminal_descendant::<UtteranceEndNode>(main.source_slice())?.source_slice(),
        Err(CatalogRefusal::Ineligible(CatalogIneligibility::MissingCarrier)) => {
            let mor = enclosing_node::<MorDependentTierNode>(parsed, diagnosis.span)?;
            terminal_descendant::<NewlineNode>(mor.source_slice())?.source_slice()
        }
        Err(reason) => return Err(reason),
    };
    let insert_at = anchor.raw_node().start_byte();
    let alternatives = [
        (".", "Add '.' (declarative/default)"),
        ("?", "Add '?' (question)"),
        ("!", "Add '!' (exclamation)"),
    ]
    .into_iter()
    .map(|(terminator, label)| NamedAlternative {
        label: label.to_string(),
        edits: vec![SpliceEdit::new(
            EditTarget::InsertAt(insert_at),
            Replacement::new(format!(" {terminator}")),
            EditProvenance::Diagnostic(diagnosis.code),
        )],
    })
    .collect();
    Ok(CatalogFix::alternatives(alternatives))
}

/// Admit a unique typed descendant ending at its clean carrier's boundary.
fn terminal_descendant<'tree, 'source, T>(
    tier: talkbank_parser::generated_traversal::SourceSlice<'tree, 'source>,
) -> Result<talkbank_parser::generated_traversal::SourceBound<'tree, 'source, T>, CatalogRefusal>
where
    T: talkbank_parser::generated_traversal::SourceBoundKind<'tree>
        + talkbank_parser::generated_traversal::FromNodeKind<'tree>,
{
    use talkbank_parser::generated_traversal::AsRawNode;
    if tier.raw_node().has_error() {
        return Err(CatalogIneligibility::RecoveredCarrier.into());
    }
    let mut terminal = None;
    for node in tier.descendants() {
        if let Some(anchor) = node?.typed::<T>()
            && anchor.raw_node().end_byte() == tier.raw_node().end_byte()
        {
            if terminal.is_some() {
                return Err(CatalogIneligibility::NonUniqueCarrier.into());
            }
            terminal = Some(anchor);
        }
    }
    terminal.ok_or(CatalogIneligibility::MissingCarrier.into())
}

/// E306: delete a complete, clean main-tier-only utterance. Dependent tiers
/// must never be orphaned, reassigned or discarded by this semantic proposal.
fn e306_empty_utterance(
    diagnosis: Diagnosis,
    parsed: &talkbank_parser::generated_traversal::ParsedSource<'_>,
) -> CatalogResult {
    use talkbank_parser::generated_traversal::{AsRawNode, MainTierNode, UtteranceNode};
    let main = enclosing_node::<MainTierNode>(parsed, diagnosis.span)?;
    let owner = enclosing_node::<UtteranceNode>(parsed, diagnosis.span)?;
    if owner.raw_node().has_error() {
        return Err(CatalogIneligibility::RecoveredCarrier.into());
    }
    if owner.raw_node().byte_range() != main.raw_node().byte_range() {
        return Err(CatalogIneligibility::DependentTiers.into());
    }
    let range = main.raw_node().byte_range();
    let edit = SpliceEdit::new(
        EditTarget::Replace(Span::new(range.start, range.end)),
        Replacement::new(""),
        EditProvenance::Diagnostic(diagnosis.code),
    );
    Ok(CatalogFix::semantic(vec![edit]))
}

/// Evidence that the selected complete header repeats earlier identical text,
/// and no declaration of the same grammar variant conflicts with it.
struct IdenticalHeaderDuplicate<'tree, 'source>(
    talkbank_parser::generated_traversal::SourceBound<
        'tree,
        'source,
        talkbank_parser::generated_traversal::HeaderChoice<'tree>,
    >,
);

impl<'tree, 'source> IdenticalHeaderDuplicate<'tree, 'source> {
    fn admit(
        parsed: &'tree talkbank_parser::generated_traversal::ParsedSource<'source>,
        span: Span,
    ) -> Result<Self, CatalogRefusal> {
        use talkbank_parser::generated_traversal::{AsRawNode, HeaderChoice};
        let root = parsed.root_node();
        if root.has_error() {
            return Err(CatalogIneligibility::RecoveredCarrier.into());
        }
        let target = enclosing_node::<HeaderChoice>(parsed, span)?;
        let kind = std::mem::discriminant(&target.view());
        let mut earlier = false;
        for node in parsed.bind(root)?.descendants() {
            let Some(header) = node?.typed::<HeaderChoice>() else {
                continue;
            };
            if std::mem::discriminant(&header.view()) != kind {
                continue;
            }
            // Exact source equality is deliberately conservative: a whitespace
            // difference is not proof of interchangeable declarations.
            if header.text() != target.text() {
                return Err(CatalogIneligibility::HeaderConflict.into());
            }
            earlier |= header.raw_node().end_byte() <= target.raw_node().start_byte();
        }
        earlier
            .then_some(Self(target))
            .ok_or(CatalogIneligibility::HeaderConflict.into())
    }

    fn into_fix(self, code: ErrorCode) -> CatalogFix {
        use talkbank_parser::generated_traversal::AsRawNode;
        let range = self.0.raw_node().byte_range();
        CatalogFix::mechanical(vec![SpliceEdit::new(
            EditTarget::Replace(Span::new(range.start, range.end)),
            Replacement::new(""),
            EditProvenance::Diagnostic(code),
        )])
    }
}

/// E501 proposes deleting only a proven identical, complete header.
/// Conflicts and recovered structure refuse a proposal. Header edit admission
/// still reports OutsideAnyUtterance; this proof does not bypass that boundary.
fn e501_duplicate_header(
    diagnosis: Diagnosis,
    parsed: &talkbank_parser::generated_traversal::ParsedSource<'_>,
) -> CatalogResult {
    Ok(IdenticalHeaderDuplicate::admit(parsed, diagnosis.span)?.into_fix(diagnosis.code))
}

/// E502 `MissingEndHeader`: append `@End` at end of file, prefixing a
/// newline first if the file does not already end with one. Mechanical:
/// every valid CHAT file ends this way, no judgment involved.
///
/// Header-scoped, so `chatter fix` cannot apply it today; see the note on
/// `e501_duplicate_header` above.
fn e502_missing_end_header(code: ErrorCode, source: &str) -> CatalogResult {
    let text = if source.ends_with('\n') {
        "@End\n"
    } else {
        "\n@End\n"
    };
    let edit = SpliceEdit::new(
        EditTarget::InsertAt(source.len()),
        Replacement::new(text),
        EditProvenance::Diagnostic(code),
    );
    Ok(CatalogFix::mechanical(vec![edit]))
}

/// E503 `MissingUTF8Header`: prepend `@UTF8` as the first line. Mechanical:
/// every modern CHAT file declares this, no judgment involved.
///
/// Header-scoped, so `chatter fix` cannot apply it today; see the note on
/// `e501_duplicate_header` above.
fn e503_missing_utf8_header(code: ErrorCode) -> CatalogResult {
    let edit = SpliceEdit::new(
        EditTarget::InsertAt(0),
        Replacement::new("@UTF8\n"),
        EditProvenance::Diagnostic(code),
    );
    Ok(CatalogFix::mechanical(vec![edit]))
}

/// E604 `GraWithoutMor`: the diagnostic anchors to the main tier the
/// orphaned `%gra` belongs to, not to the tier itself. Select one complete,
/// recovery-free generated GRA tier within that typed utterance. Intervening
/// tiers and continuation lines do not affect ownership. Multiple GRA tiers
/// refuse selection rather than guessing. Semantic: removes tier content.
fn e604_gra_without_mor(
    diagnosis: Diagnosis,
    parsed: &talkbank_parser::generated_traversal::ParsedSource<'_>,
) -> CatalogResult {
    use talkbank_parser::generated_traversal::{AsRawNode, GraDependentTierNode, UtteranceNode};
    let utterance = enclosing_node::<UtteranceNode>(parsed, diagnosis.span)?;
    let mut selected = None;
    for child in utterance.descendants() {
        if let Some(gra) = child?.typed::<GraDependentTierNode>() {
            if gra.raw_node().has_error() {
                return Err(CatalogIneligibility::RecoveredCarrier.into());
            }
            if selected.is_some() {
                return Err(CatalogIneligibility::NonUniqueCarrier.into());
            }
            selected = Some(gra);
        }
    }
    let gra = selected.ok_or(CatalogIneligibility::MissingCarrier)?;
    let range = gra.raw_node().byte_range();
    let edit = SpliceEdit::new(
        EditTarget::Replace(Span::new(range.start, range.end)),
        Replacement::new(""),
        EditProvenance::Diagnostic(diagnosis.code),
    );
    Ok(CatalogFix::semantic(vec![edit]))
}

/// E750 `SpaceInsideAngleGroup`: remove the whitespace run immediately after
/// `<` or immediately before `>`. This is mechanical because delimiters must
/// hug the same group content; deleting only that separator changes neither
/// tokens nor group structure.
fn e750_space_inside_angle_group(
    diagnosis: Diagnosis,
    parsed: &talkbank_parser::generated_traversal::ParsedSource<'_>,
) -> CatalogResult {
    use talkbank_parser::generated_traversal::{
        AsRawNode, GroupWithAnnotationsNode, SourceSlotView, WhitespacesNode,
    };
    let span = diagnosis.span;
    let whitespace = enclosing_node::<WhitespacesNode>(parsed, span)?;
    if whitespace.raw_node().byte_range() != span.to_range()
        || whitespace.text().is_empty()
        || !whitespace.text().bytes().all(|byte| byte == b' ')
    {
        return Err(CatalogIneligibility::GroupEdge.into());
    }
    let group = enclosing_node::<GroupWithAnnotationsNode>(parsed, span)?;
    if group.raw_node().has_error() {
        return Err(CatalogIneligibility::RecoveredCarrier.into());
    }
    let fields = group.extract();
    let content = fields.field_content_2();
    let SourceSlotView::Present(content) = content.slot().view() else {
        return Err(CatalogIneligibility::MissingCarrier.into());
    };
    let content = content.read()?;
    if whitespace.raw_node().start_byte() != content.raw_node().start_byte()
        && whitespace.raw_node().end_byte() != content.raw_node().end_byte()
    {
        return Err(CatalogIneligibility::GroupEdge.into());
    }
    let edit = SpliceEdit::new_recovery_repair(
        EditTarget::Replace(span),
        Replacement::new(""),
        diagnosis.code,
    );
    Ok(CatalogFix::mechanical(vec![edit]))
}

#[cfg(test)]
mod tests {
    use talkbank_model::{
        model::TranscriptName,
        validation::{AlignmentValidation, ValidationPolicy},
    };
    use talkbank_parser::TreeSitterParser;

    use super::{BatchSafety, FixKind};
    use crate::splice::FixDiagnosis;
    use crate::splice::apply_edits;

    #[test]
    fn e750_mechanically_removes_only_the_space_inside_group_delimiters() {
        let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\t< dog > [/] dog .\n@End\n";
        let parser = TreeSitterParser::new().expect("tree-sitter parser initializes");
        let diagnosis = FixDiagnosis::observe(
            &parser,
            source,
            ValidationPolicy::new(
                talkbank_model::RuleSelection::new(),
                AlignmentValidation::Structure,
            ),
            TranscriptName::Anonymous,
        );
        let fixes = diagnosis
            .findings()
            .filter(|error| error.code.as_str() == "E750")
            .map(|error| {
                error
                    .catalog()
                    .into_offer()
                    .expect("E750 has a catalog fix")
            })
            .collect::<Vec<_>>();

        assert_eq!(fixes.len(), 2);
        assert!(
            fixes
                .iter()
                .all(|fix| fix.safety() == BatchSafety::Mechanical)
        );
        let edits = fixes
            .into_iter()
            .flat_map(|fix| match fix.into_kind() {
                FixKind::Mechanical(edits) | FixKind::Semantic(edits) => edits,
                FixKind::Alternatives(_) => panic!("E750 has only one correct repair"),
            })
            .collect::<Vec<_>>();
        let fixed = apply_edits(source, &edits).expect("non-overlapping E750 edits apply");
        assert!(fixed.contains("*CHI:\t<dog> [/] dog ."), "got {fixed:?}");
    }
}
