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

//! A tier dropped by lenient recovery must still carry its SOURCE SPAN.
//!
//! When a `%mor` or `%gra` tier fails to parse, the parser reports one summary
//! diagnostic and substitutes an empty placeholder tier rather than cascading
//! per-element errors. That placeholder used `Span::DUMMY`, so anything reading
//! a span off a recovered tier (a diagnostic's reported location, an LSP
//! position, a `SpanShift` during edits) pointed at byte 0 instead of the tier.
//!
//! This is chatter's own design rule 7: lenient recovery preserves malformed
//! `%mor`/`%gra` tier slots IN PLACE, and never fabricates dummy model values.
//! `Span::DUMMY` on a recovery placeholder is exactly such a fabrication.
//!
//! # Why the fixture is synthesized rather than spec-generated
//!
//! Danger rule 9 routes error-code tests through `spec/errors/`, and that is
//! right for error codes: the generators assert which codes an example emits.
//! The property here is not a code but a MODEL SPAN, which no generated fixture
//! asserts, and the three generated E316 fixtures live in a different crate and
//! cover different examples (none uses an angle-bracketed stem).
//!
//! The fixture is also necessarily invented: checked 2026-07-29 against all
//! 106,158 files in the wild corpus, no real transcript reaches this recovery
//! path. No `%mor` carries an angle-bracketed stem any more, and no `%gra`
//! fails to parse at all. That does NOT make the path dead: E316 is
//! `Status: implemented` and the spec generators exercise it on every run.
//!
//! The durable version of this test is a property over every `Layer: parser`
//! fixture in the generated corpus ("no tier in the resulting model carries a
//! dummy span"), which grows with the specs instead of pinning one path. It
//! cannot go green yet: ~33 model constructors still default their span to
//! `Span::DUMMY` and fix it up afterwards with `with_span`. See the
//! `Known hazard` section on `talkbank_model::Span`. This narrow test is the
//! interim guard, not the end state.
//!
//! The assertion deliberately does NOT pin exact offsets, so that adding a
//! header line to the fixture cannot turn a correct parser into a red test.

use talkbank_model::{DependentTier, Span};

/// Minimal document whose `%mor` tier cannot parse: an angle-bracketed stem
/// is not valid MOR content, so the tier is dropped to a placeholder (E316).
const MALFORMED_MOR: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello .\n%mor:\tnoun|<sos>tos .\n@End\n";

/// The tier line as it appears in the fixture, used to locate its bytes.
const MALFORMED_MOR_TIER: &str = "%mor:\tnoun|<sos>tos .";

/// Span of the first `%mor` tier in the parsed file.
fn recovered_mor_tier_span(source: &str) -> Span {
    let (utterances, _diags) = crate::common::parse_utterances_and_diags(source);
    for utterance in &utterances {
        for entry in &utterance.dependent_tiers {
            if let DependentTier::Mor(tier) = &entry.tier {
                return tier.span;
            }
        }
    }
    panic!("the malformed tier must be preserved in place, not dropped");
}

/// A recovered `%mor` placeholder carries the tier's real span, not `DUMMY`.
#[test]
fn recovered_mor_tier_keeps_its_source_span() {
    let start = MALFORMED_MOR
        .find(MALFORMED_MOR_TIER)
        .expect("fixture must contain the malformed tier line") as u32;
    let end = start + MALFORMED_MOR_TIER.len() as u32;

    let span = recovered_mor_tier_span(MALFORMED_MOR);

    assert_ne!(
        span,
        Span::DUMMY,
        "recovered %mor tier still carries a fabricated DUMMY span"
    );
    // The tier node may extend one byte past the visible text to take in its
    // terminating newline, so the upper bound allows for it.
    assert!(
        span.start >= start && span.end <= end + 1,
        "recovered %mor span {span:?} should cover the tier's bytes {start}..{end}"
    );
}

// NOTE: `%gra` has the same defect and the same fix (`empty_gra_placeholder`),
// but no simple fixture reaches its placeholder: a malformed `%gra` reports
// E600 and the tier does not land in the model, so there is nothing to assert a
// span on. The mechanism is covered by the `%mor` test above. Whoever finds a
// fixture that does reach the `%gra` placeholder should add the mirror test
// here, and introduce a `TierKind` enum to select between them rather than a
// boolean parameter.
