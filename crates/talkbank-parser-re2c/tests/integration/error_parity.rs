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

//! Do the two parser backends agree with each other, and does each meet the
//! spec? Two orthogonal axes, both measured, one of them gated.
//!
//! # What this file used to be, and why that was the bug
//!
//! Until 2026-08-09 this ran on every test run, printed
//!
//! ```text
//! Exact code match:   214        Re2c SILENT:  11  <- critical gaps
//! ```
//!
//! and PASSED. It had no assertion. `E375_replacement_needs_preceding_space`
//! sat on that silent list while the very same divergence was rediscovered by
//! hand, hours later, from a corpus line: the audit had been right all along
//! and nobody read it.
//!
//! `talkbank-parser-tests`' `gate` module documents this exact bug class ("a
//! real `#[test]` that computed its findings, printed them and asserted
//! nothing") and names three instances. This was the fourth, one crate over,
//! and three more live in this very test binary: `categorize_divergences`,
//! `quick_divergence_check` and `subcategorize_main_tier` each classify
//! backend divergences, print a report, and assert nothing. They are named in
//! `gate.rs`'s own "What is NOT closed" list so the enumeration there does not
//! read as finished.
//!
//! This gate borrows that module's `listing`/`report`/[`GateOutcome`] rather
//! than re-copying them, but it cannot be REGISTERED in `gate::ALL`: that
//! registry is a `const` in the library crate, and this gate is a module of
//! another crate's test binary, which no library can name. The shape is
//! reproduced instead: [`audit`] returns a verdict and nothing else, so there
//! is no way to obtain the findings without also deciding about them. Moving
//! the gate INTO `talkbank-parser-tests` would fix that properly and needs a
//! normal (not dev) dependency on this crate; it is a structural change, not a
//! drive-by.
//!
//! # Two axes, because five buckets were one value proxying for two facts
//!
//! Every bucket in the old version was defined against the SPEC's expectation,
//! so the headline "Exact parity: 214/283" measured *both backends satisfy the
//! spec*, which is not what parity means for an oracle whose whole job is to
//! track the other backend. Two cases it graded backwards:
//!
//! - both backends report the same wrong code: perfect agreement, counted as a
//!   parity failure;
//! - both meet the expectation but re2c also invents an extra diagnostic: a
//!   real divergence, counted as parity.
//!
//! So [`model::Conformance`] answers "did this backend meet the spec", per
//! backend, and [`model::Divergence`] answers "do the backends produce the same
//! diagnostics". Both are DERIVED from the four recorded fields of a
//! [`CaseReport`], never
//! stored beside them, because a stored classification is a value that can
//! drift from the thing it classifies.
//!
//! # No `bool` crosses a function boundary in these modules
//!
//! That is the checkable form of "no boolean blindness", and the first draft of
//! this rewrite broke it while removing the same fault from the code it
//! replaced: it grew `Reported::is_silent()` and `meets()`, then matched on the
//! PAIR `(tree_sitter.is_silent(), re2c.is_silent())`, two bools with an
//! unreachable arm. Emptiness is therefore a VARIANT here ([`Reported::Silent`]
//! versus a non-empty [`Reported::Spoke`]), not a property a caller has to
//! remember to test, and the derived verdicts carry payloads (WHICH expected
//! codes were absent) instead of collapsing to yes/no.
//!
//! Equality and emptiness comparisons consumed on the spot are still written as
//! comparisons: a bool that never travels cannot lose what it was about.
//!
//! # The ratchet
//!
//! [`baseline::KNOWN_DIVERGENCES`] names every case where the backends disagree
//! today.
//! Named, not counted: a count passes a run that closes one gap and opens
//! another, which is precisely the swap a parity ratchet exists to catch.
//!
//! It is bidirectional, like `construct_coverage`'s list. An entry that stops
//! diverging FAILS until it is deleted, so the list cannot rot into a permanent
//! excuse once somebody fixes a gap and forgets to prune it, and the finish
//! line is the list being empty. That is what "re2c tracks tree-sitter" means,
//! and no percentage could ever show it.
//!
//! Only the spec suite is in scope. Divergence on WILD data is a different
//! measurement with a different instrument (the corpus differential), and this
//! gate must not be read as covering it.
//!
//! # Layout
//!
//! Split when this file passed the workspace's 800-line hard limit, along the
//! section boundaries it already had:
//!
//! - [`model`]: what one case looks like once both backends have run over it,
//!   and every type that makes an illegal state unrepresentable in it.
//! - [`spec_corpus`]: getting from markdown on disk to a testable case,
//!   including which specs are in scope.
//! - [`baseline`]: the recorded divergences. Its own file because it is what a
//!   contributor edits when they fix one, and it should shrink visibly in a
//!   diff without the machinery moving around it.
//! - here: running both backends, reconciling against the baseline, and the
//!   two `#[test]`s that are the gate.

mod baseline;
mod model;
mod spec_corpus;

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::btree_map::Entry;

use talkbank_model::ErrorCollector;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::gate::{GateOutcome, listing, report};

use baseline::KNOWN_DIVERGENCES;
use model::{CaseReport, ConformanceTally, Divergence, DivergingCase, Reported, SpecLabel};
use spec_corpus::{SpecCorpus, load_spec_corpus};
use talkbank_model::model::TranscriptName;

// ---------------------------------------------------------------------------
// Running both backends
// ---------------------------------------------------------------------------

/// Validate one input with one backend and keep only the codes.
///
/// Shared by both backends so that HOW a run is measured is written once. It
/// had been written twice, differing in one line, in the very function whose
/// output exists to detect the two backends drifting apart.
///
/// `into_vec` rather than `to_vec`: the collector dies on the next line, and
/// `to_vec` deep-clones every `ParseError`, each carrying a message `String`
/// and an optional context holding two more, to read one `Copy` field off it.
fn codes_from(lower: impl FnOnce(&ErrorCollector) -> talkbank_model::model::ChatFile) -> Reported {
    let errors = ErrorCollector::new();
    let mut file = lower(&errors);
    file.validate_with_alignment(&errors, TranscriptName::Anonymous);
    Reported::of(
        errors
            .into_vec()
            .into_iter()
            .map(|error| error.code)
            .collect(),
    )
}

/// Parse and validate every case with each backend, in one pass.
///
/// The tree-sitter parser is built ONCE, though that is a smaller saving than
/// it sounds: the grammar is statically linked, so `TreeSitterParser::new` is a
/// `Parser::new` plus `set_language` and this crate's benchmark note calls that
/// cost negligible. The real waste removed was the old debug pass, which walked
/// `spec/errors` a second time and re-parsed all 239 markdown files on every
/// run purely to print what the first pass had already computed.
fn measure(corpus: &SpecCorpus) -> Result<Vec<CaseReport>, String> {
    let parser =
        TreeSitterParser::new().map_err(|err| format!("cannot load the CHAT grammar: {err}"))?;

    let reports = corpus
        .cases
        .iter()
        .map(|case| CaseReport {
            label: case.label.clone(),
            expected: case.expected.clone(),
            tree_sitter: codes_from(|errors| parser.parse_chat_file_streaming(&case.input, errors)),
            re2c: codes_from(|errors| {
                let parsed =
                    talkbank_parser_re2c::parser::parse_chat_file_streaming(&case.input, errors);
                talkbank_model::model::ChatFile::from(&parsed)
            }),
        })
        .collect();
    Ok(reports)
}

// ---------------------------------------------------------------------------
// The gate
// ---------------------------------------------------------------------------

/// Read [`KNOWN_DIVERGENCES`] into a map, refusing a duplicated label.
///
/// A plain `collect()` keeps the LAST of two entries naming the same case and
/// says nothing, so the earlier one, and whatever reason was written beside it,
/// vanishes from the ratchet with no diagnostic anywhere.
fn recorded_divergences() -> Result<BTreeMap<&'static str, Divergence>, String> {
    let mut recorded = BTreeMap::new();
    for (label, shape) in KNOWN_DIVERGENCES {
        match recorded.insert(*label, *shape) {
            None => {}
            Some(previous) => {
                return Err(format!(
                    "KNOWN_DIVERGENCES lists {label:?} twice ({previous:?} then {shape:?}); \
                     keep one entry"
                ));
            }
        }
    }
    Ok(recorded)
}

/// Index this run's diverging cases by label, refusing a collision.
///
/// Two distinct cases rendering to one label would silently drop a divergence.
/// It cannot happen for generated spec names, and the point of checking is that
/// nothing has to KNOW that in order for the count to be right.
fn observed_divergences<'a>(
    reports: &'a [CaseReport],
) -> Result<BTreeMap<String, DivergingCase<'a>>, String> {
    let mut observed = BTreeMap::new();
    for report in reports {
        let Some(diverging) = DivergingCase::of(report) else {
            continue;
        };
        match observed.entry(report.label.to_string()) {
            Entry::Vacant(slot) => {
                slot.insert(diverging);
            }
            Entry::Occupied(slot) => {
                return Err(format!("two spec cases both render as {:?}", slot.key()));
            }
        }
    }
    Ok(observed)
}

/// The whole check. `Ok` is the summary of what was verified; `Err` is what an
/// operator has to do about it.
///
/// There is deliberately no function yielding the findings without a verdict:
/// that separation is what let the previous version print eleven critical gaps
/// and pass.
fn audit() -> GateOutcome {
    let corpus = load_spec_corpus()?;
    let reports = measure(&corpus)?;

    let observed = observed_divergences(&reports)?;
    let recorded = recorded_divergences()?;

    let tree_sitter = ConformanceTally::of(reports.iter().map(CaseReport::tree_sitter_conformance));
    let re2c = ConformanceTally::of(reports.iter().map(CaseReport::re2c_conformance));
    let mut shapes: BTreeMap<Divergence, usize> = BTreeMap::new();
    for diverging in observed.values() {
        *shapes.entry(diverging.shape).or_default() += 1;
    }

    let reconciliation = reconcile(&observed, &recorded, &reports);

    let mut summary = format!(
        "{} case(s) over {} spec file(s), {} skipped as not_implemented; {} agree, {} diverge",
        reports.len(),
        corpus.files_scanned,
        corpus.not_implemented,
        reports.len() - observed.len(),
        observed.len(),
    );
    summary.push_str(&format!(
        "\n     against the spec, tree-sitter: {tree_sitter}"
    ));
    summary.push_str(&format!("\n     against the spec, re2c:        {re2c}"));
    for (shape, count) in &shapes {
        summary.push_str(&format!("\n     {count:>4}  diverging: {shape:?}"));
    }
    match corpus.unclassifiable.first() {
        None => {}
        Some(_) => summary.push_str(&format!(
            "\n     {:>4}  chat block(s) with no expectation, UNTESTED: {}",
            corpus.unclassifiable.len(),
            corpus
                .unclassifiable
                .iter()
                .map(SpecLabel::to_string)
                .collect::<Vec<_>>()
                .join(", "),
        )),
    }

    match reconciliation.agrees_with_baseline() {
        true => Ok(summary),
        false => {
            let mut sections = reconciliation.render();
            sections.push(format!("Measured over: {summary}"));
            Err(report(sections))
        }
    }
}

/// Compare what diverges NOW against what the baseline records, and describe
/// every way the two can fail to line up.
///
/// One pass rather than three functions. The first cut had `new_divergences`
/// and `reshaped_divergences` as separate walks of `observed`, both asking
/// `recorded.get(label)` and differing only in which arm of that one `Option`
/// they handled, so "how a diverging case is presented" had two sites and could
/// be changed at one of them.
///
/// Each of the four sections is a distinct operator action, which is why they
/// are not merged into one list: paste an entry, delete an entry, delete a
/// STALE entry naming a spec that no longer exists, or look at a case whose
/// disagreement changed character.
fn reconcile(
    observed: &BTreeMap<String, DivergingCase<'_>>,
    recorded: &BTreeMap<&str, Divergence>,
    reports: &[CaseReport],
) -> Reconciliation {
    let mut appeared = Vec::new();
    let mut reshaped = Vec::new();
    for (label, diverging) in observed {
        let now = diverging.shape;
        match recorded.get(label.as_str()) {
            None => appeared.push(format!(
                "(\"{label}\", {now:?}),\n      {}",
                diverging.report.detail()
            )),
            Some(was) if *was == now => {}
            Some(was) => reshaped.push(format!(
                "{label}: baseline says {was:?}, now {now:?}\n      {}",
                diverging.report.detail()
            )),
        }
    }

    // Built only when something recorded is no longer diverging, which on a
    // green run is never: every recorded label is observed, so the whole set
    // would be allocated and thrown away.
    let mut fixed = Vec::new();
    let mut vanished = Vec::new();
    let unobserved: Vec<&&str> = recorded
        .keys()
        .filter(|label| !observed.contains_key(**label))
        .collect();
    match unobserved.first() {
        None => {}
        Some(_) => {
            let every_label: BTreeSet<String> = reports
                .iter()
                .map(|report| report.label.to_string())
                .collect();
            for label in unobserved {
                // Matched as an `Option`, not reduced to a bool: "the case
                // agrees now" and "there is no such case" are different
                // operator actions, and a bool would need a comment to say
                // which way round it read.
                match every_label.get(*label) {
                    Some(_) => fixed.push((*label).to_owned()),
                    None => vanished.push((*label).to_owned()),
                }
            }
        }
    }

    Reconciliation {
        appeared,
        fixed,
        vanished,
        reshaped,
    }
}

/// The four ways this run and the baseline can fail to line up.
///
/// Typed lists rather than rendered sections, because the first cut had
/// `reconcile` return `Vec<String>` and [`audit`] decide pass or fail by
/// whether that vector was empty. That is this module's OWN bug class,
/// reproduced one level down inside the gate that exists to forbid it: the
/// verdict became a property of presentation text, so a heading emitted for an
/// empty list would have turned a green run red, and a section built but never
/// pushed would have turned a red run green.
///
/// Now [`Reconciliation::agrees_with_baseline`] reads the lists and
/// [`Reconciliation::render`] is reached only on the failure path.
struct Reconciliation {
    /// Diverging now, absent from the baseline: a regression.
    appeared: Vec<String>,
    /// In the baseline, agreeing now: the entry is stale and should go.
    fixed: Vec<String>,
    /// In the baseline, naming a spec case that no longer exists at all.
    vanished: Vec<String>,
    /// Still diverging, but not in the recorded way.
    reshaped: Vec<String>,
}

impl Reconciliation {
    /// The verdict, read off the findings themselves.
    fn agrees_with_baseline(&self) -> bool {
        self.appeared.is_empty()
            && self.fixed.is_empty()
            && self.vanished.is_empty()
            && self.reshaped.is_empty()
    }

    /// Operator-facing text. `gate::report` drops the empty sections, so each
    /// heading is emitted unconditionally here.
    fn render(&self) -> Vec<String> {
        vec![
            listing(
                "NEW DIVERGENCES: the backends disagree here and the baseline does not say so.\n\
                 Fix the parser, or paste these into KNOWN_DIVERGENCES under the right family:",
                &self.appeared,
            ),
            listing(
                "RETIRED: listed as diverging, but the backends now agree.\n\
                 Delete these from KNOWN_DIVERGENCES in the commit that fixed them:",
                &self.fixed,
            ),
            listing(
                "STALE: listed as diverging, but no such spec case exists.\n\
                 The spec was renamed or deleted; delete the entry, it proves nothing:",
                &self.vanished,
            ),
            listing(
                "CHANGED SHAPE: still diverging, but not in the recorded way:",
                &self.reshaped,
            ),
        ]
    }
}

/// SURVIVES: policy. WHICH divergences this project has decided to ship with is
/// a set of judgements about real alternatives, so no type can hold the list.
/// What the types DO hold is that a divergence cannot be observed without being
/// classified ([`Divergence`] has no "unknown" variant, and
/// [`CaseReport::divergence`] is the only way to obtain one), and that findings
/// cannot be produced without a verdict ([`audit`] returns only a `Result`).
#[test]
fn backends_diverge_only_where_recorded() -> Result<(), String> {
    let summary = audit()?;
    println!("ok  re2c/tree-sitter spec parity: {summary}");
    Ok(())
}

/// Neither backend may panic on invalid input: every spec case must yield a
/// `ChatFile`, however malformed the input.
///
/// SURVIVES: behaviour a signature cannot describe. "Returns `ChatFile`" does
/// not promise "does not abort on the way there".
#[test]
fn re2c_never_panics_on_invalid_input() -> Result<(), String> {
    let corpus = load_spec_corpus()?;
    for case in &corpus.cases {
        let errors = ErrorCollector::new();
        let parsed = talkbank_parser_re2c::parser::parse_chat_file_streaming(&case.input, &errors);
        let _file = talkbank_model::model::ChatFile::from(&parsed);
    }
    println!(
        "ok  {} invalid input(s) parsed without panic",
        corpus.cases.len()
    );
    Ok(())
}
