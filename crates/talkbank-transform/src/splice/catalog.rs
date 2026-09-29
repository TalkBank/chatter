//! The fix catalog: one answer to "what fix does error code X get".
//!
//! Before this module, "what fix does error code X get" had two unshared
//! answers: the LSP's `code_action_fixes.rs` (~21 codes, a human accepts
//! each action individually in an editor) and `chatter lint`'s own
//! three-code copy (deleted 2026-07-31, a live unguarded byte writer). This
//! module is the single answer both a future editor integration and a
//! future batch CLI can consume, and it adds the judgment neither
//! predecessor needed on its own: [`BatchSafety`].
//!
//! # Why a tier, not just a fix
//!
//! The LSP catalog can afford to be liberal because a human reviews and
//! accepts each action individually. A batch CLI applying the same catalog
//! over a corpus turns twenty reviewed decisions into twenty unreviewed
//! ones. On 2026-05-06 a batch rewriter damaged 440 files and 679
//! utterances in this corpus; only 5 of those were even detectable by
//! re-validating, because the rest were structurally valid and
//! semantically wrong. [`BatchSafety`] exists so that a caller can tell,
//! per code, whether that risk applies:
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
//! [`catalog_fix`] is not a mechanical port of
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
//!   template): every real E506 diagnostic observed here carries
//!   `location.span == Span::DUMMY` (`{0, 0}`), regardless of where the
//!   empty `@Participants` header actually is. [`crate::splice::apply_edits`]
//!   correctly refuses a `Replace` on the dummy span
//!   ([`crate::splice::SpliceError::DummySpan`]) rather than guessing at
//!   file start, so this module does not build an edit it knows will be
//!   rejected. This is a `chatter` diagnostic-emission defect (E506 should
//!   carry a real span), not a catalog design gap; fixing it is out of
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
//! matches what the fix assumes before building an edit, and returns `None`
//! rather than guessing when it does not. E241 is the entry that shows what
//! that check should look like: it asks the model's vocabulary owner whether
//! the span reads as a misspelled marker, rather than comparing it to a
//! literal, which is what the rest of this catalog still does. The retained
//! parse capability supplies source to [`catalog_fix`]
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
use talkbank_model::{ErrorCode, ParseError, Span};

use super::engine::{EditProvenance, EditTarget, Replacement, SpliceEdit};

/// How safe a fix is to apply without a human looking at each site.
///
/// The LSP catalog can afford to be liberal because a human accepts each
/// action individually. A batch CLI applying the same catalog converts twenty
/// reviewed decisions into twenty unreviewed ones, so every entry carries a
/// tier and bare `--apply` writes only the mechanical ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchSafety {
    /// One right answer, no semantic judgment.
    Mechanical,
    /// Deterministic, but changes meaning enough to require naming the code.
    Semantic,
    /// Several valid answers; never batch-applied.
    Ambiguous,
}

/// One named choice for an ambiguous fix.
#[derive(Clone, Debug)]
pub struct NamedAlternative {
    /// Human-facing label, e.g. "Add '.' (declarative)".
    pub label: String,
    /// The edits this alternative would apply.
    pub edits: Vec<SpliceEdit>,
}

/// What a catalog entry produces for a given diagnostic.
#[derive(Clone, Debug)]
pub enum FixKind {
    /// Exactly one set of edits.
    Deterministic(Vec<SpliceEdit>),
    /// Several mutually exclusive candidate edit sets.
    Alternatives(Vec<NamedAlternative>),
}

/// A catalog entry resolved against one concrete diagnostic.
#[derive(Clone, Debug)]
pub struct CatalogFix {
    /// Batch-safety tier for this code.
    pub safety: BatchSafety,
    /// The edits, or the alternatives.
    pub kind: FixKind,
}

/// Resolve the catalog fix for one diagnostic, if the code has an entry.
///
/// `parsed` retains the exact text and CST from the caller's original parse.
/// No catalog entry reparses that text. The caller must supply a diagnostic
/// from that same input; this capability binds the CST to its text, not an
/// independently supplied diagnostic to its producer. The source is used both
/// to compute edits that need surrounding context (inserting into an
/// existing header line, deleting a whole line rather than a bare span) and
/// to verify a span actually contains what a fix assumes before trusting
/// it, per the module-level doc.
///
/// Every arm is explicit. A code with no entry, whether never considered or
/// deliberately excluded (see the module docs), falls through to `None`;
/// nothing here invents a default fix for a code it does not recognize.
///
/// An independent string cannot substitute for the parse capability:
///
/// ```compile_fail
/// use talkbank_model::ParseError;
/// use talkbank_transform::splice::catalog_fix;
/// fn disconnected(error: &ParseError, source: &str) {
///     let _ = catalog_fix(error, source);
/// }
/// ```
pub fn catalog_fix(
    error: &ParseError,
    parsed: &talkbank_parser::generated_traversal::ParsedSource<'_>,
) -> Option<CatalogFix> {
    let source = parsed.source();
    match error.code {
        ErrorCode::IllegalUntranscribed => e241_illegal_untranscribed(error, parsed),
        ErrorCode::ConsecutiveStressMarkers => e244_consecutive_stress_markers(error, parsed),
        ErrorCode::ConsecutiveCommas => e258_consecutive_commas(error, parsed),
        ErrorCode::CommaAfterNonSpokenContent => e259_comma_after_non_spoken_content(error, parsed),
        ErrorCode::MissingTerminator => e305_missing_terminator(error, parsed),
        ErrorCode::EmptyUtterance => e306_empty_utterance(error, parsed),
        ErrorCode::DuplicateHeader => e501_duplicate_header(error, parsed),
        ErrorCode::MissingEndHeader => e502_missing_end_header(error, source),
        ErrorCode::MissingUTF8Header => e503_missing_utf8_header(error),
        ErrorCode::GraWithoutMor => e604_gra_without_mor(error, parsed),
        ErrorCode::SpaceInsideAngleGroup => e750_space_inside_angle_group(error, parsed),
        ErrorCode::MediaFilenameNonCanonicalUnicode => w109_media_name(error, parsed),

        // Missing participant/language facts require user-supplied values.
        // A diagnostic code or message cannot establish a role or language.
        ErrorCode::UndeclaredSpeaker
        | ErrorCode::MissingRequiredHeader
        | ErrorCode::EmptyLanguagesHeader => None,

        // E301: seed source aliased this to E305's terminator fix, but the
        // real diagnostic is "Empty speaker code", unrelated to terminators
        // (see module docs). No safe fix to guess.
        ErrorCode::MissingMainTier => None,
        // E242: seed source's "+..." insertion does not address the real
        // "Unbalanced quotation" diagnostic (see module docs). Which side
        // is wrong (missing open vs. missing close) is not determinable
        // from the diagnostic alone.
        ErrorCode::UnbalancedQuotation => None,
        // E362: seed source's within-bullet digit swap does not fix the
        // real cross-utterance monotonicity check (see module docs).
        ErrorCode::TimestampBackwards => None,
        // E322: "delete the whole line" is disproportionate to a missing
        // colon token, and the code is currently unreachable via the
        // parser (see module docs).
        ErrorCode::EmptyColon => None,
        // E506: every observed diagnostic carries Span::DUMMY, which the
        // splice engine correctly refuses; there is no location to build
        // an edit against until chatter attaches a real span (see module
        // docs).
        ErrorCode::EmptyParticipantsHeader => None,
        // E312 / E313 / E323: sound fix intent, but all three are
        // currently unreachable via the parser, so there is no way to
        // verify an entry against a real diagnostic (see module docs).
        ErrorCode::UnclosedBracket => None,
        ErrorCode::UnclosedParenthesis => None,
        ErrorCode::MissingColonAfterSpeaker => None,

        _ => None,
    }
}

/// Locate a typed enclosing node without reconstructing CHAT from source text.
fn enclosing_node<'tree, 'source, T>(
    parsed: &'tree talkbank_parser::generated_traversal::ParsedSource<'source>,
    span: Span,
) -> Option<talkbank_parser::generated_traversal::SourceBound<'tree, 'source, T>>
where
    T: talkbank_parser::generated_traversal::SourceBoundKind<'tree>
        + talkbank_parser::generated_traversal::FromNodeKind<'tree>,
{
    let mut node = parsed
        .root_node()
        .descendant_for_byte_range(span.start as usize, span.end as usize)?;
    loop {
        if let Some(typed) = parsed.bind(node).ok()?.typed::<T>() {
            return Some(typed);
        }
        node = node.parent()?;
    }
}

/// Normalize only the generated CST's filename token in a clean media header.
/// URL tokens and already-canonical names never produce edits.
fn w109_media_name(
    error: &ParseError,
    parsed: &talkbank_parser::generated_traversal::ParsedSource<'_>,
) -> Option<CatalogFix> {
    use talkbank_parser::generated_traversal::{AsRawNode, MediaHeaderNode, SourceSlotView};
    use unicode_normalization::UnicodeNormalization;
    let header = enclosing_node::<MediaHeaderNode>(parsed, error.location.span)?;
    if header.raw_node().has_error() {
        return None;
    }
    let fields = header.extract().ok()?;
    let SourceSlotView::Present(contents) = fields.field_child_2().slot().view() else {
        return None;
    };
    let contents = contents.read().ok()?;
    let fields = contents.extract().ok()?;
    let SourceSlotView::Present(filename) = fields.field_child_0().slot().view() else {
        return None;
    };
    let filename = filename.read().ok()?;
    let text = filename.text();
    if talkbank_model::model::MediaFilename::parse(text)
        .ok()?
        .is_remote_url()
    {
        return None;
    }
    let canonical: String = text.nfc().collect();
    if canonical == text {
        return None;
    }
    let range = filename.raw_node().byte_range();
    let edit = SpliceEdit::new_header_token(
        EditTarget::Replace(Span::from_usize(range.start, range.end)),
        Replacement::new(canonical),
        error.code,
    );
    Some(single_edit_fix(BatchSafety::Mechanical, edit))
}

/// A single-edit [`CatalogFix`], the common shape for every deterministic
/// entry in this catalog.
fn single_edit_fix(safety: BatchSafety, edit: SpliceEdit) -> CatalogFix {
    CatalogFix {
        safety,
        kind: FixKind::Deterministic(vec![edit]),
    }
}

/// E241 replaces a complete source-bound word only when the model-owned
/// [`MarkerSpelling::of`] classifies its spelling as a misspelled marker.
/// Do not duplicate that vocabulary or erase omission/shortening notation.
fn e241_illegal_untranscribed(
    error: &ParseError,
    parsed: &talkbank_parser::generated_traversal::ParsedSource<'_>,
) -> Option<CatalogFix> {
    use talkbank_parser::generated_traversal::{AsRawNode, StandaloneWordNode};
    let span = error.location.span;
    let word = enclosing_node::<StandaloneWordNode>(parsed, span)?;
    if word.raw_node().byte_range() != span.to_range() || word.raw_node().has_error() {
        return None;
    }
    // The token itself must be a marker, not merely its cleaned spelling.
    let intended = MarkerSpelling::of(word.text()).misspelled()?;
    let edit = SpliceEdit::new(
        EditTarget::Replace(span),
        Replacement::new(intended.canonical()),
        EditProvenance::Diagnostic(error.code),
    );
    Some(single_edit_fix(BatchSafety::Mechanical, edit))
}

/// E244 `ConsecutiveStressMarkers`: collapse runs of consecutive primary
/// stress marks (`ˈ`, U+02C8) to one, without touching the word content the
/// diagnostic span also covers (see the module-level doc for why the naive
/// whole-span replace this was ported from is unsafe).
fn e244_consecutive_stress_markers(
    error: &ParseError,
    parsed: &talkbank_parser::generated_traversal::ParsedSource<'_>,
) -> Option<CatalogFix> {
    use talkbank_parser::generated_traversal::{
        AsRawNode, SourceBound, StandaloneWordNode, StressMarkerNode,
    };
    // Only a checked primary token can become the previous primary witness.
    struct PrimaryStress<'tree, 'source>(SourceBound<'tree, 'source, StressMarkerNode<'tree>>);
    let word = enclosing_node::<StandaloneWordNode>(parsed, error.location.span)?;
    if word.raw_node().has_error() {
        return None;
    }
    let mut previous: Option<PrimaryStress<'_, '_>> = None;
    let mut edits = Vec::new();
    for node in word.source_slice().descendants() {
        let Some(mark) = node.ok()?.typed::<StressMarkerNode>() else {
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
                EditTarget::Replace(Span::from_usize(
                    current.0.raw_node().start_byte(),
                    current.0.raw_node().end_byte(),
                )),
                Replacement::new(""),
                EditProvenance::Diagnostic(error.code),
            ));
        }
        previous = Some(current);
    }
    (!edits.is_empty()).then_some(CatalogFix {
        safety: BatchSafety::Mechanical,
        kind: FixKind::Deterministic(edits),
    })
}

/// E258 `ConsecutiveCommas`: the diagnostic span covers exactly one of the
/// pair; deleting it collapses `",,"` to `","`.
fn e258_consecutive_commas(
    error: &ParseError,
    parsed: &talkbank_parser::generated_traversal::ParsedSource<'_>,
) -> Option<CatalogFix> {
    use talkbank_parser::generated_traversal::{AsRawNode, TierBodyNode};
    let comma = diagnostic_comma(parsed, error.location.span)?;
    let body = enclosing_node::<TierBodyNode>(parsed, error.location.span)?;
    if body.raw_node().has_error() {
        return None;
    }
    let edit = SpliceEdit::new(
        EditTarget::Replace(Span::from_usize(
            comma.raw_node().start_byte(),
            comma.raw_node().end_byte(),
        )),
        Replacement::new(""),
        EditProvenance::Diagnostic(error.code),
    );
    Some(single_edit_fix(BatchSafety::Mechanical, edit))
}

/// Admit only a complete grammar comma at the diagnostic's exact source range.
fn diagnostic_comma<'tree, 'source>(
    parsed: &'tree talkbank_parser::generated_traversal::ParsedSource<'source>,
    span: Span,
) -> Option<
    talkbank_parser::generated_traversal::SourceBound<
        'tree,
        'source,
        talkbank_parser::generated_traversal::CommaNode<'tree>,
    >,
> {
    use talkbank_parser::generated_traversal::{AsRawNode, CommaNode};
    let comma = enclosing_node::<CommaNode>(parsed, span)?;
    (comma.raw_node().byte_range() == span.to_range()).then_some(comma)
}

/// E259 `CommaAfterNonSpokenContent`: delete the comma that has no
/// preceding spoken word to attach to. Semantic, not mechanical: unlike
/// E258's redundant comma, this comma is the ONLY one at its position, so
/// deleting it is a real content change rather than de-duplication.
fn e259_comma_after_non_spoken_content(
    error: &ParseError,
    parsed: &talkbank_parser::generated_traversal::ParsedSource<'_>,
) -> Option<CatalogFix> {
    use talkbank_parser::generated_traversal::{AsRawNode, TierBodyNode, WhitespacesNode};
    let span = error.location.span;
    let comma = diagnostic_comma(parsed, span)?;
    let body = enclosing_node::<TierBodyNode>(parsed, span)?;
    if body.raw_node().has_error() {
        return None;
    }
    let mut delete_span = span;
    if comma.raw_node().start_byte() == body.raw_node().start_byte() {
        // Only a body-initial comma owns the following separator: deleting it
        // alone would introduce leading whitespace. Interior commas retain it.
        for node in body.source_slice().descendants() {
            if let Some(space) = node.ok()?.typed::<WhitespacesNode>()
                && space.raw_node().start_byte() == comma.raw_node().end_byte()
            {
                delete_span =
                    Span::from_usize(comma.raw_node().start_byte(), space.raw_node().end_byte());
                break;
            }
        }
    }
    let edit = SpliceEdit::new(
        EditTarget::Replace(delete_span),
        Replacement::new(""),
        EditProvenance::Diagnostic(error.code),
    );
    Some(single_edit_fix(BatchSafety::Semantic, edit))
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
    error: &ParseError,
    parsed: &talkbank_parser::generated_traversal::ParsedSource<'_>,
) -> Option<CatalogFix> {
    use talkbank_parser::generated_traversal::{
        AsRawNode, MainTierNode, MorDependentTierNode, NewlineNode, UtteranceEndNode,
    };
    let anchor = if let Some(main) = enclosing_node::<MainTierNode>(parsed, error.location.span) {
        terminal_descendant::<UtteranceEndNode>(main.source_slice())?.source_slice()
    } else {
        let mor = enclosing_node::<MorDependentTierNode>(parsed, error.location.span)?;
        terminal_descendant::<NewlineNode>(mor.source_slice())?.source_slice()
    };
    let insert_at = u32::try_from(anchor.raw_node().start_byte()).ok()?;
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
            EditProvenance::Diagnostic(error.code),
        )],
    })
    .collect();
    Some(CatalogFix {
        safety: BatchSafety::Ambiguous,
        kind: FixKind::Alternatives(alternatives),
    })
}

/// Admit a unique typed descendant ending at its clean carrier's boundary.
fn terminal_descendant<'tree, 'source, T>(
    tier: talkbank_parser::generated_traversal::SourceSlice<'tree, 'source>,
) -> Option<talkbank_parser::generated_traversal::SourceBound<'tree, 'source, T>>
where
    T: talkbank_parser::generated_traversal::SourceBoundKind<'tree>
        + talkbank_parser::generated_traversal::FromNodeKind<'tree>,
{
    use talkbank_parser::generated_traversal::AsRawNode;
    if tier.raw_node().has_error() {
        return None;
    }
    let mut terminal = None;
    for node in tier.descendants() {
        if let Some(anchor) = node.ok()?.typed::<T>()
            && anchor.raw_node().end_byte() == tier.raw_node().end_byte()
        {
            if terminal.is_some() {
                return None;
            }
            terminal = Some(anchor);
        }
    }
    terminal
}

/// E306: delete a complete, clean main-tier-only utterance. Dependent tiers
/// must never be orphaned, reassigned or discarded by this semantic proposal.
fn e306_empty_utterance(
    error: &ParseError,
    parsed: &talkbank_parser::generated_traversal::ParsedSource<'_>,
) -> Option<CatalogFix> {
    use talkbank_parser::generated_traversal::{AsRawNode, MainTierNode, UtteranceNode};
    let main = enclosing_node::<MainTierNode>(parsed, error.location.span)?;
    let owner = enclosing_node::<UtteranceNode>(parsed, error.location.span)?;
    if owner.raw_node().has_error() || owner.raw_node().byte_range() != main.raw_node().byte_range()
    {
        return None;
    }
    let range = main.raw_node().byte_range();
    let edit = SpliceEdit::new(
        EditTarget::Replace(Span::from_usize(range.start, range.end)),
        Replacement::new(""),
        EditProvenance::Diagnostic(error.code),
    );
    Some(single_edit_fix(BatchSafety::Semantic, edit))
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
    ) -> Option<Self> {
        use talkbank_parser::generated_traversal::{AsRawNode, HeaderChoice};
        let root = parsed.root_node();
        if root.has_error() {
            return None;
        }
        let target = enclosing_node::<HeaderChoice>(parsed, span)?;
        let kind = std::mem::discriminant(&target.view());
        let mut earlier = false;
        for node in parsed.bind(root).ok()?.descendants() {
            let Some(header) = node.ok()?.typed::<HeaderChoice>() else {
                continue;
            };
            if std::mem::discriminant(&header.view()) != kind {
                continue;
            }
            // Exact source equality is deliberately conservative: a whitespace
            // difference is not proof of interchangeable declarations.
            if header.text() != target.text() {
                return None;
            }
            earlier |= header.raw_node().end_byte() <= target.raw_node().start_byte();
        }
        earlier.then_some(Self(target))
    }

    fn into_fix(self, code: ErrorCode) -> CatalogFix {
        use talkbank_parser::generated_traversal::AsRawNode;
        let range = self.0.raw_node().byte_range();
        single_edit_fix(
            BatchSafety::Mechanical,
            SpliceEdit::new(
                EditTarget::Replace(Span::from_usize(range.start, range.end)),
                Replacement::new(""),
                EditProvenance::Diagnostic(code),
            ),
        )
    }
}

/// E501 proposes deleting only a proven identical, complete header.
/// Conflicts and recovered structure refuse a proposal. Header edit admission
/// still reports OutsideAnyUtterance; this proof does not bypass that boundary.
fn e501_duplicate_header(
    error: &ParseError,
    parsed: &talkbank_parser::generated_traversal::ParsedSource<'_>,
) -> Option<CatalogFix> {
    Some(IdenticalHeaderDuplicate::admit(parsed, error.location.span)?.into_fix(error.code))
}

/// E502 `MissingEndHeader`: append `@End` at end of file, prefixing a
/// newline first if the file does not already end with one. Mechanical:
/// every valid CHAT file ends this way, no judgment involved.
///
/// Header-scoped, so `chatter fix` cannot apply it today; see the note on
/// `e501_duplicate_header` above.
fn e502_missing_end_header(error: &ParseError, source: &str) -> Option<CatalogFix> {
    let text = if source.ends_with('\n') {
        "@End\n"
    } else {
        "\n@End\n"
    };
    let edit = SpliceEdit::new(
        EditTarget::InsertAt(source.len() as u32),
        Replacement::new(text),
        EditProvenance::Diagnostic(error.code),
    );
    Some(single_edit_fix(BatchSafety::Mechanical, edit))
}

/// E503 `MissingUTF8Header`: prepend `@UTF8` as the first line. Mechanical:
/// every modern CHAT file declares this, no judgment involved.
///
/// Header-scoped, so `chatter fix` cannot apply it today; see the note on
/// `e501_duplicate_header` above.
fn e503_missing_utf8_header(error: &ParseError) -> Option<CatalogFix> {
    let edit = SpliceEdit::new(
        EditTarget::InsertAt(0),
        Replacement::new("@UTF8\n"),
        EditProvenance::Diagnostic(error.code),
    );
    Some(single_edit_fix(BatchSafety::Mechanical, edit))
}

/// E604 `GraWithoutMor`: the diagnostic anchors to the main tier the
/// orphaned `%gra` belongs to, not to the tier itself. Select one complete,
/// recovery-free generated GRA tier within that typed utterance. Intervening
/// tiers and continuation lines do not affect ownership. Multiple GRA tiers
/// refuse selection rather than guessing. Semantic: removes tier content.
fn e604_gra_without_mor(
    error: &ParseError,
    parsed: &talkbank_parser::generated_traversal::ParsedSource<'_>,
) -> Option<CatalogFix> {
    use talkbank_parser::generated_traversal::{AsRawNode, GraDependentTierNode, UtteranceNode};
    let utterance = enclosing_node::<UtteranceNode>(parsed, error.location.span)?;
    let mut selected = None;
    for child in utterance.descendants() {
        if let Some(gra) = child.ok()?.typed::<GraDependentTierNode>() {
            if gra.raw_node().has_error() || selected.is_some() {
                return None;
            }
            selected = Some(gra);
        }
    }
    let gra = selected?;
    let range = gra.raw_node().byte_range();
    let edit = SpliceEdit::new(
        EditTarget::Replace(Span::from_usize(range.start, range.end)),
        Replacement::new(""),
        EditProvenance::Diagnostic(error.code),
    );
    Some(single_edit_fix(BatchSafety::Semantic, edit))
}

/// E750 `SpaceInsideAngleGroup`: remove the whitespace run immediately after
/// `<` or immediately before `>`. This is mechanical because delimiters must
/// hug the same group content; deleting only that separator changes neither
/// tokens nor group structure.
fn e750_space_inside_angle_group(
    error: &ParseError,
    parsed: &talkbank_parser::generated_traversal::ParsedSource<'_>,
) -> Option<CatalogFix> {
    use talkbank_parser::generated_traversal::{
        AsRawNode, GroupWithAnnotationsNode, SourceSlotView, WhitespacesNode,
    };
    let span = error.location.span;
    let whitespace = enclosing_node::<WhitespacesNode>(parsed, span)?;
    if whitespace.raw_node().byte_range() != span.to_range()
        || whitespace.text().is_empty()
        || !whitespace.text().bytes().all(|byte| byte == b' ')
    {
        return None;
    }
    let group = enclosing_node::<GroupWithAnnotationsNode>(parsed, span)?;
    if group.raw_node().has_error() {
        return None;
    }
    let fields = group.extract().ok()?;
    let content = fields.field_content_2();
    let SourceSlotView::Present(content) = content.slot().view() else {
        return None;
    };
    let content = content.read().ok()?;
    if whitespace.raw_node().start_byte() != content.raw_node().start_byte()
        && whitespace.raw_node().end_byte() != content.raw_node().end_byte()
    {
        return None;
    }
    let edit = SpliceEdit::new_recovery_repair(
        EditTarget::Replace(span),
        Replacement::new(""),
        error.code,
    );
    Some(single_edit_fix(BatchSafety::Mechanical, edit))
}

#[cfg(test)]
mod tests {
    use talkbank_model::ErrorCollector;
    use talkbank_parser::TreeSitterParser;

    use super::{BatchSafety, FixKind, catalog_fix};
    use crate::splice::apply_edits;

    #[test]
    fn e750_mechanically_removes_only_the_space_inside_group_delimiters() {
        let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\t< dog > [/] dog .\n@End\n";
        let parser = TreeSitterParser::new().expect("tree-sitter parser initializes");
        let errors = ErrorCollector::new();
        let (_file, parsed) = parser.parse_chat_file_with_source(source, &errors);
        let parsed = parsed.expect("source-bound parse");
        let diagnostics = errors.into_vec();
        let fixes = diagnostics
            .iter()
            .filter(|error| error.code.as_str() == "E750")
            .map(|error| catalog_fix(error, &parsed).expect("E750 has a catalog fix"))
            .collect::<Vec<_>>();

        assert_eq!(fixes.len(), 2);
        assert!(
            fixes
                .iter()
                .all(|fix| fix.safety == BatchSafety::Mechanical)
        );
        let edits = fixes
            .into_iter()
            .flat_map(|fix| match fix.kind {
                FixKind::Deterministic(edits) => edits,
                FixKind::Alternatives(_) => panic!("E750 has only one correct repair"),
            })
            .collect::<Vec<_>>();
        let fixed = apply_edits(source, &edits).expect("non-overlapping E750 edits apply");
        assert!(fixed.contains("*CHI:\t<dog> [/] dog ."), "got {fixed:?}");
    }
}
