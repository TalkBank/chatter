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
//! A [`Gate`] reads a [`Tree`] and returns an [`Outcome`], and has no other
//! output. There is no method that yields findings without a verdict, so
//! "compute the list and forget to act on it" is not expressible: the list is
//! not obtainable on its own.
//!
//! Beyond that shape, each of these closes a hole the shape left open:
//!
//! - **The input is a parameter.** A gate that reached for `workspace_root()`
//!   itself could only ever be run against the live checkout, so nobody could
//!   ask whether it can fail. It now reads through [`Tree`], which reads
//!   through to the real checkout with an in-memory map of overrides on top.
//! - **A clean verdict carries the evidence.** Because [`ReadTree`] is the
//!   only read path a gate is given, it can say what the gate actually read,
//!   and [`ReadTree::clean`] requires that [`Examined`] witness. The single
//!   owner of the refusal is [`Outcome::clean_if_examined`], which the probe
//!   harness also uses for its own verdict. A gate that examined nothing
//!   cannot construct a clean verdict, and one that went back to `std::fs`
//!   cannot get a witness at all.
//! - **Every gate declares its [`probes`](Gate::probes).** No default body:
//!   a new gate does not COMPILE until its author says how it can be made to
//!   fail. That is the forcing function; a rule about writing probes would be
//!   a wish.
//! - **`Result` is not enough.** [`Outcome`] has a third case,
//!   [`Outcome::Unavailable`], for a gate that could not judge because an
//!   input is not in this checkout. Folded into `Ok` that is a gate skipping
//!   itself and reporting clean; folded into `Err` it is a partial checkout
//!   accused of a defect. It is neither, and the summary refuses to say
//!   everything passed while anything is unavailable.
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
//! - **A gate cannot open a second tree**, and no type says so.
//!   [`Tree::live`] is public and `pub(crate)` is no barrier to a gate in this
//!   crate, so a `check` that ignores its parameter, reads one file through a
//!   tree of its own and calls [`ReadTree::clean`] would compose a clean
//!   verdict about a checkout it was never handed, with every probe green
//!   because the plant lived in the discarded parameter. A review found that
//!   reachable on 2026-09-08, a day after this module was written, while a
//!   sentence here named a test for it that nobody had written.
//!   [`crate::gate_discipline`] is that check: it reads the `fn check` body of
//!   every file declaring `impl Gate for` and refuses the calls that reach a
//!   checkout directly. Where no affordance exists, a scan of the sources is
//!   the next best thing, and it is a registered gate with its own probes.
//!
//! And a probe reaches only what a file can change. Every gate additionally
//! declares its [`unproven_rules`](Gate::unproven_rules): the rules no plant
//! can reach, because their input is the linked grammar, a `const` in this
//! crate, or a filesystem fault an overlay cannot express. That list is
//! ratcheted in both directions by the harness, so it is a record rather than
//! a licence.
//!
//! # A gate this registry cannot reach
//!
//! `talkbank-parser-re2c`'s `tests/integration/error_parity.rs` asserts, but it
//! CANNOT be registered in [`ALL`]: the registry is a `const` in this library,
//! and that gate is a module of another crate's test binary, which no library
//! can name. It borrows [`listing`] and [`report`] and reproduces the [`Gate`]
//! shape locally.
//!
//! A 2026-09-08 review proposed lifting `gate::{tree, probe}` into a small
//! crate both depend on, and said the move was cheap now and dearer later. The
//! MACHINERY would move cleanly; the REGISTRY would not, and that is the
//! constraint worth writing down so nobody re-derives it. A `const ALL` must
//! name every implementor, so it can only live in a crate that depends on all
//! of them. Moving it to a shared crate does not help, because that crate is
//! below the gates rather than above them. The routes are a fourth crate that
//! depends on everything, a normal (not dev) dependency from here on
//! `talkbank-parser-re2c`, or link-time registration. Each is a real design
//! decision, so this is a change to argue for rather than a tidy-up to slip in.
//!
//! Three sibling modules in that same binary (`categorize_divergences`,
//! `quick_divergence_check`, `subcategorize_main_tier`) were briefly listed
//! here as unconverted instances too. That was wrong and the entry is gone:
//! they are `#[ignore]`d report generators that emit a taxonomy and example
//! paths for a human to read, documented in that crate's CLAUDE.md as manual
//! investigations. This bug class is about checks that LOOK like gates, and an
//! ignored report generator looks like nothing of the sort.

use std::fmt;

pub mod probe;
pub mod tree;

pub use probe::{Probe, ProbeReport, ProbeSuite, UnprovenRule, Verdict, run_all, unproven_listing};
pub use tree::{Examined, Plant, PlantFailed, ReadTree, RelPath, Tree, TreeEdit, TreeError};

/// Where in the development loop a check is being run, ordered by strictness.
///
/// A [`Precondition`] carries the tier at which its absence stops being a skip
/// and becomes a failure. Both ends are real and both are TRAVELLED: the
/// justfile's `_test` recipe takes the tier as an ARGUMENT, `test` passes
/// `inner-loop` and `test-all` passes `pre-push`, and
/// `an_absent_input_is_a_skip_in_the_inner_loop_and_a_failure_at_the_gate`
/// asserts that the two decide differently over a real precondition. A
/// contributor with a filtered clone that omits `corpus/` hears "not judged"
/// rather than a defect report; the pre-push gate never accepts "not judged"
/// as a pass.
///
/// The export lived on `test` for a few hours on 2026-09-08, and `test-all`
/// depends on `test`, so `just gate` inherited `inner-loop` and could have
/// skipped a gate and stayed green. A tier belongs to the caller, not to the
/// command.
///
/// There were three variants until 2026-09-08, the third being `Release`.
/// Nothing constructed it, nothing set it, and no precondition required it, so
/// it was a point on an axis that no run occupied and no rule referred to.
/// Deleted rather than documented as unreachable: a release-only precondition
/// would bring it back in one line, and until one exists the enum should hold
/// exactly what a run can be.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// `just test`, the per-edit loop.
    InnerLoop,
    /// `just gate`, which is the pre-push gate and what CI runs.
    PrePush,
}

impl Tier {
    /// The tier a run assumes when nothing says otherwise.
    ///
    /// Fail-CLOSED: the strict reading unless something explicitly relaxes it.
    /// A default of `InnerLoop` would mean the ordinary run treats every
    /// missing input as a skip, and a gate that can skip itself is not a gate.
    pub const DEFAULT_RUN: Self = Self::PrePush;

    /// The tier this run is at, from `CHATTER_GATE_TIER`.
    ///
    /// # Errors
    ///
    /// On an unrecognised value. Never a silent default: a typo that quietly
    /// relaxed the run is precisely the failure this whole module is about.
    pub fn from_env() -> Result<Self, String> {
        match std::env::var("CHATTER_GATE_TIER") {
            Err(_) => Ok(Self::DEFAULT_RUN),
            Ok(text) => match text.trim() {
                "inner-loop" => Ok(Self::InnerLoop),
                "pre-push" => Ok(Self::PrePush),
                other => Err(format!(
                    "CHATTER_GATE_TIER={other:?} is not a tier; \
                     use inner-loop or pre-push"
                )),
            },
        }
    }

    /// How the tier is written in a report and in the environment.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InnerLoop => "inner-loop",
            Self::PrePush => "pre-push",
        }
    }
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An input the gate needs and this checkout does not have.
///
/// Carries the tier at which its absence is a hard failure, so the decision is
/// made where the fact is known rather than by a reader of a log line.
pub struct Precondition {
    /// What is missing, in the words an operator would use to supply it.
    pub missing: String,
    /// From this tier upward, its absence fails rather than skips.
    pub required_at: Tier,
}

impl Precondition {
    /// What a run at `tier` does about this absence.
    ///
    /// The decision, as a value, at the one place the fact is known. It was an
    /// `if run_tier >= precondition.required_at` inside the integration test,
    /// which is this project's "an `if` on a domain fact" smell in its exact
    /// form: the comparison is a bool that has lost the question it answers,
    /// and being inside a `#[test]` it could not be called at two tiers, so
    /// nothing established that the two tiers decide differently. They now
    /// have a test, and the caller matches instead of comparing.
    #[must_use]
    pub fn at(&self, tier: Tier) -> Unmet {
        match tier >= self.required_at {
            true => Unmet::Fail,
            false => Unmet::Skip,
        }
    }
}

/// What an unmet precondition means at the tier a run is at.
///
/// Two names rather than a bool, because they are two operator actions: a
/// contributor with a filtered clone is told the gate did not judge, and the
/// pre-push gate refuses to accept "not judged" as a pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unmet {
    /// Report it, do not count it as a pass, do not call it a defect.
    Skip,
    /// A checkout that cannot answer the question fails the run.
    Fail,
}

impl fmt::Display for Precondition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (required from the {} tier upward)",
            self.missing, self.required_at
        )
    }
}

/// What a gate establishes about a tree.
///
/// Three cases, not `Result`'s two. The clean side is not `()` on purpose: a
/// gate that passes should still say WHAT it checked, because "0 problems" and
/// "checked nothing" print identically and the second is how a broken gate
/// looks. And a gate that could not read its input is neither of the other
/// two, which a `Result` forced it to be.
///
/// The clean side carries an [`Examined`] for the same reason one step
/// further: a SUMMARY is a sentence the gate wrote, and a gate that read
/// nothing can write a perfectly convincing one. The witness is issued by the
/// tree, not by the gate, so it is the one part of a clean verdict its author
/// cannot compose.
pub enum Outcome {
    /// Checked, and in order.
    Clean(CleanVerdict),
    /// Checked, and not in order. The text says what an operator must DO.
    Failed(String),
    /// Not checked, because an input is not in this tree.
    Unavailable(Precondition),
}

/// A clean verdict and the evidence it rests on, which cannot be paired by
/// hand.
///
/// # Why the payload is a struct with private fields
///
/// `Clean(String, Examined)` read well and was forgeable: variants of a `pub`
/// enum are constructible wherever the enum is visible, [`Examined`] is `Copy`,
/// and [`Tree::live`] is `pub`, so a gate could ignore the tree it was handed,
/// read one file through a tree of its own, and compose a clean verdict about
/// a checkout it never opened. Every probe against such a gate goes green,
/// because the planted violation lived in the discarded parameter. The module
/// doc claimed the opposite: it said there was deliberately no public
/// constructor pairing a summary with a witness from anywhere, and the variant
/// WAS that constructor, spelled differently.
///
/// # Every way to obtain one
///
/// [`Outcome::clean_if_examined`], which is crate-private, and
/// [`ReadTree::clean`], which is that with the tree's own evidence supplied.
/// The fields are private to this module, so no `Self { .. }` literal is
/// writable elsewhere, and the only accessors outside are `Display`.
///
/// This closes the composition hole and not the wider one, which no type can:
/// a gate can still call the public [`Tree::live`] inside its own `check`,
/// read one file through that, and compose a clean verdict about a tree it was
/// never handed. [`crate::gate_discipline`] is the check that forbids it,
/// written on 2026-09-08 after a review found the hole reachable and found
/// this paragraph naming a test for it that did not exist.
pub struct CleanVerdict {
    summary: String,
    examined: Examined,
}

impl CleanVerdict {
    /// The evidence, for the harness that folds several gates' reads.
    pub(crate) fn examined(&self) -> Examined {
        self.examined
    }
}

impl fmt::Display for CleanVerdict {
    /// The summary and its evidence, never one without the other: printing the
    /// sentence alone is how "0 problems" and "checked nothing" came to look
    /// alike in the first place.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.summary, self.examined)
    }
}

impl Outcome {
    /// Clean, or the refusal a gate owes when it examined NOTHING.
    ///
    /// ONE owner of that refusal, and it is deliberately a FAILURE rather than
    /// an `Unavailable`: a tree the gate never read is not a tree that lacks an
    /// input, it is a gate that did not run. Every route to a clean verdict
    /// passes through here, so the two spellings of "0 problems" cannot drift
    /// apart.
    ///
    /// # Every route to a clean verdict
    ///
    /// This one, crate-private, and [`ReadTree::clean`], which is this one with
    /// the tree's own evidence supplied. There is deliberately no public
    /// `Outcome::clean(summary, witness)` taking a witness from anywhere: it
    /// existed for one commit, had no caller, and would have let a summary
    /// about one tree be paired with a witness earned by another.
    ///
    /// That sentence was FALSE until 2026-09-08, and the falsehood is worth
    /// keeping here rather than quietly deleting. `Clean(String, Examined)` was
    /// itself the missing constructor: a `pub` enum's variants are writable
    /// wherever the enum is, so the function the doc said did not exist was
    /// available under another name. [`CleanVerdict`] is the fix, and the
    /// general lesson is that a tuple variant of a public enum is a public
    /// constructor of whatever it holds.
    pub(crate) fn clean_if_examined(
        summary: impl Into<String>,
        examined: Option<Examined>,
    ) -> Self {
        let summary = summary.into();
        match examined {
            Some(examined) => Self::Clean(CleanVerdict { summary, examined }),
            None => Self::failed(format!(
                "REFUSING A CLEAN VERDICT: this gate read no file and enumerated no \
                 directory through the tree it was handed, so\n  {summary}\n\
                 is a report about nothing. A gate whose scan reads nothing answers \
                 identically to a gate that works, which is the bug class this trait \
                 exists to close."
            )),
        }
    }

    /// Not in order, with the operator-facing report.
    #[must_use]
    pub fn failed(report: impl Into<String>) -> Self {
        Self::Failed(report.into())
    }

    /// Print it, and give the process its exit code.
    ///
    /// Three outcomes, three codes, in ONE place. UNAVAILABLE is deliberately
    /// not success: a run that could not read its input has established
    /// nothing, and exiting 0 on it is the shape this module exists to refuse.
    /// It lives here rather than in each `src/bin` renderer because the two
    /// that existed were already byte-identical, and a third would have picked
    /// its own mapping.
    #[must_use]
    pub fn into_exit_code(self) -> std::process::ExitCode {
        match self {
            Self::Clean(verdict) => {
                println!("{verdict}");
                std::process::ExitCode::SUCCESS
            }
            Self::Failed(why) => {
                eprintln!("{why}");
                std::process::ExitCode::FAILURE
            }
            Self::Unavailable(precondition) => {
                eprintln!("NOT CHECKED: {precondition}");
                std::process::ExitCode::from(2)
            }
        }
    }

    /// Not judged: an input is absent from this checkout.
    #[must_use]
    pub fn unavailable(missing: impl Into<String>, required_at: Tier) -> Self {
        Self::Unavailable(Precondition {
            missing: missing.into(),
            required_at,
        })
    }
}

/// The two-case shape, kept for ONE borrower outside this crate.
///
/// `talkbank-parser-re2c`'s `tests/integration/error_parity.rs` reproduces the
/// [`Gate`] shape locally because it cannot be registered in [`ALL`] (see the
/// module doc), and it returns this. It is NOT what a registered gate returns:
/// that is [`Outcome`], which has a third case for a tree it could not judge.
/// Do not reach for this in a new gate.
pub type GateOutcome = Result<String, String>;

/// A repository-wide invariant that CI enforces.
///
/// Implementors live beside the logic they check and are listed in [`ALL`].
pub trait Gate: Sync {
    /// How the gate is named in CI output.
    fn name(&self) -> &'static str;

    /// Run it against a tree.
    ///
    /// The tree is a PARAMETER, and reads go through it rather than through
    /// `std::fs`, or a probe could not plant anything the gate would see.
    ///
    /// It is taken BY VALUE, and that is load-bearing rather than tidy. The
    /// tree records what was read through it, and [`Tree::clean`] turns that
    /// into the witness a clean verdict needs; one tree shared across the
    /// registry would let the sixth gate report clean on the strength of the
    /// first five gates' reads. Moving it makes the caller hand each gate its
    /// own, and the compiler, not a reviewer, is what says so.
    fn check(&self, tree: ReadTree) -> Outcome;

    /// Trees this gate must reject, and trees it must still accept.
    ///
    /// **No default body, deliberately.** A gate with no probes is a gate
    /// nobody has watched fail, and an empty default would let one be written
    /// in silence. With none, a new gate does not compile until its author
    /// supplies them, which is the affordance rather than the rule.
    ///
    /// [`ProbeSuite`]'s constructor takes the first refusal as arguments and
    /// adds the control itself, so neither a suite with no probes nor one with
    /// only a control is a value this can return. The harness used to check
    /// for the second at runtime and did not catch the first.
    fn probes(&self) -> ProbeSuite;

    /// Rules of this gate that no probe can reach, and why.
    ///
    /// Also no default body. An empty slice is a legal answer and says
    /// "everything here is probed"; what is not available is saying nothing.
    fn unproven_rules(&self) -> &'static [UnprovenRule];
}

/// Every gate in this crate.
///
/// ONE list, walked by `tests/integration/gates.rs`, and checked against the
/// `impl Gate for` declarations in this crate's sources so that adding an
/// implementor without adding it here fails rather than silently not running.
pub const ALL: &[&dyn Gate] = &[
    &crate::construct_coverage::ConstructCoverageGate,
    &crate::content_catch_alls::CatchAllGate,
    &crate::error_code_demonstration::ErrorCodeDemonstrationGate,
    &crate::error_code_specs::ErrorCodeSpecGate,
    &crate::fabricated_ast::FabricatedAstGate,
    &crate::gate_discipline::GateDisciplineGate,
    &crate::golden_word_validity::GoldenWordsGate,
    &crate::test_hygiene::DuplicateTestGate,
    &crate::test_hygiene::SilentSkipGate,
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
