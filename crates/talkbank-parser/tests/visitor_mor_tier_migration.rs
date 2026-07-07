//! Characterization tests for the `%mor` dependent tier migrated onto the
//! generated `extract_mor_dependent_tier` / `extract_mor_contents` /
//! `extract_mor_content` / `extract_mor_word` / `extract_mor_feature` /
//! `extract_mor_post_clitic` typed visitor (Task C, resuming paused sub-task
//! 4g).
//!
//! These tests pin the OBSERVABLE behaviour of the `%mor` tier at the real
//! parser boundary (`parse_chat_file_streaming` -> `ChatFile` + collected
//! diagnostics). The migration replaces the hand-walk across
//! `tier_parsers/mor/{tier,item,word}.rs`: the positional `expect_child_at`
//! get of `mor_contents` plus the flat `while mor_contents.child(idx)` /
//! `while node.child(idx)` scans that dispatched by `node.kind()`, with
//! exhaustive typed `NodeSlot` matching at every position.
//!
//! The migration is BEHAVIOUR-PRESERVING: the parsed tier content and the
//! recovery diagnostics must not change. Values here are derived by tracing
//! the removed hand-walk and the model's own round-trip serialization
//! (`MorTier::to_content()`), not guessed; the round-trip check is exactly as
//! strong a pin as re-deriving the typed `Mor`/`MorWord` structures by hand,
//! and is robust to any incidental internal representation detail.
//!
//! # Reachability note (why the malformed case looks the way it does)
//!
//! `parse_mor_tier` is only invoked from the dispatch when the tier node has
//! NO tree-sitter error (`dependent_tier_dispatch/parsed.rs`:
//! `MOR_DEPENDENT_TIER => if tier_node.has_error() { ... } else {
//! parse_mor_tier(...) }`). A structurally broken `%mor` line (no content at
//! all) makes the tier node `has_error()` and never reaches `parse_mor_tier`.
//! The reachable malformed path exercised here is a STRUCTURALLY valid
//! `%mor` line that simply omits the (grammar-optional) trailing terminator,
//! which `parse_mor_tier_inner`'s own policy rejects with `MissingTerminator`
//! (E305), matching the tier's decade-old behavior of never allowing a
//! terminator-less `%mor` tier through even though the grammar permits the
//! shape structurally.

use talkbank_model::ErrorCollector;
use talkbank_model::model::{DependentTier, Line};
use talkbank_parser::TreeSitterParser;

/// A valid `%mor` line with four plain items and a period terminator.
const VALID_MULTI_ITEM: &str =
    "@UTF8\n@Begin\n*CHI:\tI go home now .\n%mor:\tpro|I v|go n|home adv|now .\n@End\n";

/// A valid `%mor` line exercising a post-clitic with morphological features
/// (`cop|be-Fin-Ind-Pres-S3`) and a POS subcategory (`pro:poss`), plus a
/// period terminator.
const VALID_POST_CLITIC_WITH_FEATURES: &str = "@UTF8\n@Begin\n*CHI:\tthat's mine .\n%mor:\tpro|that~cop|be-Fin-Ind-Pres-S3 pro:poss|mine .\n@End\n";

/// A structurally valid `%mor` line with two items and NO terminator (the
/// grammar makes the trailing `(whitespaces, terminator)` optional).
/// `parse_mor_tier_inner` rejects this with `MissingTerminator` (E305); the
/// `mor_contents` node span is `v|pick prt|up` at byte offsets 35..48.
const NO_TERMINATOR: &str = "@UTF8\n@Begin\n*CHI:\tpick up .\n%mor:\tv|pick prt|up\n@End\n";

/// Parse `input` at the streaming boundary and return, in document order,
/// every `%mor` tier's `to_content()` serialization (items + terminator,
/// matching the tier's canonical round-trip text), whether at least one
/// `%mor` tier was attached at all, and every collected diagnostic as
/// `(code, start, end, message)`.
fn parse_mor(input: &str) -> (Vec<String>, bool, Vec<(String, u32, u32, String)>) {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let chat = parser.parse_chat_file_streaming(input, &errors);

    let mut tiers = Vec::new();
    let mut saw_mor_tier = false;
    for line in &chat.lines.0 {
        if let Line::Utterance(u) = line {
            for dt in &u.dependent_tiers {
                if let DependentTier::Mor(t) = dt {
                    saw_mor_tier = true;
                    tiers.push(t.to_content());
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

    (tiers, saw_mor_tier, diags)
}

/// VALID: a four-item `%mor` line must round-trip to identical content and
/// produce ZERO diagnostics. Pins the `NodeSlot::Present(MorContent(...))`
/// items path -> `extract_mor_content` -> `extract_mor_word` (POS/pipe/lemma,
/// no features) that valid CHAT with plain items always takes, plus the
/// `NodeSlot::Present` terminator-group path via `terminator_from_new_choice`.
#[test]
fn valid_multi_item_mor_round_trips_with_zero_diagnostics() {
    let (tiers, saw_mor_tier, diags) = parse_mor(VALID_MULTI_ITEM);

    assert!(saw_mor_tier, "a valid %mor line must attach a mor tier");
    assert_eq!(
        tiers,
        vec!["pro|I v|go n|home adv|now .".to_string()],
        "the tier must round-trip to identical %mor content"
    );
    assert!(
        diags.is_empty(),
        "a valid %mor line must produce zero diagnostics, got: {diags:?}"
    );
}

/// VALID: a post-clitic with morphological features and a POS subcategory
/// must round-trip identically. Pins `extract_mor_content`'s `post_clitics`
/// repeat -> `extract_mor_post_clitic` -> `extract_mor_word` ->
/// `extract_mor_feature` (repeat of 4 features) path.
#[test]
fn valid_post_clitic_with_features_round_trips_with_zero_diagnostics() {
    let (tiers, saw_mor_tier, diags) = parse_mor(VALID_POST_CLITIC_WITH_FEATURES);

    assert!(saw_mor_tier, "a valid %mor line must attach a mor tier");
    assert_eq!(
        tiers,
        vec!["pro|that~cop|be-Fin-Ind-Pres-S3 pro:poss|mine .".to_string()],
        "the tier must round-trip to identical %mor content, including the \
         post-clitic's four features and the POS subcategory"
    );
    assert!(
        diags.is_empty(),
        "a valid %mor line must produce zero diagnostics, got: {diags:?}"
    );
}

/// MALFORMED (reachable): a structurally valid `%mor` line with items but no
/// terminator. `parse_mor_tier_inner` rejects the whole tier with
/// `MissingTerminator` (E305) at the `mor_contents` node's exact span
/// (35..48); the dispatch (`dependent_tier_dispatch/parsed.rs`, untouched by
/// this migration) still attaches an EMPTY placeholder mor tier on
/// `Rejected` (`empty_mor_placeholder()`, zero items + a synthetic Period
/// terminator), so `saw_mor_tier` stays true but the two real items are
/// dropped, never silently recovered as if they had parsed.
#[test]
fn missing_terminator_rejects_the_whole_tier_leaving_the_empty_placeholder() {
    let (tiers, saw_mor_tier, diags) = parse_mor(NO_TERMINATOR);

    assert!(
        saw_mor_tier,
        "the mor tier must remain attached (as the empty placeholder) for downstream diagnostics"
    );
    assert_eq!(
        tiers,
        vec![".".to_string()],
        "a rejected tier falls back to the empty placeholder (zero items, \
         synthetic Period terminator), never the two real items"
    );

    let e305: Vec<&(String, u32, u32, String)> =
        diags.iter().filter(|(c, _, _, _)| c == "E305").collect();
    assert_eq!(
        e305,
        vec![&(
            "E305".to_string(),
            35,
            48,
            "%mor tier is missing a terminator".to_string(),
        )],
        "exactly one MissingTerminator diagnostic at the mor_contents span is expected, got: {diags:?}"
    );
}
