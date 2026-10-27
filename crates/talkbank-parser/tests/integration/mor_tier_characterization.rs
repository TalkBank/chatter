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

//! Parsed MOR content and malformed-tier refusal at the public parser boundary.
//! Valid content round-trips; missing terminators retain raw source rather than
//! inventing an empty successful tier. Diagnostics keep their original location.
//!
//! # Reachability note (why the malformed case looks the way it does)
//!
//! `parse_mor_tier` takes a `HealthyTier`, which only `TierAdmission::admit`
//! builds, and only for a tier node with NO tree-sitter error; the dispatch
//! reports a recovery carrier instead. A structurally broken `%mor` line (no
//! content at all) makes the tier node `has_error()` and never reaches
//! `parse_mor_tier`.
//! The reachable malformed path exercised here is a STRUCTURALLY valid
//! `%mor` line that simply omits the (grammar-optional) trailing terminator,
//! which `parse_mor_tier_inner`'s own policy rejects with `MissingTerminator`
//! (E305), matching the tier's decade-old behavior of never allowing a
//! terminator-less `%mor` tier through even though the grammar permits the
//! shape structurally.

use talkbank_model::ErrorCollector;
use talkbank_model::model::{DependentTier, Line};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::ParseProductTestExt;

/// One diagnostic: (code, span start, span end, message).
type Diag = (String, usize, usize, String);

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
fn parse_mor(input: &str) -> (Vec<String>, bool, Vec<Diag>) {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let chat = parser.parse_chat_file_streaming(input, &errors);

    let mut tiers = Vec::new();
    let mut saw_mor_tier = false;
    for line in chat.lines.as_slice() {
        if let Line::Utterance(u) = line {
            for dt in &u.dependent_tiers {
                if let DependentTier::Mor(t) = &dt.tier {
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
                d.location.span().expect("located diagnostic").start(),
                d.location.span().expect("located diagnostic").end(),
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

/// A grammar-admitted tier without a required terminator reports E305 while
/// preserving its original source as explicitly unparsed morphology.
#[test]
fn missing_terminator_preserves_unparsed_source_without_a_fabricated_period() {
    let (tiers, saw_mor_tier, diags) = parse_mor(NO_TERMINATOR);
    assert!(!saw_mor_tier, "raw recovery is not parsed morphology");
    assert!(tiers.is_empty());
    let parser = TreeSitterParser::new().expect("grammar loads");
    let file = parser.parse_chat_file(NO_TERMINATOR).expect_built();
    let utterance = file.utterances().next().expect("retained utterance");
    assert!(matches!(&utterance.dependent_tiers[0].tier,
        DependentTier::UnparsedMor(raw) if raw.source() == "%mor:\tv|pick prt|up\n"));
    assert!(!utterance.parse_health().can_align_main_to_mor());

    let e305: Vec<&(String, usize, usize, String)> =
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
