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
//!   (deletes content, fabricates a placeholder, changes what a speaker
//!   said) that a batch run should require the caller to name the code.
//! - [`BatchSafety::Ambiguous`]: several valid answers exist and no
//!   evidence in the file picks one. Never batch-applied; only reported.
//!
//! # Porting from the LSP catalog: verified, not copied
//!
//! [`catalog_fix`] is not a mechanical port of
//! `crates/talkbank-lsp/src/backend/features/code_action_fixes.rs`. Every
//! ported code was checked against what the CURRENT `ErrorCode` variant and
//! a real `chatter validate` run actually produce (`chatter validate` is
//! this repo's authority on CHAT validity; see the root `CLAUDE.md`
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
//!   `spec/errors/E242_auto.md` and a live run). Appending a trailing-off
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
//!   unreachable (`spec/errors/E322_auto.md`: `Status: not_implemented`).
//!   No catalog entry.
//! - **E506** (LSP: `replace_diagnostic_range` with a participant
//!   template): every real E506 diagnostic observed here carries
//!   `location.span == Span::DUMMY` (`{0, 0}`), regardless of where the
//!   empty `@Participants` header actually is. [`super::engine::apply_edits`]
//!   correctly refuses a `Replace` on the dummy span
//!   ([`super::engine::SpliceError::DummySpan`]) rather than guessing at
//!   file start, so this module does not build an edit it knows will be
//!   rejected. This is a `chatter` diagnostic-emission defect (E506 should
//!   carry a real span), not a catalog design gap; fixing it is out of
//!   this module's scope. No catalog entry until it is fixed upstream.
//! - **E312 / E313 / E323** (LSP: append `]` / `)` / `:`): the fixes
//!   themselves are sound in intent, but all three codes are currently
//!   unreachable via the tree-sitter parser (`spec/errors/E312_auto.md`,
//!   `E313_auto.md`, `E323_auto.md`: `Status: not_implemented`; the
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
//! literal, which is what the rest of this catalog still does. `source` is threaded into [`catalog_fix`]
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
//! [`super::admit::admit_edits`] admits an edit only when
//! `ChatFile::utterance_containing` finds an enclosing utterance whose
//! parse health is `Clean`. E501, E502, E503, E504, E506, E507 are all
//! header-region diagnostics: their fixes land before the first utterance
//! (or, for E501, on a duplicated `@Begin`), so `utterance_containing`
//! will never find an enclosing utterance for them and `admit_edits` will
//! always report `SkipReason::OutsideAnyUtterance`. That is a real,
//! documented limitation of today's admission gate, not a bug in these
//! catalog entries: a header-scoped admission path is a separate piece of
//! work this module does not attempt.

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
/// `source` is the full text `error` was diagnosed against. It is used both
/// to compute edits that need surrounding context (inserting into an
/// existing header line, deleting a whole line rather than a bare span) and
/// to verify a span actually contains what a fix assumes before trusting
/// it, per the module-level doc.
///
/// Every arm is explicit. A code with no entry, whether never considered or
/// deliberately excluded (see the module docs), falls through to `None`;
/// nothing here invents a default fix for a code it does not recognize.
pub fn catalog_fix(error: &ParseError, source: &str) -> Option<CatalogFix> {
    match error.code {
        ErrorCode::IllegalUntranscribed => e241_illegal_untranscribed(error, source),
        ErrorCode::ConsecutiveStressMarkers => e244_consecutive_stress_markers(error, source),
        ErrorCode::ConsecutiveCommas => e258_consecutive_commas(error, source),
        ErrorCode::CommaAfterNonSpokenContent => e259_comma_after_non_spoken_content(error, source),
        ErrorCode::MissingTerminator => e305_missing_terminator(error, source),
        ErrorCode::EmptyUtterance => e306_empty_utterance(error, source),
        ErrorCode::UndeclaredSpeaker => e308_undeclared_speaker(error, source),
        ErrorCode::DuplicateHeader => e501_duplicate_header(error, source),
        ErrorCode::MissingEndHeader => e502_missing_end_header(error, source),
        ErrorCode::MissingUTF8Header => e503_missing_utf8_header(error),
        ErrorCode::MissingRequiredHeader => e504_missing_required_header(error, source),
        ErrorCode::EmptyLanguagesHeader => e507_empty_languages_header(error, source),
        ErrorCode::GraWithoutMor => e604_gra_without_mor(error, source),

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

/// The full line containing byte `offset`, including its trailing `\n` when
/// the line has one.
///
/// Used to delete or locate whole header/tier/utterance lines, which are
/// often wider than the span a diagnostic anchors to (a diagnostic may
/// point at one offending token inside a line that should be removed in
/// full). Returns `None` rather than panicking when `offset` is out of
/// bounds or not a character boundary, which should not happen for a span
/// that came from real parsing, but this module never trusts that without
/// checking.
fn line_span_at(source: &str, offset: u32) -> Option<Span> {
    let offset = offset as usize;
    if offset > source.len() || !source.is_char_boundary(offset) {
        return None;
    }
    let start = source[..offset].rfind('\n').map_or(0, |i| i + 1);
    let end = source[offset..]
        .find('\n')
        .map_or(source.len(), |i| offset + i + 1);
    Some(Span::from_usize(start, end))
}

/// The byte span of the first line in `source` starting with `prefix`,
/// including its trailing `\n` when present.
fn find_line(source: &str, prefix: &str) -> Option<Span> {
    let mut offset = 0usize;
    for line in source.split_inclusive('\n') {
        if line.starts_with(prefix) {
            return Some(Span::from_usize(offset, offset + line.len()));
        }
        offset += line.len();
    }
    None
}

/// A single-edit [`CatalogFix`], the common shape for every deterministic
/// entry in this catalog.
fn single_edit_fix(safety: BatchSafety, edit: SpliceEdit) -> CatalogFix {
    CatalogFix {
        safety,
        kind: FixKind::Deterministic(vec![edit]),
    }
}

/// E241 `IllegalUntranscribed`: a marker written wrongly has exactly one right
/// spelling, so the repair is the canonical form of whichever marker it is.
///
/// # Why this asks the model instead of comparing to a literal
///
/// It used to read `if source.get(span.to_range())? != "xx" { return None }`
/// and splice in `"xxx"`, which is a fourth hand-written copy of a vocabulary
/// that has three members and six or more wrong spellings. E241 fires on all of
/// them; this could repair one. `chatter fix` therefore reported nothing to do
/// on a file whose only fault was `YYY`, while `chatter validate` on the same
/// file said exactly what was wrong and what it should be.
///
/// [`MarkerSpelling::of`] is the owner of that question, so this stays correct
/// when the vocabulary changes rather than becoming the next copy to drift.
fn e241_illegal_untranscribed(error: &ParseError, source: &str) -> Option<CatalogFix> {
    let span = error.location.span;
    // `None` when the span does not read as a misspelled marker ON ITS OWN, so
    // no edit is certainly right and the fix declines rather than guessing. The
    // real case is an omitted word (`0xx`), where the diagnostic is computed
    // from the cleaned text while the span also covers the `0` prefix:
    // replacing the whole span would silently delete the omission.
    let intended = MarkerSpelling::of(source.get(span.to_range())?).misspelled()?;
    let edit = SpliceEdit::new(
        EditTarget::Replace(span),
        Replacement::new(intended.canonical()),
        EditProvenance::Diagnostic(error.code),
    );
    Some(single_edit_fix(BatchSafety::Mechanical, edit))
}

/// E244 `ConsecutiveStressMarkers`: collapse a run of consecutive primary
/// stress marks (`ˈ`, U+02C8) to one, without touching the word content the
/// diagnostic span also covers (see the module-level doc for why the naive
/// whole-span replace this was ported from is unsafe).
fn e244_consecutive_stress_markers(error: &ParseError, source: &str) -> Option<CatalogFix> {
    const STRESS_MARK: char = '\u{02C8}';

    let span = error.location.span;
    let text = source.get(span.to_range())?;
    let run_start_byte = text.find(STRESS_MARK)?;
    let run_len: usize = text[run_start_byte..]
        .chars()
        .take_while(|&c| c == STRESS_MARK)
        .map(char::len_utf8)
        .sum();
    // A single mark is not "consecutive"; require at least two to collapse.
    if run_len < STRESS_MARK.len_utf8() * 2 {
        return None;
    }
    let run_start = span.start + run_start_byte as u32;
    let run_end = run_start + run_len as u32;
    let edit = SpliceEdit::new(
        EditTarget::Replace(Span::new(run_start, run_end)),
        Replacement::new(STRESS_MARK.to_string()),
        EditProvenance::Diagnostic(error.code),
    );
    Some(single_edit_fix(BatchSafety::Mechanical, edit))
}

/// E258 `ConsecutiveCommas`: the diagnostic span covers exactly one of the
/// pair; deleting it collapses `",,"` to `","`.
fn e258_consecutive_commas(error: &ParseError, source: &str) -> Option<CatalogFix> {
    let span = error.location.span;
    if source.get(span.to_range())? != "," {
        return None;
    }
    let edit = SpliceEdit::new(
        EditTarget::Replace(span),
        Replacement::new(""),
        EditProvenance::Diagnostic(error.code),
    );
    Some(single_edit_fix(BatchSafety::Mechanical, edit))
}

/// E259 `CommaAfterNonSpokenContent`: delete the comma that has no
/// preceding spoken word to attach to. Semantic, not mechanical: unlike
/// E258's redundant comma, this comma is the ONLY one at its position, so
/// deleting it is a real content change rather than de-duplication.
fn e259_comma_after_non_spoken_content(error: &ParseError, source: &str) -> Option<CatalogFix> {
    let span = error.location.span;
    if source.get(span.to_range())? != "," {
        return None;
    }
    let delete_span = widen_tier_initial_comma_deletion(source, span);
    let edit = SpliceEdit::new(
        EditTarget::Replace(delete_span),
        Replacement::new(""),
        EditProvenance::Diagnostic(error.code),
    );
    Some(single_edit_fix(BatchSafety::Semantic, edit))
}

/// Widen a tier-initial comma's deletion span to also consume the one
/// space that follows it.
///
/// A comma directly preceded by the tab that opens the main tier (nothing
/// between them) IS the tier's first character. Deleting only that comma
/// leaves the space after it as a new leading space on the tier, which
/// chatter's own validator (E758 `LeadingSpaceOnMainTier`) correctly
/// rejects: `"*CHI:\t, xx .\n"` naively becomes `"*CHI:\t xx .\n"`, trading
/// one invalidity for another. (Found 2026-07-31 by `chatter fix`'s own
/// post-splice re-parse check in `crates/chatter/src/commands/fix.rs`,
/// exactly the class of bug that check exists to catch.)
///
/// A comma glued to PRECEDING word content instead (`"www, the rest"`, the
/// shape this diagnostic actually fires on most often: real speech attaches
/// a comma directly to the word before it) needs no widening: deleting just
/// the comma already leaves exactly one separating space between the words
/// on either side, so widening there would instead glue them together.
fn widen_tier_initial_comma_deletion(source: &str, span: Span) -> Span {
    let bytes = source.as_bytes();
    let tier_initial = span.start > 0 && bytes.get(span.start as usize - 1) == Some(&b'\t');
    let followed_by_space = bytes.get(span.end as usize) == Some(&b' ');
    if tier_initial && followed_by_space {
        Span::new(span.start, span.end + 1)
    } else {
        span
    }
}

/// E305 `MissingTerminator`: fires both for a main-tier utterance and a
/// `%mor` tier missing its own terminator; either way there are three
/// equally valid answers (`.`, `?`, `!`) and no evidence in the file picks
/// one, so this is never a single deterministic fix.
///
/// The diagnostic's span covers the WHOLE physical line, trailing `\n`
/// included (verified: for `*CHI:\thi\n`, `location.span` is exactly
/// `13..22`, the newline at byte 21 included). Inserting at the raw
/// `span.end` would land the terminator on the NEXT line, ahead of
/// whatever follows, rather than after `hi`. When the spanned text ends
/// with `\n`, this inserts just before it instead.
fn e305_missing_terminator(error: &ParseError, source: &str) -> Option<CatalogFix> {
    let span = error.location.span;
    let text = source.get(span.to_range())?;
    let insert_at = if text.ends_with('\n') {
        span.end - 1
    } else {
        span.end
    };
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

/// E306 `EmptyUtterance`: delete the whole main-tier line once confirmed to
/// actually be one (`*`-prefixed). Semantic: this removes content, even
/// though the content removed is, by definition, meaningless.
fn e306_empty_utterance(error: &ParseError, source: &str) -> Option<CatalogFix> {
    let line = line_span_at(source, error.location.span.start)?;
    let text = source.get(line.to_range())?;
    if !text.starts_with('*') {
        return None;
    }
    let edit = SpliceEdit::new(
        EditTarget::Replace(line),
        Replacement::new(""),
        EditProvenance::Diagnostic(error.code),
    );
    Some(single_edit_fix(BatchSafety::Semantic, edit))
}

/// E308 `UndeclaredSpeaker`: append the speaker code the diagnostic span
/// already names to the `@Participants` header line. Derived from the
/// span's own text rather than parsing it back out of the message string
/// (the seed source's approach), since the span is already exactly the
/// speaker code. Semantic: adds a real participant with a fabricated role
/// name ("Participant"), which needs a human to confirm.
fn e308_undeclared_speaker(error: &ParseError, source: &str) -> Option<CatalogFix> {
    let span = error.location.span;
    let speaker = source.get(span.to_range())?;
    if speaker.is_empty() || !speaker.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    let line = find_line(source, "@Participants:")?;
    let line_text = source.get(line.to_range())?;
    let insert_at = if line_text.ends_with('\n') {
        line.end - 1
    } else {
        line.end
    };
    let edit = SpliceEdit::new(
        EditTarget::InsertAt(insert_at),
        Replacement::new(format!(", {speaker} Participant")),
        EditProvenance::Diagnostic(error.code),
    );
    Some(single_edit_fix(BatchSafety::Semantic, edit))
}

/// E501 `DuplicateHeader`: delete the flagged (later) duplicate line
/// wholesale. Mechanical: the diagnostic already identifies exactly which
/// occurrence is the redundant one; keeping the first and removing the
/// rest loses nothing.
///
/// Header-scoped, so `chatter fix` cannot apply it today: see "Header-scoped
/// codes need a recovery-free parse, not utterance gating" in the module
/// docs above. The edit built here is correct; it is always reported as
/// skipped with `SkipReason::OutsideAnyUtterance`, never written.
fn e501_duplicate_header(error: &ParseError, source: &str) -> Option<CatalogFix> {
    let line = line_span_at(source, error.location.span.start)?;
    let text = source.get(line.to_range())?;
    if !text.trim_end_matches('\n').starts_with('@') {
        return None;
    }
    let edit = SpliceEdit::new(
        EditTarget::Replace(line),
        Replacement::new(""),
        EditProvenance::Diagnostic(error.code),
    );
    Some(single_edit_fix(BatchSafety::Mechanical, edit))
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

/// E504 `MissingRequiredHeader`: one code covers several distinct missing
/// headers (`@Languages`, `@Participants`, `@Begin`, ...), distinguished
/// only by `error.message` text, since `ErrorCode` does not carry which
/// header. Only the `@Participants` case gets a fix here (matching the
/// seed source's own scoping): insert a placeholder participant line right
/// after `@Begin`. Semantic: the participant name is fabricated, not
/// derived from the file.
///
/// Header-scoped, so `chatter fix` cannot apply it today; see the note on
/// `e501_duplicate_header` above.
fn e504_missing_required_header(error: &ParseError, source: &str) -> Option<CatalogFix> {
    if !error.message.contains("@Participants") {
        return None;
    }
    let begin_line = find_line(source, "@Begin")?;
    let edit = SpliceEdit::new(
        EditTarget::InsertAt(begin_line.end),
        Replacement::new("@Participants:\tCHI Child\n"),
        EditProvenance::Diagnostic(error.code),
    );
    Some(single_edit_fix(BatchSafety::Semantic, edit))
}

/// E507 `EmptyLanguagesHeader`: fill in `eng` as the language once
/// confirmed the span is exactly the empty `@Languages:` header key.
/// Semantic: `eng` is a guessed default, not read from the file.
///
/// Header-scoped, so `chatter fix` cannot apply it today; see the note on
/// `e501_duplicate_header` above.
fn e507_empty_languages_header(error: &ParseError, source: &str) -> Option<CatalogFix> {
    let span = error.location.span;
    if source.get(span.to_range())? != "@Languages:" {
        return None;
    }
    let edit = SpliceEdit::new(
        EditTarget::Replace(span),
        Replacement::new("@Languages:\teng"),
        EditProvenance::Diagnostic(error.code),
    );
    Some(single_edit_fix(BatchSafety::Semantic, edit))
}

/// E604 `GraWithoutMor`: the diagnostic anchors to the main tier the
/// orphaned `%gra` belongs to, not to the `%gra` line itself, so the fix
/// looks at the line immediately following and deletes it only once
/// confirmed to actually start with `%gra`. Semantic: removes a whole
/// tier's content.
fn e604_gra_without_mor(error: &ParseError, source: &str) -> Option<CatalogFix> {
    let next_line = line_span_at(source, error.location.span.end)?;
    let text = source.get(next_line.to_range())?;
    if !text.starts_with("%gra") {
        return None;
    }
    let edit = SpliceEdit::new(
        EditTarget::Replace(next_line),
        Replacement::new(""),
        EditProvenance::Diagnostic(error.code),
    );
    Some(single_edit_fix(BatchSafety::Semantic, edit))
}
