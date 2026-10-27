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

//! Recovered MOR source retains its real source location without inventing
//! morphology. The existing angle-bracketed-stem fixture exercises recovery.

use talkbank_model::{DependentTier, Span};

/// Minimal document whose `%mor` tier cannot parse: an angle-bracketed stem
/// is not valid MOR content, so its source is retained as unparsed (E316).
const MALFORMED_MOR: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello .\n%mor:\tnoun|<sos>tos .\n@End\n";

/// The tier line as it appears in the fixture, used to locate its bytes.
const MALFORMED_MOR_TIER: &str = "%mor:\tnoun|<sos>tos .";

/// Span of the first `%mor` tier in the parsed file.
fn recovered_mor_tier_span(source: &str) -> Span {
    let (utterances, _diags) = crate::common::parse_utterances_and_diags(source);
    for utterance in &utterances {
        for entry in &utterance.dependent_tiers {
            if let DependentTier::UnparsedMor(tier) = &entry.tier {
                assert!(tier.source().starts_with(MALFORMED_MOR_TIER));
                return tier
                    .location()
                    .span()
                    .expect("recovered %mor tier still carries no source location");
            }
        }
    }
    panic!("the malformed tier must be preserved in place, not dropped");
}

/// A recovered `%mor` source carries the tier's real location.
#[test]
fn recovered_mor_tier_keeps_its_source_span() {
    let start = MALFORMED_MOR
        .find(MALFORMED_MOR_TIER)
        .expect("fixture must contain the malformed tier line");
    let end = start + MALFORMED_MOR_TIER.len();

    let span = recovered_mor_tier_span(MALFORMED_MOR);

    // The tier node may extend one byte past the visible text to take in its
    // terminating newline, so the upper bound allows for it.
    assert!(
        span.start() >= start && span.end() <= end + 1,
        "recovered %mor span {span:?} should cover the tier's bytes {start}..{end}"
    );
}
