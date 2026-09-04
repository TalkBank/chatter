//! The recorded divergences: what the two backends disagree about today.
//!
//! Split out of `error_parity.rs` when that file passed the workspace's 800
//! line hard limit. A file of its own suits it: the list is the thing a
//! contributor edits when they fix a divergence, and it should shrink visibly
//! in a diff without the surrounding machinery moving.

use super::model::Divergence;
// Imported unqualified: rustfmt explodes a tuple literal past 60 columns, so
// over a table this long the qualified form would cost four lines per long
// entry, for no reader benefit in a column whose type the const's own
// signature states. (No row count here on purpose: a number written beside the
// list it counts is the drift this file exists to make visible.)
use super::model::Divergence::{Conflicting, Re2cExtra, Re2cIncomplete, Re2cSilent};

// ---------------------------------------------------------------------------
// The baseline
// ---------------------------------------------------------------------------

/// Every spec case on which the two backends disagree today.
///
/// Keyed by [`super::model::SpecLabel`]'s rendering, the name a failing run
/// prints,
/// so a new entry can be copied straight out of the failure text.
///
/// Delete an entry in the commit that makes the case agree. Adding one is an
/// admission of a new divergence and wants a sentence saying why it ships.
///
/// # Two entries left on 2026-08-21, and what they were measuring
///
/// 2026-09-03: regenerated from the harness output after Phase 5 and Phase 6
/// renamed and re-indexed most spec files (33 keys added, 8 retired, 46 stale
/// removed; the divergence set itself is the same parser behaviour under new
/// names, plus E315 now lexical on tree-sitter).
/// `E502_wor_cascade_regression.md#0` and `#1` went when the spec format moved
/// to frontmatter, and the reason is worth more than the entries were. That
/// spec declares NO examples: its two ```` ```chat ```` blocks sit under
/// `## Minimal Reproduction`, and the second is labelled in the file as a
/// CONTROL that must NOT produce the code. The reader this suite used scanned
/// the whole file for fences and fell back to the FILENAME for an expectation,
/// so it asserted E502 on both, including the block documented as expecting
/// the opposite.
///
/// So they were not two divergences; they were one fabricated expectation,
/// recorded as a divergence, defended by a baseline entry. Reading examples
/// from the file's own declarations is what made them stop existing.
///
/// If those blocks SHOULD be measured, the fix is to declare them as examples
/// in the spec, which changes what is generated and is a separate, adjudicated
/// change.
///
/// # What was here when the gate was first closed, 2026-08-09
///
/// 99 of 283 cases, so the backends agree on **184/283 (65.0%)**. The number
/// the old audit printed, 214/283 (75.6%), was answering the other question
/// (does each backend satisfy the spec) and read as though it answered this
/// one. Nothing regressed between the two figures; only the question did.
///
/// | shape | what the work is |
/// |---|---|
/// | `Conflicting` | each names a code the other does not. Dominated by re2c reporting the generic `E321` where tree-sitter names the specific rule, and by `E600` versus `E605` on `%mor`. |
/// | `Re2cIncomplete` | tree-sitter catches strictly more. A rule re2c has not implemented. |
/// | `Re2cSilent` | re2c reports NOTHING on invalid input. The critical class, and the only one the previous audit named. |
/// | `Re2cExtra` | re2c reports everything tree-sitter does and more. Over-reporting, or tree-sitter under-reporting: adjudicate before assuming which. |
///
/// THE COUNT COLUMN IS GONE, and its removal is the point. It read 61 / 23 /
/// 11 / 4 while the table below held 60 / 20 / 10 / 4, so three of four
/// numbers had drifted, in the file whose own header says a number written
/// beside the list it counts is the drift this file exists to make visible.
/// `backends_diverge_only_where_recorded` PRINTS the live per-shape counts on
/// every failure, which is a derived number and cannot rot; read it there.
///
/// The families matter more than the entries: `E321` alone accounts for over
/// twenty of the conflicts, so one fix should retire a large block of this
/// list at once, and that is the order to work in.
///
/// # Added since
///
/// **2026-08-16, one**: `E756.md#0`, `Re2cIncomplete`.
/// That example's body is a lone space, so the tier is empty AND carries an
/// illegal trailing space after the separator; tree-sitter now reports both
/// E756 and E758, re2c reports only E756. It ships because re2c does not track
/// separator provenance at all and so cannot emit E758 under any input, which
/// is the same gap the `E758_leading_space_on_main_tier` entries above already
/// record: one family, not a new class.
///
/// It became visible, rather than becoming true, when the E756 widening
/// stopped the `%x` parse path dropping empty tiers from the model. While the
/// tier was dropped there was no separator for the validator to judge, so
/// tree-sitter under-reported and accidentally matched re2c. Agreement that
/// rests on both backends missing something is not agreement.
///
/// # Retired since
///
/// **2026-08-15, three at once**: `E511.md`, `E523.md` and
/// `E524.md`. All three were the same defect. The re2c backend lowered a
/// file through an infallible `From`, which had nowhere to put a diagnostic,
/// so the participant join's E522/E523/E524 were computed and dropped. The
/// conversion now takes the caller's sink, and the join's map is reachable
/// only by handing over a sink, so the discard is no longer expressible.
///
/// # Why there is no per-entry reason field
///
/// The obvious next move is a third slot holding a sentence per entry, the way
/// `check_parity/manifest.json` carries a `note`. It was considered and
/// refused: 99 entries were measured in one pass, and the reasons are per
/// FAMILY, not per entry. Writing 99 individual sentences would mean inventing
/// 88 of them, which is the fabrication this same session removed from the
/// parser. The families are stated in the table above, where they are true.
///
/// When an entry is added ONE at a time, by somebody who knows why, a sentence
/// belongs beside it as a comment. That is the case the docstring above asks
/// for, and a comment carries it without requiring the other 98 to lie.
pub(super) const KNOWN_DIVERGENCES: &[(&str, Divergence)] = &[
    ("E202_missing_form_type.md#0", Conflicting),
    ("E202_missing_form_type.md#1", Conflicting),
    // `word@@`: tree-sitter names the repeated `@` run as E203, re2c's lexer
    // cannot form the word at all and reports E321. Both REFUSE the file; they
    // disagree on whether the answer is about the file or about the parser,
    // which is the standing re2c gap recorded in the 0.16.0 known limitations.
    ("E202_missing_form_type.md#2", Conflicting),
    // `#0` since E203 gained a second example: a bare name addresses a
    // single-example spec. `dog@b@c` still diverges (tree-sitter E203, re2c
    // E209 plus E253). The new `#1`, `gumma@c@s:spa`, is deliberately absent:
    // both backends answer E203 there, so the case the at-most-one-suffix
    // ruling actually decided AGREES.
    ("E202_missing_form_type.md#3", Conflicting),
    ("E203.md#0", Conflicting),
    ("E208.md", Conflicting),
    ("E231.md", Conflicting),
    ("E232.md", Conflicting),
    ("E242.md#0", Conflicting),
    // `#1` arrived 2026-09-01 with E242 decided from CST structure: an
    // unmatched opener inside a longer utterance. re2c still answers its
    // generic E321 there, the same Conflicting shape as `#0`.
    ("E242.md#1", Conflicting),
    ("E245.md", Re2cExtra),
    // `#0` since 2026-09-01, when E252, E253, E301, E306 and E307 were
    // rewritten from auto-generated stubs into stated rules and gained a
    // second example each; a bare name addresses a single-example spec. The
    // divergences themselves are unchanged in kind: re2c has not implemented
    // the rewritten rules, and its E321 stands where tree-sitter names one.
    ("E252.md#0", Re2cExtra),
    ("E253.md#0", Re2cIncomplete),
    ("E301.md#0", Conflicting),
    ("E306.md#0", Re2cIncomplete),
    ("E307.md#0", Conflicting),
    // ADJUDICATED 2026-08-11 against real CLAN CHECK, and BOTH SIDES FIXED.
    // The case became visible only when E311's spec stopped being
    // `not_implemented`: it had been skipped, not agreed.
    //
    // `*CHI:\t[: unclosed replacement [* error] .` CHECK reports "Unmatched [
    // found on the tier.(22)", so the outer bracket really is never closed.
    //
    // Was: tree-sitter [E311, E305], re2c [E759]. The first note here reasoned
    // from the codes alone and concluded the OPPOSITE, that the oracle was
    // right; one `clan-run.sh` run settled it. The oracle is authoritative
    // about the EXISTENCE of a divergence, never about which side is correct.
    //
    // Now: tree-sitter [E311] alone, having stopped claiming a terminator was
    // missing on a line that ends with one; re2c [E321], having stopped
    // swallowing a `[` inside a replacement and reporting a wrong reason.
    //
    // Still listed because the codes differ: E311 names the construct, E321
    // says the utterance did not parse. That is the oracle rejecting for a
    // vaguer reason than the canonical parser, which is the acceptable
    // direction; it is NOT silence, which is what it used to be on
    // `word [: a [* b] .`.
    //
    // Evidence: docs/audits/2026-08-11-utterance-initial-annotation-adjudication.md
    // Keyed per CASE since 2026-08-12: the spec gained a second example, so the
    // bare filename no longer identifies which one is meant.
    //
    // #0 (`[:` at utterance start) additionally carries E316 from the
    // whole-tree backstop, which names the same ERROR node E311 already names
    // specifically. Redundant rather than wrong; the specific code is present
    // and first.
    ("E311.md#0", Conflicting),
    // #1 (`hello [: world .`, the same construct AFTER spoken material) is the
    // case that had no example until the typed-traversal migration silently
    // degraded it to E316. It exists so that route is never again covered only
    // by accident.
    ("E311.md#1", Conflicting),
    ("E313.md#0", Conflicting),
    ("E314.md#0", Conflicting),
    ("E315.md#0", Conflicting),
    ("E315.md#1", Re2cSilent),
    ("E316.md#0", Conflicting),
    ("E316.md#1", Conflicting),
    ("E316.md#2", Conflicting),
    ("E316.md#3", Conflicting),
    ("E316.md#4", Conflicting),
    ("E316.md#5", Conflicting),
    ("E316.md#6", Conflicting),
    ("E316.md#7", Re2cSilent),
    ("E316.md#8", Conflicting),
    ("E324.md#0", Conflicting),
    ("E326.md", Conflicting),
    // These four arrived on 2026-08-11 without any parser change: E342_auto.md
    // was marked `not_implemented` by a stale auto-generated stub while its
    // real spec said `implemented`, so this gate had been skipping it. Fixing
    // the status brought four cases into scope, and they diverge in the E600
    // versus E605 way that already dominates the Conflicting family. A ratchet
    // that demands this be acknowledged rather than absorbed is the point.
    ("E330.md#0", Conflicting),
    ("E342.md#0", Re2cIncomplete),
    ("E342.md#1", Conflicting),
    ("E363.md#0", Re2cSilent),
    ("E363.md#1", Conflicting),
    ("E373.md#1", Conflicting),
    ("E375.md#0", Conflicting),
    ("E375.md#1", Re2cSilent),
    ("E376.md", Conflicting),
    ("E404.md", Conflicting),
    ("E503.md", Re2cIncomplete),
    ("E505.md#0", Conflicting),
    ("E505.md#1", Conflicting),
    ("E505.md#2", Conflicting),
    ("E506.md#0", Re2cIncomplete),
    ("E507.md#0", Re2cIncomplete),
    ("E507.md#1", Re2cIncomplete),
    ("E509.md#0", Conflicting),
    ("E512.md#0", Conflicting),
    ("E513.md#0", Re2cIncomplete),
    ("E515.md#0", Conflicting),
    ("E533.md#0", Conflicting),
    ("E550.md", Re2cSilent),
    ("E600.md", Conflicting),
    ("E601.md#0", Conflicting),
    ("E602.md#0", Re2cExtra),
    ("E602.md#1", Re2cSilent),
    ("E709.md", Conflicting),
    ("E710.md", Conflicting),
    ("E747.md", Re2cSilent),
    ("E756.md#0", Re2cIncomplete),
    ("E757.md#1", Re2cSilent),
    ("E757.md#2", Re2cSilent),
    ("E758.md#1", Re2cSilent),
    ("E758.md#2", Re2cSilent),
    ("E758.md#3", Re2cSilent),
    ("E758.md#4", Re2cExtra),
    ("E760.md#0", Conflicting),
    ("E760.md#1", Conflicting),
];
