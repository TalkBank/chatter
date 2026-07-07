//! Characterization tests for the raw / user-defined / unsupported dependent-tier
//! appliers migrated onto the generated typed visitor (Task C, sub-task 4h).
//!
//! These tests pin the OBSERVABLE behaviour at the real parser boundary
//! (`parse_chat_file_streaming` -> `ChatFile` + collected diagnostics). The
//! migration replaces three `node.kind()` / text-hacking hand-walks with the
//! generated typed `extract_*` visitor:
//!
//! - `dependent_tier_dispatch/helpers.rs`: the removed `extract_unparsed_tier_content`
//!   located a raw tier's body by scanning `node.children()` for a
//!   `text_with_bullets`-kind child; now each raw tier
//!   (`%ort`/`%eng`/`%gls`/...) extracts its own `extract_<kind>_dependent_tier`
//!   and reads the typed `child_2` body slot exhaustively.
//! - `dependent_tier_dispatch/user_defined.rs`: the `%x*` user-defined path
//!   replaces `find_child_by_kind` `match child.kind()` scans for `x_tier_prefix`
//!   / `text_with_bullets` (both real named nodes) with typed `child_0` / `child_2`
//!   slot reads. The unsupported catch-all path replaces only the
//!   `find_child_by_kind(unsupported_tier_prefix)` PREFIX scan with a typed
//!   `child_0` read; its CONTENT still comes from the tier's source text, because
//!   the `unsupported_dependent_tier` body is an ANONYMOUS `/[^\n\r]*/` token that
//!   tree-sitter emits as NO named child (so a typed `child_2` is always `Absent`).
//!
//! The migration is BEHAVIOUR-PRESERVING: the parsed tier model and the recovery
//! diagnostics must not change. Coverage split:
//! - `%eng` (raw) and `%xtra` (`%x*`) are also exercised across the reference
//!   corpus (`corpus/reference/tiers/`), so the `parser_equivalence` + roundtrip
//!   gate is their exhaustive guard; these are valid CHAT.
//! - `%custom` (unknown tier) is INVALID CHAT that the parse-don't-validate
//!   grammar structurally captures as `Unsupported`. The reference corpus contains
//!   NO unknown/unsupported tier, so this file is the SOLE guard for
//!   `apply_unsupported_tier`, a LIVE path (an unknown tier parses as
//!   `unsupported_dependent_tier`, NOT `x_dependent_tier`).

use talkbank_model::ErrorCollector;
use talkbank_model::model::{DependentTier, Line};
use talkbank_parser::TreeSitterParser;

/// A raw text-like tier (`%eng`, English translation) on one utterance. Exercises
/// the reachable `apply_raw_tier` path: `extract_eng_dependent_tier` -> a
/// `Present` `text_with_bullets` body slot -> `read_tier_body_text` -> a
/// non-empty `TextTier`.
const RAW_ENG_TIER: &str = "@UTF8\n@Begin\n*CHI:\tthe dog runs .\n%eng:\tthe dog runs\n@End\n";

/// A user-defined `%x*` tier (`%xtra`). Exercises `apply_x_tier`:
/// `extract_x_dependent_tier` -> `child_0` prefix (`%xtra`) + `child_2` body
/// (`extra annotation`) -> `DependentTier::UserDefined { label: "xtra", .. }`
/// (the "x" prefix is preserved on the label to avoid collision with built-ins).
const X_USER_TIER: &str = "@UTF8\n@Begin\n*CHI:\thello there .\n%xtra:\textra annotation\n@End\n";

/// An UNKNOWN tier name (`%custom`) with no dedicated grammar rule. `%custom` is
/// NOT valid CHAT: `chatter validate` rejects it. But the grammar is
/// parse-don't-validate, so tree-sitter STRUCTURALLY captures the line as an
/// `unsupported_dependent_tier` CST node (verified via `tree-sitter parse`), which
/// `apply_unsupported_tier` turns into `DependentTier::Unsupported { label:
/// "custom", content: "some note" }` so the VALIDATOR (a separate pass) can flag
/// it. This is the right fixture to exercise the catch-all path.
const UNKNOWN_UNSUPPORTED_TIER: &str = "@UTF8\n@Begin\n*CHI:\thi .\n%custom:\tsome note\n@End\n";

/// Collect, in document order, every dependent tier of every utterance rendered
/// as a stable `(kind, detail)` tuple, plus every collected diagnostic as
/// `(code, message)`.
fn parse_tiers(input: &str) -> (Vec<(String, String)>, Vec<(String, String)>) {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let chat = parser.parse_chat_file_streaming(input, &errors);

    let mut tiers = Vec::new();
    for line in &chat.lines.0 {
        if let Line::Utterance(u) = line {
            for dt in &u.dependent_tiers {
                tiers.push(describe_tier(dt));
            }
        }
    }

    let diags = errors
        .to_vec()
        .into_iter()
        .map(|d| (d.code.as_str().to_string(), d.message))
        .collect();

    (tiers, diags)
}

/// Render one dependent tier as a `(variant, detail)` tuple for assertions,
/// covering only the variants these tests exercise.
fn describe_tier(dt: &DependentTier) -> (String, String) {
    match dt {
        DependentTier::Eng(t) => ("Eng".to_string(), t.as_str().to_string()),
        DependentTier::UserDefined(t) => (
            format!("UserDefined:{}", t.label.as_str()),
            t.content.as_str().to_string(),
        ),
        DependentTier::Unsupported(t) => (
            format!("Unsupported:{}", t.label.as_str()),
            t.content.as_str().to_string(),
        ),
        other => (
            format!("{:?}", std::mem::discriminant(other)),
            String::new(),
        ),
    }
}

/// RAW: a `%eng` tier must parse to `Eng(TextTier)` with its exact content and
/// produce zero diagnostics.
#[test]
fn raw_eng_tier_parses_to_text_tier_with_zero_diagnostics() {
    let (tiers, diags) = parse_tiers(RAW_ENG_TIER);
    assert_eq!(
        tiers,
        vec![("Eng".to_string(), "the dog runs".to_string())],
        "the %eng raw tier must decode to its exact TextTier content"
    );
    assert!(
        diags.is_empty(),
        "a valid %eng tier must produce zero diagnostics, got: {diags:?}"
    );
}

/// USER-DEFINED: a `%xtra` tier must parse to `UserDefined` with label `xtra`
/// (the "x" preserved) and its exact content, with zero diagnostics.
#[test]
fn x_user_tier_parses_to_user_defined_with_zero_diagnostics() {
    let (tiers, diags) = parse_tiers(X_USER_TIER);
    assert_eq!(
        tiers,
        vec![(
            "UserDefined:xtra".to_string(),
            "extra annotation".to_string()
        )],
        "the %xtra tier must decode to a UserDefined tier with the x-prefixed label"
    );
    assert!(
        diags.is_empty(),
        "a valid %xtra tier must produce zero diagnostics, got: {diags:?}"
    );
}

/// CATCH-ALL: an UNKNOWN tier name (`%custom`) is INVALID CHAT (`chatter validate`
/// rejects it), but the parse-don't-validate grammar STRUCTURALLY captures it as an
/// `unsupported_dependent_tier` and `apply_unsupported_tier` turns it into
/// `DependentTier::Unsupported { label: "custom", content: "some note" }` (leading
/// `%` stripped from the label, content trimmed) so the separate validator can flag
/// it. Zero PARSE diagnostics is correct here (structural capture succeeds); the
/// invalidity is reported by `chatter validate`, not by the parser.
///
/// GROUND TRUTH (verified via `tree-sitter parse`): the `unsupported_dependent_tier`
/// body is the anonymous seq-member regex `/[^\n\r]*/`, which tree-sitter emits as
/// NO named child (the concrete children are only `unsupported_tier_prefix`,
/// `tier_sep`, `newline`; the body text sits in the byte gap between siblings). The
/// generator models that absorbed anonymous seq-token as a typed `LeafSpan`
/// (`child_2`) carrying its inter-sibling byte range, so `apply_unsupported_tier`
/// reads the body directly from that typed span (the prefix `child_0`, a real named
/// node, is visitor-driven too). This test is the SOLE guard for
/// `apply_unsupported_tier` (the reference corpus contains no unknown/unsupported
/// tier, so the equivalence/roundtrip gate never exercises this LIVE path).
/// HISTORY: before the anonymous-seq-token `LeafSpan` fix, `child_2` was always
/// `Absent`, so an earlier draft that read it regressed this to `[]` (empty) and the
/// body had to be recovered by string-splitting the source text; the `LeafSpan` fix
/// closed that generator gap, so the text-hack was removed and the body is now read
/// from the typed span.
#[test]
fn unknown_tier_is_structurally_captured_as_unsupported() {
    let (tiers, diags) = parse_tiers(UNKNOWN_UNSUPPORTED_TIER);
    assert_eq!(
        tiers,
        vec![("Unsupported:custom".to_string(), "some note".to_string())],
        "the unknown %custom tier must be structurally captured as an Unsupported tier"
    );
    assert!(
        diags.is_empty(),
        "structural capture of %custom must produce zero PARSE diagnostics \
         (its invalidity is reported separately by chatter validate), got: {diags:?}"
    );
}
