//! Run every registered repository gate, and check the registry itself.
//!
//! ONE test over `gate::ALL` rather than one test file per gate: a gate is
//! enforced by being listed, not by somebody also remembering to write a test
//! module and declare it in `main.rs`, which is the step three of this
//! workspace's gates were lost at.
//!
//! The failure text is the gate's own operator-facing report, so CI output
//! reads exactly like running the corresponding audit binary by hand.
//!
//! # Two questions, not one
//!
//! `every_registered_gate_passes` asks whether the repository is in order.
//! `every_gate_fails_on_a_planted_violation` asks whether the gates can tell,
//! by planting a violation into an in-memory overlay of the checkout and
//! requiring the gate to report it, naming its own rule. Only the first was
//! ever asked, and a gate whose scan reads no files answers it identically to
//! a gate that works.

use std::collections::BTreeSet;

use talkbank_parser_tests::construct_coverage::ConstructCoverageGate;
use talkbank_parser_tests::gate::Gate;
use talkbank_parser_tests::gate::tree::RelPath;
use talkbank_parser_tests::gate::{ALL, Outcome, Tier, Tree, Unmet, listing, report, run_all};

/// Every rule the gates declare as unreachable by any probe, by (gate, rule).
///
/// # A set of pairs, not a count
///
/// This was `const UNPROVEN_BUDGET: usize`, on the stated grounds that "there
/// is no measurement to compare the record against, because the record IS the
/// fact". That was wrong twice. There IS a measurement: `Gate::unproven_rules`
/// is declared beside each gate's logic and this const is a separate pin.
///
/// That is WEAKER than `UNPROTECTED`, and a doc here claimed otherwise until
/// 2026-09-08: `UNPROTECTED` is compared against a sweep of the tree, so it
/// ratchets against reality, while both sides of this one are written by the
/// same author in the same commit. What it catches is drift between two files,
/// which is worth having and is not coverage. Deriving it needs a per-gate
/// vocabulary of rule identifiers that a `Probe` and an `UnprovenRule` each
/// name, so the unproven set is `rules() - probed()`; that is the change this
/// record is waiting for.
///
/// A scalar over a heterogeneous set is still the shape `content_catch_alls`
/// rejects twenty lines from here, and here with more force: SEVERAL of these
/// entries are recorded as LIVE HOLES in a gate's rule rather than gaps in its
/// probes, and under a count any of them could be traded for a note about a
/// line number with the total unmoved and the test green. No number is given
/// for how many, because a count beside the list it counts is the drift this
/// file exists to make visible; read the `why` fields.
///
/// Checked in BOTH directions, so a new unproven rule fails until somebody
/// adds it here deliberately, and a rule that becomes probeable fails until
/// its entry is deleted in the commit that closed it. The pair is the key
/// because two gates legitimately declare the same rule text: `SCOPE: only
/// `crates/` is walked` is declared twice, through the one `sources` walk both
/// hygiene gates share. The walkdir-error half used to be declared twice
/// alongside it, and by three more gates in their own words, until
/// `TreeEdit::fail_walk_under` made a walk failure something a probe can
/// plant; five entries went at once.
///
/// The list is expected to SHRINK. Adding to it is a decision, not an edit.
const UNPROVEN_RECORD: &[(&str, &str)] = &[
    // error codes are demonstrated. One is a derived report nothing compares,
    // and two are the honest limits of a text scan, both declared by sibling
    // gates in the same words.
    (
        "error codes are demonstrated",
        "the SPLIT in the clean summary, which no verdict reads",
    ),
    (
        "error codes are demonstrated",
        "ALIASED or STARRED imports: `use ErrorCode::*` then a bare variant",
    ),
    // ADDED 2026-09-08 with R4, and it is a LIVE HOLE rather than a probe gap,
    // which is why it is worded as one. Both of that gate's ratchets are
    // compiled `const` arrays, so a plant that edits their source in the
    // overlay changes nothing the running check reads; MEASURED, not assumed,
    // by a probe that added an entry and reported INERT. It applies equally to
    // `UNDEMONSTRATED`, which has had the limit undeclared since it was
    // written. Closing it means the baselines living in a file the tree hands
    // over, and a data file nobody must justify in review is a weaker ratchet
    // than a compiled one somebody has to edit.
    (
        "error codes are demonstrated",
        "the STALE direction of either compiled ratchet",
    ),
    (
        "error codes are demonstrated",
        "BLOCK COMMENTS, which `blank_literals` does not handle",
    ),
    // fabricated-AST construction. Both are limits of a text scan and say so;
    // the string-literal case that used to sit beside them was closed by
    // blanking rather than recorded.
    (
        "fabricated-AST construction",
        "SCOPE: everything outside `crates/`",
    ),
    (
        "fabricated-AST construction",
        "BLOCK COMMENTS, which `blank_literals` does not handle",
    ),
    (
        "reference-corpus combination coverage",
        "R4: a policy name that is not a grammar node kind",
    ),
    (
        "reference-corpus combination coverage",
        "the `parse returned nothing` branch",
    ),
    (
        "reference-corpus combination coverage",
        "R5's DISCOVERY direction, which the gate does not enforce",
    ),
    (
        "reference-corpus combination coverage",
        "the two numbers in the clean summary",
    ),
    (
        "reference-corpus combination coverage",
        "R5 for 18 of its 23 listed pairs",
    ),
    (
        "content-enum catch-alls (design rule 3)",
        "the `/generated/` and non-`.rs` exclusions",
    ),
    (
        "content-enum catch-alls (design rule 3)",
        "`RelPath`'s forward-slash normalisation",
    ),
    (
        "content-enum catch-alls (design rule 3)",
        "`CatchAll.line`, which the verdict never reads",
    ),
    (
        "content-enum catch-alls (design rule 3)",
        "SCOPE: everything outside `crates/`",
    ),
    (
        "content-enum catch-alls (design rule 3)",
        "VACUITY: nothing asserts the sweep read any FILE",
    ),
    (
        "content-enum catch-alls (design rule 3)",
        "a BOUND wildcard (`_other =>`) is a catch-all the scan cannot see",
    ),
    (
        "error codes have spec files",
        "the DECLARED side of the coverage rule",
    ),
    ("error codes have spec files", "an EMPTY declared set"),
    (
        "error codes have spec files",
        "per-file read failure and the directory-walk failure",
    ),
    (
        "error codes have spec files",
        "the non-UTF-8 FILENAME arm, which `continue`s rather than reporting",
    ),
    (
        "error codes have spec files",
        "everything about spec CONTENT",
    ),
    (
        "golden words parse (full corpus)",
        "parser construction failing",
    ),
    (
        "golden words parse (full corpus)",
        "CORPUS SHRINKAGE, which is not a rule this gate has",
    ),
    (
        "golden words parse (full corpus)",
        "ROUNDTRIP fidelity, MODEL correctness and BACKEND parity",
    ),
    (
        "duplicate-tests",
        "VACUITY: nothing asserts the scan found any TEST",
    ),
    ("duplicate-tests", "the `/generated/` exclusion"),
    ("duplicate-tests", "SCOPE: only `crates/` is walked"),
    ("duplicate-tests", "MEMBER ORDER within a finding"),
    (
        "duplicate-tests",
        "the FALSE-POSITIVE vector, which must not be made a probe",
    ),
    // `silent skips` reads that a reason EXISTS, never what it says, so
    // `#[ignore = "flaky"]` satisfies it and "flaky" is the one reason this
    // project refuses as a diagnosis. No plant expresses the difference: the
    // failing input is prose a human has to judge, and moving the reason into
    // the attribute is what lets a reviewer judge it at all.
    ("silent skips", "WHETHER A STATED REASON IS TRUE"),
    ("vacuous-tests", "SCOPE: only `crates/` is walked"),
    ("vacuous-tests", "ALTERNATE TEST ATTRIBUTES"),
    ("vacuous-tests", "HELPER SHADOWING, the largest hole"),
    ("vacuous-tests", "DUPLICATE `path::name` KEYS"),
    ("vacuous-tests", "nine of the ten `can_fail` tokens"),
    (
        "vacuous-tests",
        "BLOCK COMMENTS, which `blank_literals` does not handle",
    ),
    (
        "vacuous-tests",
        "the silent UTF-8 fallback inside `blank_literals`",
    ),
    ("vacuous-tests", "the scan-then-resolve ORDERING"),
    // gates read only their own tree. Two of the three are the honest limit
    // of a textual scan and say so; the third is the filesystem fault every
    // walking gate here shares.
    (
        "gates read only their own tree",
        "INDIRECTION: a `check` that calls a helper which opens a tree",
    ),
    (
        "gates read only their own tree",
        "BLOCK COMMENTS, which `blank_literals` does not handle",
    ),
    (
        "gates read only their own tree",
        "ALIASED IMPORTS: `use std::fs;` then `fs::read_to_string`",
    ),
];

/// SURVIVES: policy. WHICH invariants this repository enforces is a set of
/// choices with real alternatives, so no type can hold the list. What the type
/// does hold is that a registered gate cannot report findings without a
/// verdict: `Gate::check` returns `Outcome` and there is no accessor yielding
/// the findings alone.
#[test]
fn every_registered_gate_passes() -> Result<(), String> {
    let run_tier = Tier::from_env()?;

    let mut clean = 0usize;
    let mut failures: Vec<String> = Vec::new();
    let mut unavailable: Vec<String> = Vec::new();

    for gate in ALL {
        // A tree PER GATE, and the compiler is what says so: `check` takes it
        // by value, so one tree hoisted out of this loop is a use-after-move.
        // The tree records what was read through it, and a shared one would let
        // the last gate report clean on the strength of the first gate's reads.
        //
        // The loop COULD be rewritten to share one tree through
        // `Tree::planted`, which takes `&self`; what makes that harmless is
        // that every public route from a `Tree` to a `ReadTree` issues zero
        // evidence. `into_read` moves, `planted` copies the overlay and zeroes
        // the counters, and the `Clone` that could have handed out a tree with
        // a witness already on it is gone.
        match gate.check(Tree::live().into_read()) {
            Outcome::Clean(verdict) => {
                clean += 1;
                println!("ok  {}: {verdict}", gate.name());
            }
            Outcome::Failed(failure) => failures.push(format!("{}:\n{failure}", gate.name())),
            // An absent input is neither a pass nor a defect. Which of the two
            // it becomes is the PRECONDITION's decision, made where the fact is
            // known, and this run's tier is what it is compared against.
            Outcome::Unavailable(precondition) => match precondition.at(run_tier) {
                Unmet::Fail => failures.push(format!(
                    "{}:\nNOT CHECKED, and this run is at the {run_tier} tier: {precondition}",
                    gate.name()
                )),
                Unmet::Skip => unavailable.push(format!("{}: {precondition}", gate.name())),
            },
        }
    }

    // The summary never says everything passed while anything went unchecked.
    // "0 problems" and "checked nothing" print identically, and this is the one
    // place the difference is still known.
    if unavailable.is_empty() {
        println!("{clean} gate(s) clean, none skipped");
    } else {
        println!("{clean} gate(s) clean; {} NOT CHECKED", unavailable.len());
        for skipped in &unavailable {
            println!("skip  {skipped}");
        }
    }

    if failures.is_empty() {
        return Ok(());
    }
    Err(report(failures))
}

/// SURVIVES: behaviour reaching the outside world. Whether a gate can fail is
/// a property of running it over a tree, which no signature describes.
///
/// Every probe is planted into an in-memory overlay of the live checkout, so
/// almost all of the gate's input stays the real tree and nothing is written
/// to disk. A crashed run cannot leave the working tree dirty, and a plant
/// cannot be seen by another suite in the same process.
///
/// The run itself lives in `gate::probe::run_all`, which the renderer
/// `audit_gate_probes` also calls, so the narration a human reads and the
/// verdict CI enforces are one value rather than two implementations of it.
#[test]
fn every_gate_fails_on_a_planted_violation() -> Result<(), String> {
    let report = run_all();
    for observation in report.observations() {
        println!("{observation}");
    }
    match report.outcome() {
        Outcome::Clean(verdict) => {
            println!("{verdict}");
            Ok(())
        }
        Outcome::Failed(problems) => Err(problems),
        // Unreachable from `run_all`, and written out rather than swept into a
        // catch-all so that it breaks compilation if that ever changes.
        Outcome::Unavailable(precondition) => Err(format!(
            "the probe run itself reported an absent precondition: {precondition}"
        )),
    }
}

/// SURVIVES: policy. Which absences a run tolerates is a choice with real
/// alternatives, and the alternatives are the whole point of the tier.
///
/// The three-value [`Tier`] had one reachable value until 2026-09-08: every
/// precondition in the tree declared `PrePush`, `Tier::DEFAULT_RUN` was
/// `PrePush`, and nothing set `CHATTER_GATE_TIER`, so the comparison was
/// always `PrePush >= PrePush` and the skip branch was dead. Three independent
/// reviewers found it on the same diff, and each observed that the doc
/// promising a filtered clone would hear "not judged" described behaviour no
/// configuration produced. `just test` now runs at `inner-loop`, and this test
/// is what says the two tiers decide differently, over a REAL precondition
/// from a real gate rather than over the enum's ordering.
#[test]
fn an_absent_input_is_a_skip_in_the_inner_loop_and_a_failure_at_the_gate() -> Result<(), String> {
    let planted = Tree::live()
        .planted(|edit| {
            edit.remove_dir("corpus/reference");
            Ok(())
        })
        .map_err(|failed| format!("the plant did not apply: {failed}"))?;

    match ConstructCoverageGate.check(planted) {
        Outcome::Unavailable(precondition) => {
            assert_eq!(
                precondition.at(Tier::InnerLoop),
                Unmet::Skip,
                "a contributor whose clone omits the reference corpus must hear \
                 that the gate did not judge, not that the repository is broken"
            );
            assert_eq!(
                precondition.at(Tier::PrePush),
                Unmet::Fail,
                "the pre-push gate must never accept an unjudged gate as a pass"
            );
            Ok(())
        }
        Outcome::Clean(verdict) => Err(format!(
            "the gate reported CLEAN over a tree with no reference corpus: {verdict}"
        )),
        Outcome::Failed(why) => Err(format!(
            "the gate reported a DEFECT over a tree that merely lacks an input:\n{why}"
        )),
    }
}

/// SURVIVES: policy. WHICH rules a gate cannot probe is a judgement about each
/// gate's inputs, not a fact any type can derive, so the record is written and
/// pinned as a set.
#[test]
fn the_unproven_rule_record_matches_the_gates() -> Result<(), String> {
    // Printed by the renderer the audit binary uses, so the two cannot show a
    // reader different things. They already did: the binary dropped each
    // rule's `why` and this test kept it.
    println!("{}", talkbank_parser_tests::gate::unproven_listing());

    let mut declared: BTreeSet<(&str, &str)> = BTreeSet::new();
    let mut collisions: Vec<String> = Vec::new();
    for gate in ALL {
        let rules = gate.unproven_rules();
        for rule in rules {
            // Two entries of one gate under one name would compare as one, so
            // the set would silently hold fewer facts than the gates declare.
            // A key that is not unique is not a key.
            if !declared.insert((gate.name(), rule.rule)) {
                collisions.push(format!("{} :: {}", gate.name(), rule.rule));
            }
        }
    }

    let recorded: BTreeSet<(&str, &str)> = UNPROVEN_RECORD.iter().copied().collect();
    let show = |pairs: Vec<&(&str, &str)>| -> Vec<String> {
        pairs
            .into_iter()
            .map(|(gate, rule)| format!("{gate} :: {rule}"))
            .collect()
    };

    let text = report([
        listing(
            "FAIL: a gate declares an unprobed rule that UNPROVEN_RECORD does not \
             carry.\n\
             Adding one is a DECISION: say in the commit why no plant reaches it,\n\
             then record it here:",
            show(declared.difference(&recorded).collect()),
        ),
        listing(
            "FAIL: UNPROVEN_RECORD carries a rule no gate declares.\n\
             Delete the entry in the commit that made the rule probeable, or\n\
             re-anchor it if the rule was merely renamed. A record that outlives\n\
             its entries becomes a permanent exemption:",
            show(recorded.difference(&declared).collect()),
        ),
        listing(
            "FAIL: one gate declares two unproven rules under the same name, so \
             the record cannot tell them apart. Rename one:",
            &collisions,
        ),
    ]);
    if text.is_empty() { Ok(()) } else { Err(text) }
}

/// SURVIVES: policy. That the registry lists every implementor is a convention
/// about this crate's own source, which no type can carry.
///
/// The module doc for `gate` used to claim an unregistered gate "shows up as an
/// unused-import or dead-code warning". That was FALSE: `dead_code` does not
/// fire on public items of a library crate, and the `use crate::gate::...`
/// import is consumed by the `impl` block whether or not the type is ever
/// registered. An unregistered gate produced exactly zero diagnostics, and
/// asserting `!ALL.is_empty()` would let a one-of-four registry pass.
///
/// So the registry is checked the way this workspace checks its other
/// source-derived facts, in BOTH directions: an implementor missing from `ALL`
/// fails, and an entry naming a type that no longer implements `Gate` fails
/// (the latter would usually be a compile error, but not if the type still
/// exists under a different trait).
#[test]
fn the_registry_lists_every_gate() -> Result<(), String> {
    const SRC: &str = "crates/talkbank-parser-tests/src";
    let tree = Tree::live();

    let mut implementors: BTreeSet<String> = BTreeSet::new();
    let mut registered: BTreeSet<String> = BTreeSet::new();

    let files = tree.files_under(SRC).map_err(|err| err.to_string())?;
    for path in files {
        if !path.extension_is("rs") {
            continue;
        }
        let text = tree.read_to_string(&path).map_err(|err| err.to_string())?;
        // BLANKED before scanning, for the reason `gate_discipline` learned the
        // same day: `impl Gate for` written inside a string or a comment is not
        // a declaration. It is written inside strings in that module's own test
        // fixtures, and this check reported them as unregistered gates named
        // `B {\n    fn check(&self, ...`. A scan of source text that cannot
        // tell code from prose is the defect this whole night keeps finding.
        let blanked = talkbank_parser_tests::test_hygiene::blank_literals(&text);
        for line in blanked.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("impl Gate for ") {
                implementors.insert(rest.trim_end_matches(" {").trim().to_owned());
            }
            // Entries in `ALL` are written `&crate::<module>::<Type>,`.
            if let Some(rest) = trimmed.strip_prefix("&crate::")
                && let Some(name) = rest.trim_end_matches(',').rsplit("::").next()
            {
                registered.insert(name.to_owned());
            }
        }
    }

    if implementors.is_empty() {
        return Err(format!(
            "found no `impl Gate for` in {}; this check cannot report clean \
             having matched nothing",
            tree.root().join(RelPath::new(SRC).as_str()).display()
        ));
    }

    let unregistered: Vec<&String> = implementors.difference(&registered).collect();
    let phantom: Vec<&String> = registered.difference(&implementors).collect();

    let text = report([
        listing(
            "FAIL: gate(s) implement `Gate` but are missing from `gate::ALL`,\n\
             so they do not run. Add them:",
            &unregistered,
        ),
        listing(
            "FAIL: `gate::ALL` names type(s) with no `impl Gate for`. Remove them:",
            &phantom,
        ),
    ]);
    if text.is_empty() { Ok(()) } else { Err(text) }
}
