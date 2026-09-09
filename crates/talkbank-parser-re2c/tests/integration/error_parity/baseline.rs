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
/// # The keys were re-numbered on 2026-09-07, once
///
/// Every key now carries `#<ordinal>`, counting from ONE, matching the
/// observation snapshot's `"example"` field and the generated fixture
/// filenames. Two things changed together: a lone example used to have no
/// suffix at all, so adding a sibling renamed it and silently retired its
/// entry; and the suffix that did exist counted from zero, so `#2` named the
/// case every other artifact calls 3. `SpecLabel` carries the reasoning. The
/// migration shifted 39 keys by one and gave 9 an ordinal they had never had,
/// touching no shape, and the gate itself proved it: a key transformed wrongly
/// shows up as a STALE entry beside a NEW one.
///
/// Labels quoted in the prose BELOW predate this and are in the old scheme.
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
/// **Closed 2026-09-06 by lexer-owned separator provenance.**
/// Historical addition, **2026-08-16, one**: `E756.md#0`, `Re2cIncomplete`.
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
/// 2026-09-06: E503's document without `@UTF8` now agrees. The canonical
/// grammar had discarded the document and added false missing-header errors;
/// re2c's smaller diagnostic set was correct. Both retain the document and
/// report only E503, matching CHECK (69). The paired declaration-present
/// example is clean. The old `Re2cIncomplete` shape did not assign correctness.
///
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
    // E202's missing/invalid/repeated suffix cases and E203#0 were retired
    // together when rich-word recovery retained the complete suffix for
    // semantic validation (2026-09-07).
    ("E208.md#1", Conflicting),
    ("E231.md#1", Conflicting),
    // `#2` arrived 2026-09-07 with the example that first demonstrates E231
    // end to end: a terminator glued to the closing paren, where tree-sitter
    // repairs the missing paren and reaches the rule (E231 plus the E342
    // recovery marker) and re2c answers its generic E321. Same family as `#1`.
    ("E231.md#2", Conflicting),
    ("E232.md#1", Conflicting),
    ("E242.md#1", Conflicting),
    // `#1` arrived 2026-09-01 with E242 decided from CST structure: an
    // unmatched opener inside a longer utterance. re2c still answers its
    // generic E321 there, the same Conflicting shape as `#0`.
    ("E242.md#2", Conflicting),
    ("E245.md#1", Re2cExtra),
    // E251's historical `@s:eng` sample became measured when its status moved
    // from planned to model-only. Tree-sitter reports E255/E342; re2c recovers
    // an empty word and reports E209/E253/E255. Neither emits E251. Retain the
    // sample and expose the existing recovery discrepancy while correcting
    // the model-boundary classification; this is not a parser parity claim.
    ("E342.md#3", Conflicting),
    // `#0` since 2026-09-01, when E252, E253, E301, E306 and E307 were
    // rewritten from auto-generated stubs into stated rules and gained a
    // second example each; a bare name addresses a single-example spec. The
    // divergences themselves are unchanged in kind: re2c has not implemented
    // the rewritten rules, and its E321 stands where tree-sitter names one.
    ("E252.md#1", Re2cExtra),
    ("E253.md#1", Re2cIncomplete),
    ("E301.md#1", Conflicting),
    ("E306.md#1", Re2cIncomplete),
    // `E306#2` (`*CHI:` with nothing after the colon, not even the tab):
    // tree-sitter [E306], which is what the spec claims, from the file-level
    // analyzer reading the ERROR text that ends at its colon; re2c [E321].
    // Same shape as E360#1: re2c stops at the malformed prefix and names the
    // line, not the rule.
    ("E306.md#2", Conflicting),
    ("E307.md#1", Conflicting),
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
    ("E311.md#1", Conflicting),
    // #1 (`hello [: world .`, the same construct AFTER spoken material) is the
    // case that had no example until the typed-traversal migration silently
    // degraded it to E316. It exists so that route is never again covered only
    // by accident.
    ("E311.md#2", Conflicting),
    ("E313.md#1", Conflicting),
    // `#3` arrived 2026-09-07 with E313's first working example. tree-sitter
    // names the rule, re2c answers E321: the generic-code family again.
    ("E313.md#3", Conflicting),
    ("E314.md#1", Conflicting),
    ("E315.md#1", Conflicting),
    ("E316.md#1", Conflicting),
    ("E316.md#2", Conflicting),
    ("E316.md#3", Conflicting),
    ("E316.md#4", Conflicting),
    ("E316.md#5", Conflicting),
    ("E316.md#6", Conflicting),
    // ADDED 2026-09-08: a content-bearing recovery node inside a `%mor` tier
    // is E702 at any depth in tree-sitter (one reporter walks the whole tier
    // in the tier's words); re2c's `%mor` recovery reports E316. The same
    // family as E702.md#2 below.
    ("E316.md#7", Conflicting),
    ("E316.md#8", Conflicting),
    ("E316.md#9", Conflicting),
    // E320 entered this suite on 2026-09-08 and had never been measured here.
    // Its registry status said `not_implemented`, which excuses a code from the
    // spec corpus this gate runs over as well as from the demonstration gate,
    // so one wrong word kept the code out of BOTH measurements while the parser
    // emitted it the whole time. tree-sitter reports E316 alongside E539 for a
    // header line it cannot parse; re2c reports only E539, so it is missing the
    // recovery diagnostic rather than disagreeing about the rule.
    ("E320.md#1", Re2cIncomplete),
    ("E324.md#1", Conflicting),
    // These four arrived on 2026-08-11 without any parser change: E342_auto.md
    // was marked `not_implemented` by a stale auto-generated stub while its
    // real spec said `implemented`, so this gate had been skipping it. Fixing
    // the status brought four cases into scope, and they diverge in the E600
    // versus E605 way that already dominates the Conflicting family. A ratchet
    // that demands this be acknowledged rather than absorbed is the point.
    ("E330.md#1", Conflicting),
    // THE ONLY Re2cSilent ENTRY IN THIS BASELINE, and the class the header
    // calls critical: re2c reports NOTHING on invalid input. Found 2026-09-07
    // by writing E330's first reachable example, a dependent-tier body opening
    // with a bare bullet delimiter. tree-sitter reports E316 and E330; re2c's
    // tier lexer says nothing at all. Its sibling `E710.md#2` arrived the same
    // day and was closed the next (see the note below the list); this one ships
    // recorded rather than
    // fixed because the fix is in re2c's tier LEXING rather than in a lowering,
    // and it is the first thing to look at in this file.
    ("E330.md#3", Re2cSilent),
    // `E342.md#1` (`<I don't> &-uh ...`, a group with no marker) was
    // Re2cIncomplete here until 2026-09-09: tree-sitter decoded the MISSING
    // marker's kind into a `Full` retrace and then reported E757 on the
    // phantom marker. The decoder skips MISSING nodes now, both backends
    // build the bare group, and the entry retired.
    ("E342.md#2", Conflicting),
    // Added 2026-09-08 from a mutation sweep over `%mor`: an empty lemma before
    // a clitic (`pron|~aux|be`) and an empty feature value (`be-Fin--Ind`).
    // Tree-sitter fills the required `mor_lemma` / `mor_feature_value` with
    // a MISSING placeholder and names it (E342); re2c rejects the whole tier
    // as the generic E316. The spec exists now, so this is a re2c gap.
    ("E342.md#4", Conflicting),
    ("E342.md#5", Conflicting),
    // Same sweep, headers: a doubled comma in `@Participants`. Tree-sitter
    // walks the list and names the fault (E506); re2c rejects the header as
    // the generic E316.
    ("E506.md#3", Conflicting),
    ("E363.md#2", Conflicting),
    ("E373.md#2", Conflicting),
    ("E375.md#1", Conflicting),
    ("E376.md#1", Conflicting),
    // ADDED 2026-09-08, and NEWLY VISIBLE rather than newly created. Each of
    // E312, E331 and E360 was registered `not_implemented`, which excuses a
    // spec from this harness entirely, so these cases had never been compared
    // on either backend. Correcting the status is what exposed them.
    //
    // `E312#1` (`word [= comment .`): tree-sitter [E342, E375] vs re2c [E321].
    // Both reject and neither is silent; they disagree about WHICH rule the
    // unclosed bracket breaks, which is a genuine conflict.
    ("E312.md#1", Conflicting),
    // `E312#2` (`<hello[,> [!] .`), the example that demonstrates the code:
    // tree-sitter [E312] vs re2c [E321]. The bracket analyser tree-sitter runs
    // over its recovery node has no re2c counterpart, so this is an oracle gap
    // wearing a conflict's clothes; recorded as Conflicting because re2c does
    // emit something rather than nothing.
    ("E312.md#2", Conflicting),
    // `E331#1` (a `%mor` word with no stem): tree-sitter [E316, E600, E724] vs
    // re2c [E321]. The spec's claim (subsumed by E316) is met by tree-sitter
    // and not by re2c, which reaches a coarser verdict on the same input.
    // ADDED 2026-09-08, newly VISIBLE rather than newly created: E319 and E321
    // were registered `not_implemented`, which excuses a spec from this
    // harness, so neither had ever been compared.
    //
    // `E319#1` (a `%com` tier with no main tier before it): tree-sitter [E602]
    // vs re2c [E309, E319]. The spec claims E602 and tree-sitter meets it.
    // re2c names the orphan tier directly, with two codes the canonical parser
    // has no producer for; that is the whole reason both codes' statuses could
    // not be told apart from unimplemented until this week.
    ("E319.md#1", Conflicting),
    // ADDED 2026-09-08, newly visible for the same reason as E319#1.
    // `E321#1`: tree-sitter [E342, E375] vs re2c [E321]. re2c reaches its
    // generic unparsable-utterance verdict where tree-sitter's recovery names
    // a missing element and an annotation parse error.
    ("E321.md#1", Conflicting),
    ("E331.md#1", Conflicting),
    // `E360#1` (the deprecated trailing-dash skip bullet): tree-sitter [E316],
    // which is what the spec claims, vs re2c [E321]. Same shape as E331#1.
    ("E360.md#1", Conflicting),
    // `E360#3` (timestamps that overflow the model's integer type), the
    // example that demonstrates the code: tree-sitter [E360] vs re2c [E360,
    // E362, E752]. Both now name the rule; re2c adds two.
    //
    // Where the two come from, because the first draft of this entry got it
    // wrong and the wrong version is the instructive one. It said re2c "reads
    // the overflowing digits as a backwards range" and called the E362 an
    // arithmetic wrap. Measured, re2c reports "start time (0ms) must be less
    // than end time (0ms)": both timestamps are ZERO, not wrapped, from an
    // `unwrap_or(0)` in `convert/items.rs` whose own doc comment asserted the
    // case "cannot be reached by any well-formed shape". This example is that
    // shape. So E362 and E752 are a FABRICATED VALUE speaking, and they are
    // recorded here rather than explained away.
    //
    // What changed in the same commit: the rule is now reported by
    // `report_unrepresentable_bullet_times` in re2c's `parser/file.rs`, a
    // token-level scan where the raw digits still exist, so a user is told
    // E360 instead of a statement about ordering the input does not support.
    // Removing the fabrication itself needs the converter to become fallible
    // through five call sites that hold no error sink.
    ("E360.md#3", Re2cExtra),
    ("E404.md#1", Conflicting),
    ("E505.md#1", Conflicting),
    ("E505.md#2", Conflicting),
    ("E505.md#3", Conflicting),
    ("E506.md#1", Re2cIncomplete),
    ("E507.md#1", Re2cIncomplete),
    ("E507.md#2", Re2cIncomplete),
    ("E509.md#1", Conflicting),
    ("E512.md#1", Conflicting),
    ("E513.md#1", Re2cIncomplete),
    ("E515.md#1", Conflicting),
    ("E533.md#1", Conflicting),
    ("E600.md#1", Conflicting),
    ("E601.md#1", Conflicting),
    ("E602.md#1", Re2cExtra),
    // ADDED 2026-09-08 with the example that created it, which is the honest
    // order: E702 was registered `not_implemented` and the prose in its spec
    // said "there is no E702 today". Both were false, measured, and adding an
    // example that demonstrates the code is what exposed them. re2c has no
    // equivalent classifier: the tree-sitter producer is an ERROR-NODE analyser
    // on the recovery path, and re2c's `%mor` recovery does not surface one, so
    // it reports E316 and E600 and stops. A real gap in the oracle backend
    // rather than a disagreement about CHAT, and re2c is the lower priority of
    // the two parsers.
    ("E702.md#1", Conflicting),
    // Re2cIncomplete until 2026-09-08 (tree-sitter reported E316 beside E702
    // there); now Conflicting, with E702.md#1 and E711.md#1, for the reason
    // above: tree-sitter says E702 where re2c says E316.
    ("E702.md#2", Conflicting),
    ("E711.md#1", Conflicting),
    // ADDED 2026-09-08, newly visible for the same reason as the E3xx block
    // above. E708's own example has a `%gra` tier with no `%mor`, so
    // tree-sitter reports [E604] and re2c reports [E316, E605]. Both reject;
    // they name different things about the same missing tier. That example's
    // CLAIM was corrected in the same change, from a `violates E708` it never
    // met to the `subsumed_by E604` it does.
    ("E708.md#1", Conflicting),
    ("E710.md#1", Conflicting),
    // WAS `Re2cSilent` for about an hour on 2026-09-08, and this entry is kept
    // with its history because the history is the finding. `E710.md#1` is the
    // ERROR-node case an earlier classifier claims; what follows is about its
    // SIBLING, `E710.md#2`, whose head is a digit run overflowing `usize`. That
    // entry, and `E709.md#1`, were RETIRED the same day and their account is
    // below the list rather than beside a row that is no longer here.
    //
    // re2c reported NOTHING on it, and `Silent` understated what it did:
    // `convert/text_tiers.rs`'s `From<&GraRelationParsed>` was infallible and
    // wrote `r.head.parse().unwrap_or(0)`, so the unrepresentable head became
    // head 0, the ROOT attachment. The oracle backend was not missing a
    // diagnostic, it was fabricating a well formed dependency tree out of
    // invalid input, and this gate could see only the missing diagnostic. The
    // lowering takes the sink now (the same change this file's header records
    // for the participant join, E522/E523/E524, 2026-08-15, applied to the
    // dependent-tier path) and rejects the relation with E710, E709 or E708,
    // code for code with the canonical parser.
];

// ---------------------------------------------------------------------------
// Retired 2026-09-08: `E710.md#2` and `E709.md#1`
// ---------------------------------------------------------------------------
//
// Both were one dropped `%gra` relation read as a second fault in the
// transcript. tree-sitter added E600 and E722; re2c added E720; the spec
// expects E710 for the first case and E709 for the second. Two defects, both
// in this repository rather than in the input.
//
// re2c dropped the relation and recorded nothing, so alignment ran against a
// tier one relation short and blamed the transcript for a difference its own
// lowering had made (E720). It records the loss now, on the tier as a
// `GraCompleteness` and on the utterance as a `%gra` taint.
//
// And the model ran its graph-structure rules on the shortened tier. Two
// things were wrong there and only the first was suspected: the suppression it
// had was keyed on ALIGNMENT diagnostics, and tainting a tier is precisely
// what stops those being produced, so the two mechanisms cancelled; and the
// root count excluded a root at `index == relations.len()` to skip a
// terminator, so the one surviving relation `1|0|ROOT` was excluded as though
// it were one. `WholeGra` in `talkbank-model` carries the full account, and
// that exclusion is deleted rather than repaired.
//
// Both backends now report the fault and nothing else: E710 (or E709) plus
// E600, which says an alignment was skipped rather than inventing its result.
