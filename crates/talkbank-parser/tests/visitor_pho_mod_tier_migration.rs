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

//! Characterization tests for the `%pho` and `%mod` dependent tiers migrated
//! onto the generated `extract_pho_dependent_tier` / `extract_mod_dependent_tier`
//! / `extract_pho_groups` / `extract_pho_group` typed visitor (Task 4d).
//!
//! These tests pin the OBSERVABLE behaviour of the `%pho` and `%mod` tiers at the
//! real parser boundary (`parse_chat_file_streaming` -> `ChatFile` + collected
//! diagnostics). The migration replaces two hand-walks:
//!
//! - `tier_parsers/pho/cst.rs` (`parse_pho_tier_inner`, shared by `%pho` and
//!   `%mod`): the `child.kind() == PHO_GROUPS` locate loop plus the
//!   `while pho_groups.child(idx)` + `match child.kind()` group iteration, with
//!   exhaustive typed `NodeSlot` matching over the dep-tier body slot (`child_2`)
//!   and each `pho_groups` group slot.
//! - `tier_parsers/pho/groups.rs` (`extract_pho_group_items`): the
//!   `node.child(0).kind()` / `node.child(1)` interior walk, with the typed
//!   `PhoGroupChoice` enum (`PhoWords` flat-words alt vs `PhoBeginGroup` bracketed
//!   `seq` alt) emitted by the 4d generator classifier.
//!
//! The migration is BEHAVIOUR-PRESERVING: the parsed phonology model and the
//! recovery diagnostics must not change. Every value here was captured by RUNNING
//! the pre-migration parser (base HEAD `e969028`), not guessed. The tests PASS on
//! the current code and MUST STAY GREEN, byte-identical, after the migration.
//!
//! # Reachability note (why the empty-body case looks the way it does)
//!
//! `parse_pho_tier_inner` is only invoked from the dispatch when the tier node
//! has NO tree-sitter error. A structurally broken `%pho`/`%mod` line (empty
//! body) makes the tier node `has_error()`: the required `pho_word` is inserted
//! by tree-sitter recovery as a MISSING node, surfaced by the whole-tree recovery
//! backstop as two `MissingRequiredElement` (E342) diagnostics, and NO pho tier
//! is attached. The "no `pho_groups` child found -> return EMPTY tier silently"
//! partial inside `parse_pho_tier_inner` is therefore UNREACHABLE from the
//! boundary. The reachable valid path (flat words AND bracketed groups) is pinned
//! by the two valid tests; the empty-body reality is pinned by
//! `empty_body_pho_tier_is_handled_upstream_without_a_pho_tier` so the migration
//! cannot accidentally start attaching an empty tier or changing these
//! diagnostics.

use talkbank_model::ErrorCollector;
use talkbank_model::model::{DependentTier, Line, PhoItem};
use talkbank_parser::TreeSitterParser;

/// A `%pho` line exercising BOTH `pho_group` interior alternatives: two bracketed
/// groups (`‹a b›`, `‹d e›`, the `PhoBeginGroup` seq alternative) interleaved
/// with flat words (`c`, `f+g` a `+`-compound, `h`, the `PhoWords` alternative).
/// Mirrors the `pho-groupings` reference shape exactly.
const VALID_PHO_GROUPS: &str = "@UTF8\n@Begin\n*CHI:\tA B C D E F .\n%pho:\t\u{2039}a b\u{203A} c \u{2039}d e\u{203A} f+g h\n@End\n";

/// The SAME body shape as [`VALID_PHO_GROUPS`] but on a `%mod` tier, proving both
/// dep-tier kinds (`extract_pho_dependent_tier` / `extract_mod_dependent_tier`)
/// route through the shared `parse_pho_tier_inner` identically.
const VALID_MOD_GROUPS: &str = "@UTF8\n@Begin\n*CHI:\tA B C D E F .\n%mod:\t\u{2039}a b\u{203A} c \u{2039}d e\u{203A} f+g h\n@End\n";

/// A `%pho:` line with an EMPTY body (tab then newline, no groups). This makes the
/// tier node `has_error()`, so it is handled by the upstream error path (E600) and
/// NO pho tier is attached; `parse_pho_tier_inner` is never called.
const EMPTY_BODY_PHO: &str = "@UTF8\n@Begin\n*CHI:\thi .\n%pho:\t\n@End\n";

/// A stable structural rendering of one `PhoItem` for assertions: a flat word or a
/// bracketed group of words.
#[derive(Debug, PartialEq, Eq)]
enum PhoItemRepr {
    /// `PhoItem::Word` rendered as its token text.
    Word(String),
    /// `PhoItem::Group` rendered as its ordered group-word texts.
    Group(Vec<String>),
}

/// Render one `PhoItem` into the stable [`PhoItemRepr`] used by assertions.
fn item_repr(item: &PhoItem) -> PhoItemRepr {
    match item {
        PhoItem::Word(word) => PhoItemRepr::Word(word.as_str().to_string()),
        PhoItem::Group(words) => {
            PhoItemRepr::Group(words.iter().map(|w| w.as_str().to_string()).collect())
        }
    }
}

/// Outcome of parsing a `%pho`/`%mod` fixture at the streaming boundary: the
/// rendered items of every phonology tier (`%pho` and `%mod` both surface as
/// `PhoTier`), whether at least one such tier was attached, and every collected
/// diagnostic as `(code, start, end, message)`.
struct PhoParse {
    items: Vec<PhoItemRepr>,
    saw_pho_tier: bool,
    diags: Vec<(String, u32, u32, String)>,
}

/// Parse `input` at the streaming boundary, collecting phonology-tier items and
/// diagnostics. Both `%pho` and `%mod` decode to `PhoTier`, so both
/// `DependentTier::Pho` and `DependentTier::Mod` are gathered.
fn parse_pho(input: &str) -> PhoParse {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let chat = parser.parse_chat_file_streaming(input, &errors);

    let mut items = Vec::new();
    let mut saw_pho_tier = false;
    for line in &chat.lines.0 {
        if let Line::Utterance(u) = line {
            for dt in &u.dependent_tiers {
                match dt {
                    DependentTier::Pho(t) | DependentTier::Mod(t) => {
                        saw_pho_tier = true;
                        items.extend(t.items.iter().map(item_repr));
                    }
                    _ => {}
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

    PhoParse {
        items,
        saw_pho_tier,
        diags,
    }
}

/// VALID `%pho`: a line with two bracketed groups and three flat words must parse
/// to the exact ordered item model and produce ZERO diagnostics. Pins the
/// `NodeSlot::Present(pho_groups)` -> `extract_pho_groups` ->
/// `NodeSlot::Present(pho_group)` -> `extract_pho_group` ->
/// `PhoGroupChoice::{PhoWords, PhoBeginGroup}` path that valid CHAT always takes,
/// including the `f+g` `+`-compound decoded as a single flat word.
#[test]
fn valid_pho_groups_parses_byte_identical_with_zero_diagnostics() {
    let parsed = parse_pho(VALID_PHO_GROUPS);

    assert!(
        parsed.saw_pho_tier,
        "a valid %pho line must attach a pho tier"
    );
    assert_eq!(
        parsed.items,
        vec![
            PhoItemRepr::Group(vec!["a".to_string(), "b".to_string()]),
            PhoItemRepr::Word("c".to_string()),
            PhoItemRepr::Group(vec!["d".to_string(), "e".to_string()]),
            PhoItemRepr::Word("f+g".to_string()),
            PhoItemRepr::Word("h".to_string()),
        ],
        "every %pho group/word must decode in order, groups as PhoItem::Group"
    );
    assert!(
        parsed.diags.is_empty(),
        "a valid %pho line must produce zero diagnostics, got: {:?}",
        parsed.diags
    );
}

/// VALID `%mod`: the SAME body on a `%mod` tier must decode to the identical item
/// model with ZERO diagnostics, proving `extract_mod_dependent_tier` routes
/// through the shared `parse_pho_tier_inner` exactly like `%pho`.
#[test]
fn valid_mod_groups_parses_byte_identical_with_zero_diagnostics() {
    let parsed = parse_pho(VALID_MOD_GROUPS);

    assert!(
        parsed.saw_pho_tier,
        "a valid %mod line must attach a pho (mod) tier"
    );
    assert_eq!(
        parsed.items,
        vec![
            PhoItemRepr::Group(vec!["a".to_string(), "b".to_string()]),
            PhoItemRepr::Word("c".to_string()),
            PhoItemRepr::Group(vec!["d".to_string(), "e".to_string()]),
            PhoItemRepr::Word("f+g".to_string()),
            PhoItemRepr::Word("h".to_string()),
        ],
        "the %mod body must decode identically to the %pho body"
    );
    assert!(
        parsed.diags.is_empty(),
        "a valid %mod line must produce zero diagnostics, got: {:?}",
        parsed.diags
    );
}

/// MALFORMED (upstream): an empty-body `%pho:` line makes the tier node
/// `has_error()` (the required `pho_word` is recovered as a MISSING node), so the
/// whole-tree recovery backstop surfaces two `MissingRequiredElement` (E342)
/// diagnostics and NO pho tier is attached; `parse_pho_tier_inner` is never called
/// and its internal "return empty tier silently" partial is unreachable from the
/// boundary. Pins that reality so the migration cannot accidentally start
/// attaching a pho tier or changing these diagnostics.
#[test]
fn empty_body_pho_tier_is_handled_upstream_without_a_pho_tier() {
    let parsed = parse_pho(EMPTY_BODY_PHO);

    assert!(
        !parsed.saw_pho_tier,
        "an empty-body %pho line must NOT attach a pho tier (handled upstream)"
    );
    assert!(
        parsed.items.is_empty(),
        "no items can exist without a pho tier: {:?}",
        parsed.items
    );
    assert_eq!(
        parsed.diags,
        vec![
            (
                "E342".to_string(),
                30,
                30,
                "Missing required 'pho_word' at byte 30 (tree-sitter error recovery)".to_string(),
            ),
            (
                "E342".to_string(),
                30,
                30,
                "Missing required 'pho_word': the document is incomplete here and was only parsed via tree-sitter recovery (recovery is not validity)".to_string(),
            ),
        ],
        "an empty-body %pho line must surface exactly the two MISSING pho_word recovery diagnostics and no pho tier, got: {:?}",
        parsed.diags
    );
}
