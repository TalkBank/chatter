// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! Characterization tests for the `%wor` dependent tier migrated onto the
//! generated `extract_wor_dependent_tier` / `extract_wor_tier_body` typed visitor
//! (Task 4f).
//!
//! These tests pin the OBSERVABLE behaviour of the `%wor` tier at the real parser
//! boundary (`parse_chat_file_streaming` -> `ChatFile` + collected diagnostics).
//! The migration replaces the hand-walk in `tier_parsers/wor.rs`
//! (`parse_wor_tier`): the `child.kind() == WOR_TIER_BODY` locate loop plus the
//! `while body.children` + `match child.kind()` item iteration (LANGCODE /
//! WOR_WORD_ITEM / BULLET / COMMA|TAG_MARKER|VOCATIVE_MARKER / TERMINATOR /
//! `terminator_from_node_kind` guard / WHITESPACES / NEWLINE), with exhaustive
//! typed `NodeSlot` matching over:
//!
//! - the dep-tier body slot (`extract_wor_dependent_tier.child_2`), replacing the
//!   `WOR_TIER_BODY` locate loop,
//! - the `wor_tier_body` fields (`extract_wor_tier_body`): optional `langcode`
//!   (`child_0`), the `Vec<WorTierBodyElement1>` item repeat (`child_1`), each
//!   element's `WorTierBodyElement1ItemChoice0` variant
//!   (`WorWordItem`/`Bullet`/`Comma`/`TagMarker`/`VocativeMarker`), the optional
//!   `terminator` supertype (`child_2`, previously UNCONSUMED, now decoded via the
//!   shared generic `terminator_from_new_choice` / `NewTerminatorChoice` trait,
//!   the same path `utterance_end` uses), and the required `newline` (`child_3`).
//!
//! The migration is BEHAVIOUR-PRESERVING: the parsed `%wor` model (langcode,
//! words, bullets with word/bullet pairing, separators, terminator) and the
//! recovery diagnostics must not change. Every value here was captured by RUNNING
//! the pre-migration parser (base HEAD `db43211`). The tests PASS on the current
//! code and MUST STAY GREEN, byte-identical, after the migration.
//!
//! # Reachability note (why the empty-body case looks the way it does)
//!
//! `parse_wor_tier` is only invoked from the dispatch when the tier node has NO
//! tree-sitter error. A structurally broken `%wor` line (empty body) makes the
//! tier node `has_error()`: the required body content is inserted by tree-sitter
//! recovery as MISSING nodes, surfaced by the whole-tree recovery backstop as
//! `MissingRequiredElement` (E342) diagnostics, and NO wor tier is attached. The
//! internal "no `wor_tier_body` child found -> return EMPTY tier silently" partial
//! inside `parse_wor_tier` is therefore UNREACHABLE from the boundary; the
//! migration preserves it for exhaustiveness. The reachable valid path is pinned
//! by the valid test; the empty-body reality is pinned by the malformed test so
//! the migration cannot accidentally start attaching a tier or changing these
//! diagnostics.

use talkbank_model::ErrorCollector;
use talkbank_model::model::dependent_tier::WorItem;
use talkbank_model::model::{Bullet, DependentTier, Line, Terminator};
use talkbank_parser::TreeSitterParser;

/// A VALID `%wor` line exercising every field arm at once:
/// - an optional `langcode` (`[- eng]`),
/// - MULTIPLE word items (`one`, `two`, `three`),
/// - a timing `bullet` after `one` and after `three` (exercises the word/bullet
///   PAIRING, and `two` having NO following bullet),
/// - a `comma` marker separator,
/// - a `period` terminator (the previously-unconsumed `child_2` terminator slot).
const VALID_WOR: &str = "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\tone two three .\n%wor:\t[- eng] one \u{0015}0_120\u{0015} two , three \u{0015}120_260\u{0015} .\n@End\n";

/// A `%wor:` line with an EMPTY body (tab then newline, no items). The
/// `wor_tier_body` grammar rule is `optional(langcode) repeat(item) optional(term)
/// newline`, so a body of just a newline is a VALID (empty) `wor_tier_body`: the
/// tier node is error-free, `parse_wor_tier` IS called, and it attaches an empty
/// `WorTier` (no langcode, no items, no terminator). This is the reachable
/// empty-tier partial the migration must preserve byte-identically.
const EMPTY_BODY_WOR: &str = "@UTF8\n@Begin\n*CHI:\thi .\n%wor:\t\n@End\n";

/// A stable structural rendering of one `WorItem` for assertions.
#[derive(Debug, PartialEq)]
enum WorItemRepr {
    /// `WorItem::Word` rendered as its cleaned text and optional timing bullet.
    Word {
        /// The word's display text (`cleaned_text`).
        text: String,
        /// The paired inline timing bullet, if any.
        bullet: Option<Bullet>,
    },
    /// `WorItem::Separator` rendered as its literal marker text.
    Separator(String),
}

/// Render one `WorItem` into the stable [`WorItemRepr`] used by assertions.
fn item_repr(item: &WorItem) -> WorItemRepr {
    match item {
        WorItem::Word(word) => WorItemRepr::Word {
            text: word.cleaned_text().to_string(),
            bullet: word.inline_bullet.clone(),
        },
        WorItem::Separator { text, .. } => WorItemRepr::Separator(text.clone()),
    }
}

/// Render a [`Terminator`] to a stable discriminant label (span-independent).
///
/// Exhaustive over all 13 variants (no `_` catch-all) so a future terminator
/// variant is a compile error here rather than a silently mislabelled assertion.
fn term_label(term: &Terminator) -> &'static str {
    match term {
        Terminator::Period { .. } => "period",
        Terminator::Question { .. } => "question",
        Terminator::Exclamation { .. } => "exclamation",
        Terminator::TrailingOff { .. } => "trailing_off",
        Terminator::Interruption { .. } => "interruption",
        Terminator::SelfInterruption { .. } => "self_interruption",
        Terminator::InterruptedQuestion { .. } => "interrupted_question",
        Terminator::BrokenQuestion { .. } => "broken_question",
        Terminator::QuotedNewLine { .. } => "quoted_new_line",
        Terminator::QuotedPeriodSimple { .. } => "quoted_period_simple",
        Terminator::SelfInterruptedQuestion { .. } => "self_interrupted_question",
        Terminator::TrailingOffQuestion { .. } => "trailing_off_question",
        Terminator::BreakForCoding { .. } => "break_for_coding",
    }
}

/// Outcome of parsing a `%wor` fixture at the streaming boundary.
struct WorParse {
    /// The optional langcode rendered as its string form (e.g. `"eng"`).
    language_code: Option<String>,
    /// The ordered rendered items of the (first) `%wor` tier.
    items: Vec<WorItemRepr>,
    /// The terminator discriminant label, if any.
    terminator: Option<&'static str>,
    /// Whether at least one `%wor` tier was attached.
    saw_wor_tier: bool,
    /// Every collected diagnostic as `(code, start, end, message)`.
    diags: Vec<(String, u32, u32, String)>,
}

/// Parse `input` at the streaming boundary, collecting `%wor`-tier fields and
/// diagnostics.
fn parse_wor(input: &str) -> WorParse {
    let parser = TreeSitterParser::new().expect("tree-sitter grammar should load");
    let errors = ErrorCollector::new();
    let chat = parser.parse_chat_file_streaming(input, &errors);

    let mut language_code = None;
    let mut items = Vec::new();
    let mut terminator = None;
    let mut saw_wor_tier = false;
    for line in &chat.lines.0 {
        if let Line::Utterance(u) = line {
            for dt in &u.dependent_tiers {
                if let DependentTier::Wor(t) = dt {
                    saw_wor_tier = true;
                    language_code = t.language_code.as_ref().map(|lc| lc.to_string());
                    items.extend(t.items.iter().map(item_repr));
                    terminator = t.terminator.as_ref().map(term_label);
                }
            }
        }
    }

    let diags = errors
        .to_vec()
        .into_iter()
        .map(|d| {
            (
                d.code.as_str().to_string(),
                d.location.span.start,
                d.location.span.end,
                d.message,
            )
        })
        .collect();

    WorParse {
        language_code,
        items,
        terminator,
        saw_wor_tier,
        diags,
    }
}

/// VALID `%wor`: the full-coverage line must parse to the exact ordered model
/// (langcode, three words with the correct bullet pairing, one comma separator,
/// and a period terminator) with ZERO diagnostics. This is the reachable path the
/// migration must keep byte-identical.
#[test]
fn valid_wor_tier_parses_byte_identical_with_zero_diagnostics() {
    let parsed = parse_wor(VALID_WOR);

    assert!(
        parsed.saw_wor_tier,
        "a valid %wor line must attach a wor tier"
    );
    assert_eq!(
        parsed.language_code,
        Some("eng".to_string()),
        "the `[- eng]` langcode must decode to `eng`"
    );
    assert_eq!(
        parsed.items,
        vec![
            WorItemRepr::Word {
                text: "one".to_string(),
                bullet: Some(Bullet::new(0, 120)),
            },
            WorItemRepr::Word {
                text: "two".to_string(),
                bullet: None,
            },
            WorItemRepr::Separator(",".to_string()),
            WorItemRepr::Word {
                text: "three".to_string(),
                bullet: Some(Bullet::new(120, 260)),
            },
        ],
        "every %wor item must decode in order, with each bullet paired to its preceding word"
    );
    assert_eq!(
        parsed.terminator,
        Some("period"),
        "the trailing `.` must decode to a Period terminator via the terminator slot"
    );
    assert!(
        parsed.diags.is_empty(),
        "a valid %wor line must produce zero diagnostics, got: {:?}",
        parsed.diags
    );
}

/// EMPTY-BODY (reachable partial): an empty-body `%wor:` line is a VALID empty
/// `wor_tier_body`, so `parse_wor_tier` attaches an empty `WorTier` (no langcode,
/// no items, no terminator) and emits ZERO diagnostics. Pins that reality so the
/// migration's `Present`/`Missing` body arm keeps yielding an empty tier and does
/// not accidentally start rejecting or emitting diagnostics.
#[test]
fn empty_body_wor_tier_attaches_an_empty_tier_with_zero_diagnostics() {
    let parsed = parse_wor(EMPTY_BODY_WOR);

    assert!(
        parsed.saw_wor_tier,
        "an empty-body %wor line attaches an (empty) wor tier: the empty body is valid"
    );
    assert!(
        parsed.items.is_empty(),
        "an empty %wor body has no items, got: {:?}",
        parsed.items
    );
    assert_eq!(
        parsed.terminator, None,
        "an empty %wor body has no terminator"
    );
    assert_eq!(
        parsed.language_code, None,
        "an empty %wor body has no langcode"
    );
    assert!(
        parsed.diags.is_empty(),
        "an empty-body %wor line must produce zero diagnostics, got: {:?}",
        parsed.diags
    );
}
