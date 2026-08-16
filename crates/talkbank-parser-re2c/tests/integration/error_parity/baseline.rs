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
/// # What was here when the gate was first closed, 2026-08-09
///
/// 99 of 283 cases, so the backends agree on **184/283 (65.0%)**. The number
/// the old audit printed, 214/283 (75.6%), was answering the other question
/// (does each backend satisfy the spec) and read as though it answered this
/// one. Nothing regressed between the two figures; only the question did.
///
/// | shape | count | what the work is |
/// |---|---|---|
/// | `Conflicting` | 61 | each names a code the other does not. Dominated by re2c reporting the generic `E321` where tree-sitter names the specific rule, and by `E600` versus `E605` on `%mor`. |
/// | `Re2cIncomplete` | 23 | tree-sitter catches strictly more. A rule re2c has not implemented. |
/// | `Re2cSilent` | 11 | re2c reports NOTHING on invalid input. The critical class, and the only one the previous audit named. |
/// | `Re2cExtra` | 4 | re2c reports everything tree-sitter does and more. Over-reporting, or tree-sitter under-reporting: adjudicate before assuming which. |
///
/// The families matter more than the entries: `E321` alone accounts for over
/// twenty of the conflicts, so one fix should retire a large block of this
/// list at once, and that is the order to work in.
///
/// # Added since
///
/// **2026-08-16, one**: `E756_empty_dependent_tier.md#0`, `Re2cIncomplete`.
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
/// **2026-08-15, three at once**: `E511_auto.md`, `E523_auto.md` and
/// `E524_auto.md`. All three were the same defect. The re2c backend lowered a
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
    ("E202_auto.md", Conflicting),
    ("E202_missing_form_type.md#0", Conflicting),
    ("E202_missing_form_type.md#1", Conflicting),
    ("E203_auto.md", Conflicting),
    ("E207_auto.md", Conflicting),
    ("E208_auto.md", Conflicting),
    ("E231_auto.md", Conflicting),
    ("E232_auto.md", Conflicting),
    ("E233_auto.md", Conflicting),
    ("E242_auto.md", Conflicting),
    ("E243_auto.md", Conflicting),
    ("E245_auto.md", Re2cExtra),
    ("E252_auto.md", Re2cExtra),
    ("E253_auto.md", Re2cIncomplete),
    ("E258_auto.md", Re2cSilent),
    ("E301_auto.md", Conflicting),
    ("E306_auto.md", Re2cIncomplete),
    ("E307_auto.md", Conflicting),
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
    ("E311_auto.md#0", Conflicting),
    // #1 (`hello [: world .`, the same construct AFTER spoken material) is the
    // case that had no example until the typed-traversal migration silently
    // degraded it to E316. It exists so that route is never again covered only
    // by accident.
    ("E311_auto.md#1", Conflicting),
    ("E313_auto.md", Conflicting),
    ("E314_auto.md", Conflicting),
    ("E315_auto.md", Conflicting),
    ("E316_angle_bracket_in_mor_stem.md#0", Re2cSilent),
    ("E316_angle_bracket_in_mor_stem.md#1", Conflicting),
    ("E316_auto.md#0", Conflicting),
    ("E316_auto.md#1", Conflicting),
    ("E316_auto.md#2", Conflicting),
    ("E316_auto.md#3", Conflicting),
    ("E316_auto.md#4", Conflicting),
    ("E316_auto.md#6", Conflicting),
    ("E316_auto.md#7", Conflicting),
    ("E324_auto.md", Conflicting),
    ("E326_auto.md", Conflicting),
    ("E330_auto.md", Conflicting),
    // These four arrived on 2026-08-11 without any parser change: E342_auto.md
    // was marked `not_implemented` by a stale auto-generated stub while its
    // real spec said `implemented`, so this gate had been skipping it. Fixing
    // the status brought four cases into scope, and they diverge in the E600
    // versus E605 way that already dominates the Conflicting family. A ratchet
    // that demands this be acknowledged rather than absorbed is the point.
    ("E342_auto.md#0", Conflicting),
    ("E342_auto.md#1", Conflicting),
    ("E342_auto.md#2", Conflicting),
    ("E342_auto.md#3", Conflicting),
    ("E342_group_without_annotation.md", Re2cIncomplete),
    ("E358_auto.md", Conflicting),
    ("E359_auto.md", Conflicting),
    ("E361_auto.md", Conflicting),
    ("E363_auto.md", Conflicting),
    ("E367_auto.md", Conflicting),
    ("E368_auto.md", Conflicting),
    ("E373_auto.md#1", Conflicting),
    ("E375_auto.md", Conflicting),
    ("E375_replacement_needs_preceding_space.md", Re2cSilent),
    ("E376_auto.md", Conflicting),
    ("E382_auto.md#0", Conflicting),
    ("E382_auto.md#1", Conflicting),
    ("E382_auto.md#2", Conflicting),
    ("E404_auto.md", Conflicting),
    ("E501_auto.md#1", Re2cIncomplete),
    ("E502_wor_cascade_regression.md#0", Conflicting),
    ("E502_wor_cascade_regression.md#1", Conflicting),
    ("E503_auto.md", Re2cIncomplete),
    ("E505_auto.md#0", Conflicting),
    ("E505_auto.md#1", Conflicting),
    ("E505_auto.md#2", Conflicting),
    ("E506_auto.md", Re2cIncomplete),
    ("E507_auto.md", Re2cIncomplete),
    ("E508_auto.md", Conflicting),
    ("E509_auto.md", Conflicting),
    ("E510_auto.md", Conflicting),
    ("E512_auto.md", Re2cIncomplete),
    ("E513_auto.md", Re2cIncomplete),
    ("E515_auto.md", Conflicting),
    ("E518_auto.md#0", Re2cIncomplete),
    ("E518_auto.md#1", Re2cIncomplete),
    ("E518_auto.md#2", Re2cIncomplete),
    ("E518_auto.md#3", Re2cIncomplete),
    ("E518_auto.md#5", Re2cIncomplete),
    ("E522_auto.md#0", Re2cIncomplete),
    ("E525_auto.md#0", Re2cIncomplete),
    ("E525_auto.md#1", Re2cIncomplete),
    ("E525_auto.md#3", Conflicting),
    ("E525_auto.md#4", Conflicting),
    ("E526_auto.md", Conflicting),
    ("E527_auto.md", Conflicting),
    ("E529_auto.md", Re2cIncomplete),
    ("E530_auto.md", Re2cIncomplete),
    ("E533_auto.md", Conflicting),
    ("E550_trailing_comma_participants.md", Re2cSilent),
    ("E600_auto.md", Conflicting),
    ("E601_auto.md", Conflicting),
    ("E602_auto.md", Re2cExtra),
    ("E709_auto.md", Conflicting),
    ("E710_auto.md", Conflicting),
    ("E747_blank_line_not_allowed.md", Re2cSilent),
    ("E756_empty_dependent_tier.md#0", Re2cIncomplete),
    ("E757_code_glued_to_following_content.md#1", Re2cSilent),
    ("E757_code_glued_to_following_content.md#2", Re2cSilent),
    ("E758_leading_space_on_main_tier.md#1", Re2cSilent),
    ("E758_leading_space_on_main_tier.md#2", Re2cSilent),
    ("E758_leading_space_on_main_tier.md#3", Re2cSilent),
    ("E758_leading_space_on_main_tier.md#4", Re2cExtra),
    ("E760_mor_item_empty_pos.md#0", Conflicting),
    ("E760_mor_item_empty_pos.md#1", Conflicting),
];
