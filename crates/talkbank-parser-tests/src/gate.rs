//! The shape every repository-wide gate has, so that none of them can forget
//! to fail.
//!
//! # The bug class this exists to close
//!
//! A gate computes findings and must fail when there are any. Written freehand
//! that is two steps, and across this workspace the second step kept going
//! missing, in four distinct spellings: a path-set comparison inside `main`
//! that CI never invoked; a real `#[test]` that computed its findings, printed
//! them and asserted nothing; a `--check-only` mode that printed "Found N
//! invalid words" and returned `Ok(())`; and a coverage percentage compared to
//! nothing.
//!
//! Every one of these type-checks. `()` and `Ok(())` are perfectly good return
//! types for "I printed something", and nothing distinguished that from "I
//! checked something".
//!
//! # The shape
//!
//! A [`Gate`] returns a [`GateOutcome`] and has no other output. There is no
//! method that yields findings without a verdict, so "compute the list and
//! forget to act on it" is not expressible: the list is not obtainable on its
//! own. `Result` is `#[must_use]`, so ignoring the outcome is a warning rather
//! than silence.
//!
//! `GateOutcome` is deliberately a plain `Result<String, String>`. An earlier
//! cut wrapped the two sides in distinct newtypes, justified as preventing a
//! caller from swapping them. It does not: `Ok(CleanSummary::new(failure_text))`
//! compiles exactly as happily as `Ok(failure_text)`. The only mistake the
//! newtypes caught was writing `Ok(GateFailure::new(..))`, which means
//! correctly identifying the text as a failure and then typing `Ok`. `Ok` and
//! `Err` already name the side at every construction site, so the wrappers were
//! forty lines and four import entries buying nothing.
//!
//! # What is NOT closed
//!
//! [`ALL`] is hand-maintained, and registration is the whole mechanism, so a
//! gate that is written and not listed does not run. That is the bug class one
//! level up, and it is guarded the way this workspace guards its other
//! source-derived facts: `tests/integration/gates.rs` reads the `impl Gate for`
//! declarations out of this crate's sources and compares them against [`ALL`]
//! in both directions.
//!
//! Two checks in this crate remain UNCONVERTED and are named here so this
//! module does not read as though the class were finished:
//! `src/bin/verify_error_coverage.rs` still prints a coverage percentage and
//! compares it to nothing, and `src/bin/validate_golden_words.rs` retains a
//! cleaning path whose reporting half is now covered by
//! [`crate::golden_word_validity`] but whose `main` is still the only caller of
//! the rest.
//!
//! # A gate this registry cannot reach
//!
//! `talkbank-parser-re2c`'s `tests/integration/error_parity.rs` asserts, but it
//! CANNOT be registered in [`ALL`]: the registry is a `const` in this library,
//! and that gate is a module of another crate's test binary, which no library
//! can name. It borrows [`listing`] and [`report`] and reproduces the [`Gate`]
//! shape locally. Registering it properly means moving the gate into this crate
//! behind a normal (not dev) dependency on `talkbank-parser-re2c`, which is
//! acyclic but is a structural change.
//!
//! Three sibling modules in that same binary (`categorize_divergences`,
//! `quick_divergence_check`, `subcategorize_main_tier`) were briefly listed
//! here as unconverted instances too. That was wrong and the entry is gone:
//! they are `#[ignore]`d report generators that emit a taxonomy and example
//! paths to `/tmp` for a human to read, documented in that crate's CLAUDE.md as
//! manual investigations. This bug class is about checks that LOOK like gates,
//! and an ignored report generator looks like nothing of the sort.
//!
//! The census was also stale one commit after it was written, claiming a
//! "Skipping: not found" early return that one of the three does not have. A
//! hand-maintained list of another crate's checks is a value mirroring a fact
//! it cannot be derived from, which is the shape this workspace keeps paying
//! for. This module documents the limits of its OWN registry; it does not keep
//! an inventory of everybody else's.

use std::fmt;

/// The only thing a gate produces: `Ok` is the one-line clean summary, `Err` is
/// what an operator must DO about it.
///
/// The clean side is not `()` on purpose. A gate that passes should still say
/// WHAT it checked, because "0 problems" and "checked nothing" print
/// identically and the second is how a broken gate looks.
pub type GateOutcome = Result<String, String>;

/// A repository-wide invariant that CI enforces.
///
/// Implementors live beside the logic they check and are listed in [`ALL`].
pub trait Gate: Sync {
    /// How the gate is named in CI output.
    fn name(&self) -> &'static str;

    /// Run it against the working tree.
    fn check(&self) -> GateOutcome;
}

/// Every gate in this crate.
///
/// ONE list, walked by `tests/integration/gates.rs`, and checked against the
/// `impl Gate for` declarations in this crate's sources so that adding an
/// implementor without adding it here fails rather than silently not running.
pub const ALL: &[&dyn Gate] = &[
    &crate::construct_coverage::ConstructCoverageGate,
    &crate::content_catch_alls::CatchAllGate,
    &crate::error_code_specs::ErrorCodeSpecGate,
    &crate::golden_word_validity::GoldenWordsGate,
    &crate::test_hygiene::DuplicateTestGate,
    &crate::test_hygiene::VacuousTestGate,
];

/// A heading followed by its items, one per indented line.
///
/// Empty `items` yields an empty string, so [`report`] can concatenate
/// unconditionally. Five near-identical copies of this loop had been written by
/// hand across the gates, differing only in whether they emitted one newline or
/// two, which is the kind of variation that is accidental every time.
pub fn listing(heading: &str, items: impl IntoIterator<Item = impl fmt::Display>) -> String {
    let mut items = items.into_iter().peekable();
    if items.peek().is_none() {
        return String::new();
    }
    let mut out = heading.to_owned();
    for item in items {
        out.push_str(&format!("\n  {item}"));
    }
    out
}

/// Join non-empty sections with a blank line between them.
pub fn report(sections: impl IntoIterator<Item = String>) -> String {
    sections
        .into_iter()
        .filter(|section| !section.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}
