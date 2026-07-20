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

//! Characterization tests for the `%sin` dependent tier migrated onto the
//! generated `extract_sin_dependent_tier` / `extract_sin_groups` /
//! `extract_sin_group` typed visitor (Task 4e).
//!
//! These tests pin the OBSERVABLE behaviour of the `%sin` tier at the real
//! parser boundary (`parse_chat_file_streaming` -> `ChatFile` + collected
//! diagnostics). The migration replaces two hand-walks:
//!
//! - `tier_parsers/sin/parse.rs` (`parse_sin_tier`): the
//!   `child.kind() == SIN_GROUPS` locate loop plus the
//!   `while sin_groups.child(idx)` + `match child.kind()` group iteration, with
//!   exhaustive typed `NodeSlot` matching over the dep-tier body slot (`child_2`)
//!   and each `sin_groups` group slot.
//! - `tier_parsers/sin/groups.rs` (`extract_sin_group_items` and its
//!   `extract_sin_grouped_content_tokens` sub-fn): the `node.child(0).kind()` /
//!   `node.child(1)` interior walk, with the typed `SinGroupChoice` enum
//!   (`SinWord` flat-token alt vs `SinBeginGroup` bracketed `seq` alt) emitted by
//!   the 4d generator classifier.
//!
//! The migration is BEHAVIOUR-PRESERVING: the parsed gesture/sign model and the
//! recovery diagnostics must not change. Every value here was captured by RUNNING
//! the pre-migration parser (base HEAD `6527dd1`), not guessed. The tests PASS on
//! the current code and MUST STAY GREEN, byte-identical, after the migration.
//!
//! # Reachability note (why the empty-body case looks the way it does)
//!
//! `parse_sin_tier` is only invoked from the dispatch when the tier node has NO
//! tree-sitter error. A structurally broken `%sin:` line (empty body) makes the
//! tier node `has_error()`: the required `sin_word` (whose first alternative
//! `zero` is what tree-sitter inserts) is recovered as a MISSING node, surfaced
//! by the whole-tree recovery backstop as two `MissingRequiredElement` (E342)
//! diagnostics, and NO sin tier is attached.
//! The "no `sin_groups` child found -> return EMPTY tier silently" partial inside
//! `parse_sin_tier` is therefore UNREACHABLE from the boundary. The reachable
//! valid path (flat tokens AND bracketed groups) is pinned by the valid test; the
//! empty-body reality is pinned by
//! `empty_body_sin_tier_is_handled_upstream_without_a_sin_tier` so the migration
//! cannot accidentally start attaching an empty tier or changing these
//! diagnostics.

use talkbank_model::ErrorCollector;
use talkbank_model::model::{DependentTier, Line, SinItem};
use talkbank_parser::TreeSitterParser;

/// The real `%sin` snapshot fixture exercising BOTH `sin_group` interior
/// alternatives: three flat gesture tokens (`g:toy:dpoint`, `b`, `e`, the
/// `SinWord` alternative) interleaved with one bracketed group (`〔c d〕`, the
/// `SinBeginGroup` seq alternative). Its `%sin` body is
/// `g:toy:dpoint b 〔c d〕 e`.
const VALID_SIN_GROUPS: &str = include_str!("../../../corpus/reference/annotation/groups-sign.cha");

/// A `%sin:` line with an EMPTY body (tab then newline, no groups). This makes
/// the tier node `has_error()`, so it is handled by the upstream error path and
/// NO sin tier is attached; `parse_sin_tier` is never called.
const EMPTY_BODY_SIN: &str = "@UTF8\n@Begin\n*CHI:\thi .\n%sin:\t\n@End\n";

/// A stable structural rendering of one `SinItem` for assertions: a flat token or
/// a bracketed group of tokens.
#[derive(Debug, PartialEq, Eq)]
enum SinItemRepr {
    /// `SinItem::Token` rendered as its token text.
    Token(String),
    /// `SinItem::SinGroup` rendered as its ordered group-token texts.
    Group(Vec<String>),
}

/// Render one `SinItem` into the stable [`SinItemRepr`] used by assertions.
fn item_repr(item: &SinItem) -> SinItemRepr {
    match item {
        SinItem::Token(token) => SinItemRepr::Token(token.as_ref().to_string()),
        SinItem::SinGroup(gestures) => {
            SinItemRepr::Group(gestures.0.iter().map(|t| t.as_ref().to_string()).collect())
        }
    }
}

/// Outcome of parsing a `%sin` fixture at the streaming boundary: the rendered
/// items of every `%sin` tier, whether at least one such tier was attached, and
/// every collected diagnostic as `(code, start, end, message)`.
struct SinParse {
    items: Vec<SinItemRepr>,
    saw_sin_tier: bool,
    diags: Vec<(String, u32, u32, String)>,
}

/// Parse `input` at the streaming boundary, collecting `%sin`-tier items and
/// diagnostics.
fn parse_sin(input: &str) -> SinParse {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let chat = parser.parse_chat_file_streaming(input, &errors);

    let mut items = Vec::new();
    let mut saw_sin_tier = false;
    for line in &chat.lines.0 {
        if let Line::Utterance(u) = line {
            for dt in &u.dependent_tiers {
                if let DependentTier::Sin(t) = &dt.tier {
                    saw_sin_tier = true;
                    items.extend(t.items.iter().map(item_repr));
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

    SinParse {
        items,
        saw_sin_tier,
        diags,
    }
}

/// VALID `%sin`: the real `groups-sign.cha` fixture must parse to the exact
/// ordered item model and produce ZERO diagnostics. Pins the
/// `NodeSlot::Present(sin_groups)` -> `extract_sin_groups` ->
/// `NodeSlot::Present(sin_group)` -> `extract_sin_group` ->
/// `SinGroupChoice::{SinWord, SinBeginGroup}` path that valid CHAT always takes,
/// including the bracketed `〔c d〕` group decoded as a `SinItem::SinGroup`.
#[test]
fn valid_sin_groups_parses_byte_identical_with_zero_diagnostics() {
    let parsed = parse_sin(VALID_SIN_GROUPS);

    assert!(
        parsed.saw_sin_tier,
        "a valid %sin line must attach a sin tier"
    );
    assert_eq!(
        parsed.items,
        vec![
            SinItemRepr::Token("g:toy:dpoint".to_string()),
            SinItemRepr::Token("b".to_string()),
            SinItemRepr::Group(vec!["c".to_string(), "d".to_string()]),
            SinItemRepr::Token("e".to_string()),
        ],
        "every %sin token/group must decode in order, groups as SinItem::SinGroup"
    );
    assert!(
        parsed.diags.is_empty(),
        "a valid %sin fixture must produce zero diagnostics, got: {:?}",
        parsed.diags
    );
}

/// MALFORMED (upstream): an empty-body `%sin:` line makes the tier node
/// `has_error()` (the required `sin_word`, via its first alternative `zero`, is
/// recovered as a MISSING node), so the
/// whole-tree recovery backstop surfaces two `MissingRequiredElement` (E342)
/// diagnostics and NO sin tier is attached; `parse_sin_tier` is never called and
/// its internal "return empty tier silently" partial is unreachable from the
/// boundary. Pins that reality so the migration cannot accidentally start
/// attaching a sin tier or changing these diagnostics.
#[test]
fn empty_body_sin_tier_is_handled_upstream_without_a_sin_tier() {
    let parsed = parse_sin(EMPTY_BODY_SIN);

    assert!(
        !parsed.saw_sin_tier,
        "an empty-body %sin line must NOT attach a sin tier (handled upstream)"
    );
    assert!(
        parsed.items.is_empty(),
        "no items can exist without a sin tier: {:?}",
        parsed.items
    );
    assert_eq!(
        parsed.diags,
        vec![
            (
                "E342".to_string(),
                30,
                30,
                "Missing required 'zero' at byte 30 (tree-sitter error recovery)".to_string(),
            ),
            (
                "E342".to_string(),
                30,
                30,
                "Missing required 'zero': the document is incomplete here and was only parsed via tree-sitter recovery (recovery is not validity)".to_string(),
            ),
        ],
        "an empty-body %sin line must surface exactly the two MISSING zero recovery diagnostics and no sin tier, got: {:?}",
        parsed.diags
    );
}
