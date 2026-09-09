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

//! E756, a dependent tier with no content, and the emptiness decision behind
//! it, over files the parser built.
//!
//! # What this replaces
//!
//! The two inline tests of `talkbank-model`'s `validation/unparsed_tier.rs`
//! called `check_dependent_tier_content("xfoo", Span::DUMMY, &errors)` and
//! asserted the report. Here each is the tier line as written, `%xfoo:`
//! followed by a tab and nothing, so the emptiness DECISION
//! (`DependentTier::empty_content_span`) and the report are both exercised
//! from CHAT text. The `%xfoo` / `%xxfoo` pair records a real past bug: the
//! label already carries its `x`, and an older format string double-prefixed
//! it.
//!
//! POLICY, not an invariant a type could absorb: "a dependent tier must
//! carry content" is a CHAT rule with a real alternative (accept it, as CLAN
//! does), so the validator states it rather than the model forbidding it.

use talkbank_model::{ErrorCode, Severity};
use talkbank_parser_tests::from_source::{Rules, SingleSpeaker, diagnostics_of};
use talkbank_parser_tests::test_error::TestError;

/// Every diagnostic for `*CHI: hello .` followed by an empty `%name:` tier.
fn empty_tier_report(name: &str) -> Result<Vec<talkbank_model::ParseError>, TestError> {
    let lines = format!("*CHI:\thello .\n%{name}:\t");
    diagnostics_of(&SingleSpeaker::english(&lines).source(), Rules::Default)
}

/// A user-defined tier is reported once, named with ONE `%` and its own `x`.
#[test]
fn the_report_names_the_tier_with_one_percent_prefix() -> Result<(), TestError> {
    let reported = empty_tier_report("xfoo")?;
    assert_eq!(reported.len(), 1, "{reported:?}");
    assert_eq!(reported[0].code, ErrorCode::EmptyDependentTier);
    assert_eq!(reported[0].severity, Severity::Error);
    assert!(reported[0].message.contains("%xfoo"));
    assert!(!reported[0].message.contains("%xxfoo"));
    Ok(())
}

/// A standard tier reports under the same code and reads naturally.
#[test]
fn a_standard_tier_reports_under_the_same_code() -> Result<(), TestError> {
    let reported = empty_tier_report("eng")?;
    assert_eq!(reported.len(), 1, "{reported:?}");
    assert_eq!(reported[0].code, ErrorCode::EmptyDependentTier);
    assert!(reported[0].message.contains("%eng"));
    Ok(())
}

// ── the emptiness DECISION, `DependentTier::empty_content_span` ─────────
//
// From `talkbank-model`'s `model/dependent_tier/kind.rs`, whose inline tests
// built `UserDefinedDependentTier { .., span: Span::DUMMY }` and
// `TextTier::empty()` by hand. Every one of those states is what the parser
// builds for a tier line: `%eng:` and a tab is `Eng(None)`, the case the
// 2026-08-15 widening added (re2c had read a file with an empty `%eng:` as
// VALID because nothing judged the state); whitespace after the tab is a
// tier whose content is that whitespace.

/// One dependent-tier line and whether the model says it declares nothing.
struct TierRow {
    what: &'static str,
    line: &'static str,
    /// The whole-file verdict; whitespace after the tab is also E758.
    codes: &'static [&'static str],
    declares_nothing: bool,
}

const TIER_ROWS: &[TierRow] = &[
    TierRow {
        what: "an empty standard text tier declares nothing",
        line: "%eng:\t",
        codes: &["E756"],
        declares_nothing: true,
    },
    TierRow {
        what: "a whitespace-only standard text tier declares nothing",
        line: "%eng:\t \t",
        codes: &["E756", "E758"],
        declares_nothing: true,
    },
    TierRow {
        what: "a whitespace-only %x tier declares nothing, the case E756 was written for",
        line: "%xtst:\t \t",
        codes: &["E756", "E758"],
        declares_nothing: true,
    },
    TierRow {
        what: "a standard text tier with content is not empty",
        line: "%eng:\treal annotation",
        codes: &[],
        declares_nothing: false,
    },
    TierRow {
        what: "a %x tier with content is not empty",
        line: "%xfoo:\ttest content",
        codes: &[],
        declares_nothing: false,
    },
    TierRow {
        what: "a %x tier with nothing after the tab declares nothing",
        line: "%xfoo:\t",
        codes: &["E756"],
        declares_nothing: true,
    },
    TierRow {
        what: "a structured tier is never reported empty, even holding only its terminator",
        line: "%mor:\t.",
        codes: &[],
        declares_nothing: false,
    },
];

/// Every row's decision holds over the tier the parser builds from its line.
#[test]
fn every_emptiness_decision_holds_over_a_parsed_tier() -> Result<(), TestError> {
    let mut wrong = Vec::new();
    for row in TIER_ROWS {
        let lines = format!("*CHI:\t&=laughs .\n{}", row.line);
        let utterance = match SingleSpeaker::english(&lines).utterance(row.codes) {
            Ok(utterance) => utterance,
            Err(err) => {
                wrong.push(format!("{}: {err}", row.what));
                continue;
            }
        };
        let Some(entry) = utterance.dependent_tiers.first() else {
            wrong.push(format!("{}: the fixture built no dependent tier", row.what));
            continue;
        };
        let got = entry.tier.empty_content_span().is_some();
        if got != row.declares_nothing {
            wrong.push(format!(
                "{}: empty_content_span().is_some() was {got}, expected {}",
                row.what, row.declares_nothing
            ));
        }
    }
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(TestError::Failure(wrong.join("\n")))
    }
}
