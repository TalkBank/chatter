//! What a gate must do to a tree somebody has broken on purpose.
//!
//! # The question this answers
//!
//! Every gate in [`ALL`](crate::gate::ALL) reports clean today. That is two
//! facts wearing one word: the repository is in order, and the gate can tell.
//! Only the first is observed. A gate whose scan reads no files, whose
//! comparison lost its right-hand side, or whose ratchet list went empty
//! reports the same clean summary, and that failure has shipped in this
//! workspace in four distinct spellings.
//!
//! A [`Probe`] is one planted violation and the verdict the gate owes on it.
//! Running the suite answers "can this gate fail, and does it fail for the
//! reason it names".
//!
//! # All three verdicts are load-bearing
//!
//! A [`Verdict::MustFail`] probe proves a rule fires. A [`Verdict::MustPass`]
//! probe proves a rule is not merely a blanket refusal: a gate that rejects
//! every tree passes every must-fail probe and is worthless. Every suite
//! carries [`Probe::control`], the unmodified tree, for exactly that reason,
//! and several carry a planted must-pass as well, which is the only way to
//! reach a sub-rule whose job is NOT to fire (a nested match that does not
//! implicate its parent, a helper call that clears a test).
//!
//! A [`Verdict::MustNotJudge`] probe proves the THIRD thing an
//! [`Outcome`](crate::gate::Outcome) can say. Until it existed the verdict had
//! two cases against an outcome that has three, so `Unavailable` was a probe
//! ERROR under both and no probe could assert that a gate correctly declines
//! to judge a tree it cannot read. That left the exact defect the third case
//! exists to prevent unprobeable: a precondition branch rewritten to report
//! clean, which is a gate skipping itself, kept every probe green.

use std::fmt;

use crate::gate::tree::Plant;
use crate::gate::{Outcome, Tier};

/// What the gate owes on the planted tree.
///
/// ONE VARIANT PER [`Outcome`] CASE, which is the property to preserve: a
/// verdict with fewer cases than the outcome it judges cannot express a
/// demand for the missing one, and the gap is silent because every probe still
/// passes. [`judge`] matches the two against each other as a PAIR with no
/// catch-all, so a new case on either side does not compile until every
/// pairing with it has been given a meaning.
///
/// The expected text lives INSIDE the variants that have one, so "a probe that
/// must pass and also names an expected failure" is not a value anybody can
/// write.
pub enum Verdict {
    /// The gate must report a failure whose text contains this.
    ///
    /// Not merely "must fail": a gate that fails for an incidental reason, a
    /// plant that broke a parse rather than tripping the rule, is not evidence
    /// about the rule under test.
    MustFail {
        /// A distinctive fragment of the failure the rule owes.
        naming: &'static str,
    },
    /// The gate must stay clean on the planted tree.
    MustPass,
    /// The gate must decline to judge, naming the input it is missing.
    ///
    /// Both fields are compared. The tier is half the fact: an absent input is
    /// a skip for a contributor with a filtered clone and a hard failure at the
    /// pre-push gate, and a precondition that named the wrong tier would turn
    /// the second into the first with nothing to notice. It is the field the
    /// two-case verdict left compared to nothing.
    MustNotJudge {
        /// A distinctive fragment of the precondition the gate owes.
        missing: &'static str,
        /// The tier from which that absence must stop being a skip.
        required_at: Tier,
    },
}

/// One planted violation, and the verdict the gate owes on it.
pub struct Probe {
    /// What is planted, in the words of the rule it tests.
    pub what: &'static str,
    /// What the gate owes.
    pub verdict: Verdict,
    /// How the tree is changed. Nothing here touches the disk.
    pub plant: Plant,
}

impl Probe {
    /// # These constructors are PRIVATE
    ///
    /// A `Probe` built by hand cannot enter a [`ProbeSuite`], whose fields are
    /// private and whose builder is the only way in, so a public constructor
    /// would be a route around the graph with no destination. Gates reach these
    /// through the suite, which is what makes "every suite has a control and at
    /// least one refusal" a fact about the type rather than about a check.
    ///
    /// A planted violation the gate must report, naming the rule's own text.
    #[must_use]
    fn must_fail(what: &'static str, naming: &'static str, plant: Plant) -> Self {
        Self {
            what,
            verdict: Verdict::MustFail { naming },
            plant,
        }
    }

    /// A planted tree the gate must still accept.
    #[must_use]
    fn must_pass(what: &'static str, plant: Plant) -> Self {
        Self {
            what,
            verdict: Verdict::MustPass,
            plant,
        }
    }

    /// A tree with an input REMOVED, which the gate must decline to judge.
    ///
    /// Distinct from `must_fail` in the way that matters most to an operator: a
    /// filtered clone is not a defect report, and a gate that answered one with
    /// the other would either accuse a sound checkout or pass a run that
    /// checked nothing.
    #[must_use]
    fn must_not_judge(
        what: &'static str,
        missing: &'static str,
        required_at: Tier,
        plant: Plant,
    ) -> Self {
        Self {
            what,
            verdict: Verdict::MustNotJudge {
                missing,
                required_at,
            },
            plant,
        }
    }

    /// The unmodified tree, which every suite carries.
    ///
    /// The control is what stops a suite proving only that the gate is capable
    /// of refusing something. It is also the assertion that the repository is
    /// currently in order, which is what `every_registered_gate_passes` says,
    /// said once more where the evidence is being weighed.
    ///
    /// Private: [`ProbeSuite`] adds it, so no gate can omit it and none has to
    /// remember to write it.
    fn control() -> Self {
        Self::must_pass("the unmodified tree", |_| Ok(()))
    }
}

/// A gate's probes: at least one refusal, and the unmodified tree.
///
/// # The two vacuous suites this type makes unwritable
///
/// `probes()` having no default body forces an author to say how the gate can
/// be made to fail. It does not force them to say anything true, and returning
/// `Vec<Probe>` left two suites that satisfy every check while establishing
/// nothing:
///
/// - `vec![]`, caught at runtime by an emptiness test.
/// - `vec![Probe::control()]`, caught by nothing. It has a `MustPass`, so the
///   "a gate that rejects everything" check passed it, and the run then
///   printed "1 probe(s) ... every planted violation was rejected" having
///   planted none. That is the module's own bug class, reproduced inside the
///   mechanism built to close it, at the one place a new gate's author works.
///
/// The cure is the constructor's signature. A suite BEGINS with a refusal,
/// stated as arguments rather than as an element that might not be there, and
/// the control is added here rather than by the caller. Both checks in
/// [`run_all`] are then unnecessary, which is the removal this type earned:
/// neither empty nor control-only is a value anyone can build.
pub struct ProbeSuite {
    probes: Vec<Probe>,
}

impl ProbeSuite {
    /// Begin a suite with the refusal the gate owes, plus the control.
    #[must_use]
    pub fn must_fail(what: &'static str, naming: &'static str, plant: Plant) -> Self {
        Self {
            probes: vec![Probe::control(), Probe::must_fail(what, naming, plant)],
        }
    }

    /// Another planted violation this gate must report.
    #[must_use]
    pub fn refusing(mut self, what: &'static str, naming: &'static str, plant: Plant) -> Self {
        self.probes.push(Probe::must_fail(what, naming, plant));
        self
    }

    /// A planted tree the gate must still accept.
    ///
    /// The only way to reach a sub-rule whose job is NOT to fire: a nested
    /// match that does not implicate its parent, a helper call that clears a
    /// test.
    #[must_use]
    pub fn accepting(mut self, what: &'static str, plant: Plant) -> Self {
        self.probes.push(Probe::must_pass(what, plant));
        self
    }

    /// A tree with an input removed, which the gate must decline to judge.
    #[must_use]
    pub fn declining(
        mut self,
        what: &'static str,
        missing: &'static str,
        required_at: Tier,
        plant: Plant,
    ) -> Self {
        self.probes
            .push(Probe::must_not_judge(what, missing, required_at, plant));
        self
    }
}

impl<'a> IntoIterator for &'a ProbeSuite {
    type Item = &'a Probe;
    type IntoIter = std::slice::Iter<'a, Probe>;

    fn into_iter(self) -> Self::IntoIter {
        self.probes.iter()
    }
}

/// A rule of a gate that no probe can reach, and why.
///
/// # Why this is declared rather than discovered
///
/// A probe suite is evidence about the rules it reaches, and silence about the
/// rest. Left implicit, the silence reads as coverage: a suite of five green
/// probes over a gate with nine rules looks exactly like a suite over a gate
/// with five. Naming the gap is the difference between "checked nothing" and
/// "found nothing", which is the distinction this whole module exists to
/// preserve.
///
/// The list is a ratchet, not a licence. `tests/integration/gates.rs` pins
/// every (gate, rule) PAIR in BOTH directions: a new entry fails until somebody
/// records it deliberately, and a rule that stops being declared fails until
/// its entry is deleted, so the record cannot rot into a permanent exemption
/// after the rule becomes probeable. It pinned only the COUNT until 2026-09-07,
/// which let a live hole be traded for a triviality with the total unmoved.
pub struct UnprovenRule {
    /// The rule, named the way the gate's own report names it.
    pub rule: &'static str,
    /// Why no plant reaches it. A compile-time input, a filesystem fault an
    /// overlay cannot express, or a direction the gate deliberately does not
    /// enforce.
    pub why: &'static str,
}

impl UnprovenRule {
    /// Record one unreachable rule.
    #[must_use]
    pub const fn new(rule: &'static str, why: &'static str) -> Self {
        Self { rule, why }
    }
}

impl fmt::Display for UnprovenRule {
    /// The rule and the reason, on two lines, never the rule alone.
    ///
    /// A rule without its `why` is a bare confession. The binary printed the
    /// short form and the integration test printed the long one, so the
    /// renderer a human reaches for while writing a probe showed strictly less
    /// than CI did; that is exactly the accidental variation [`listing`] was
    /// extracted to stop.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\n      {}", self.rule, self.why)
    }
}

/// What no probe of any registered gate reaches, for an operator to read.
///
/// One renderer, called by the binary and by the integration test, for the
/// reason `run_all` is shared between them: two hand-written copies of a
/// listing drift, and these two already had.
#[must_use]
pub fn unproven_listing() -> String {
    crate::gate::report(crate::gate::ALL.iter().map(|gate| {
        crate::gate::listing(
            &format!(
                "{}: {} rule(s) no probe reaches",
                gate.name(),
                gate.unproven_rules().len()
            ),
            gate.unproven_rules(),
        )
    }))
}

/// What one probe run established, in the words an operator reads.
///
/// Observations are the per-probe narration; problems are what must be acted
/// on. Two vectors rather than one list plus a predicate, because the caller
/// that PRINTS and the caller that FAILS want different halves and neither
/// should re-derive the other's.
pub struct ProbeReport {
    observations: Vec<String>,
    problems: Vec<String>,
    /// The evidence this run rests on: a witness issued by one of the planted
    /// trees a gate actually read. `None` means no gate reported clean over any
    /// tree, so there is nothing to say the run judged a real checkout.
    examined: Option<crate::gate::Examined>,
}

impl ProbeReport {
    /// Every probe that ran, and what it established.
    #[must_use]
    pub fn observations(&self) -> &[String] {
        &self.observations
    }

    /// The operator-facing result.
    ///
    /// One call, not a predicate beside a formatter: the renderer prints it
    /// and the gate IS it, so the two cannot disagree. That is the same shape
    /// `content_catch_alls::Audit::outcome` already uses, for the same reason.
    #[must_use]
    pub fn outcome(&self) -> Outcome {
        if !self.problems.is_empty() {
            return Outcome::failed(crate::gate::report(self.problems.clone()));
        }
        // The harness's own evidence is second-hand and says so: it is the
        // witness a gate handed back from a tree it read, carried here rather
        // than minted, so this run cannot claim to have judged a checkout it
        // never opened. Every suite carries a control probe, so a clean run
        // always has one.
        Outcome::clean_if_examined(
            format!(
                "{} probe(s) over {} gate(s): every planted violation was \
                 rejected, every accepted tree was accepted, and every \
                 unjudgeable tree was declined",
                self.observations.len(),
                crate::gate::ALL.len()
            ),
            self.examined,
        )
    }
}

/// One probe's verdict, and whatever evidence the gate handed back.
///
/// A struct rather than a tuple because the two halves answer different
/// questions and a worker returns many of them: `judged` is the operator's
/// line, `witness` is what the gate actually read.
struct Judged {
    /// `Ok` is the narration, `Err` is the problem. Both are one line each.
    judged: Result<String, String>,
    /// Present when the gate reported clean over its planted tree.
    witness: Option<crate::gate::Examined>,
}

/// Plant one probe and judge what the gate did with it.
fn run_one(gate: &dyn crate::gate::Gate, probe: &Probe, live: &crate::gate::Tree) -> Judged {
    // A PLANT failure is its own answer and never a gate failure: a probe
    // whose anchor text has moved says nothing about whether the gate can
    // fail, and reading the first as the second would accuse a sound gate of
    // being inert. Applying it here, rather than inside `judge`, is what
    // leaves `judge` a function of the verdict and the outcome alone, with no
    // tree it could read on the gate's behalf.
    let outcome = match live.planted(probe.plant) {
        Ok(planted) => gate.check(planted),
        Err(failed) => {
            return Judged {
                judged: Err(format!(
                    "{}: {}\n  THE PLANT DID NOT APPLY (this is a probe defect): {failed}",
                    gate.name(),
                    probe.what
                )),
                witness: None,
            };
        }
    };
    let witness = match &outcome {
        Outcome::Clean(verdict) => Some(verdict.examined()),
        Outcome::Failed(_) | Outcome::Unavailable(_) => None,
    };
    let judged = match judge(&probe.verdict, outcome) {
        Ok(verdict) => Ok(format!("ok  {}: {} [{verdict}]", gate.name(), probe.what)),
        Err(problem) => Err(format!("{}: {}\n  {problem}", gate.name(), probe.what)),
    };
    Judged { judged, witness }
}

/// Plant every probe of every registered gate, and judge the results.
///
/// Each probe is planted into a fresh overlay of the live checkout, so probes
/// cannot interact: several of these gates short-circuit on the first refusal,
/// and two plants at once would hide one of them behind the other.
///
/// # What this costs, measured, and what did NOT fix it
///
/// Every probe re-runs a whole-tree gate, and there are dozens: on 2026-09-08
/// the suite was 5.3 s of a 13.7 s `just test`, so the mechanism that proves
/// the gates work had nearly doubled the inner loop it protects.
///
/// Threads were tried and made `just test` WORSE, 13.7 s to 16.6 s, with system
/// time going from 27 s to 2 m 30 s. `cargo test` already runs the test
/// binaries in parallel, so a worker per core inside one of them oversubscribes
/// the machine, and every worker re-read the same 1,100 files. The STANDALONE
/// binary did get 4.5 times faster, 5.3 s to 1.2 s, which is exactly the
/// number that would have been reported as a win if the inner loop had not
/// been timed as a whole. It was reverted.
///
/// One source tree now serves every probe and carries a read cache its planted
/// copies share, so the corpus is read once per run rather than once per probe.
/// That took system time from 0.57 s to 0.14 s and the wall clock from 5.3 s to
/// 5.0 s, which is the honest verdict on it: the reads were never the cost.
/// The cost is per-probe DERIVATION, chiefly `blank_literals` over about 1,100
/// files for each of the hygiene probes, roughly 28,000 blankings a run. The
/// cure is to memoize the blanked text the way the content is memoized here,
/// which needs a key that a planted file invalidates and is a design change
/// rather than a tuning knob. Recorded rather than attempted at speed.
#[must_use]
pub fn run_all() -> ProbeReport {
    let live = crate::gate::Tree::live();
    let mut observations = Vec::new();
    let mut problems = Vec::new();
    let mut examined: Option<crate::gate::Examined> = None;

    for gate in crate::gate::ALL {
        // No "does this suite have a control" check, and no emptiness check:
        // `ProbeSuite`'s constructor supplies the first and requires a refusal,
        // so neither vacuous suite is a value that reaches here.
        for probe in &gate.probes() {
            let result = run_one(*gate, probe, &live);
            if let Some(witness) = result.witness {
                examined = Some(match examined {
                    Some(seen) => seen.and(witness),
                    None => witness,
                });
            }
            match result.judged {
                Ok(line) => observations.push(line),
                Err(problem) => problems.push(problem),
            }
        }
    }

    ProbeReport {
        observations,
        problems,
        examined,
    }
}

/// Compare what the gate said against what the probe says it owes.
///
/// The match is over the PAIR, with no catch-all, so the cross product of
/// verdicts and outcomes is enumerated and a new case on either side is a
/// compile error rather than a silently unprobeable state. That is why the
/// nine arms are written out even where three of them could be collapsed.
fn judge(verdict: &Verdict, outcome: Outcome) -> Result<String, String> {
    match (verdict, outcome) {
        (Verdict::MustFail { naming }, Outcome::Failed(text)) => {
            if text.contains(naming) {
                Ok("rejected, naming its rule".to_owned())
            } else {
                Err(format!(
                    "the gate failed, but not for the planted reason: its report \
                     does not contain {naming:?}.\n  report was:\n{text}"
                ))
            }
        }
        (Verdict::MustFail { naming }, Outcome::Clean(verdict)) => Err(format!(
            "INERT: the gate reported CLEAN over a planted violation it should \
             have named with {naming:?}.\n  it said: {verdict}"
        )),
        (Verdict::MustFail { .. }, Outcome::Unavailable(precondition)) => Err(format!(
            "NOT JUDGED: the gate could not read the planted tree \
             ({precondition}), so this probe establishes nothing either way."
        )),
        (Verdict::MustPass, Outcome::Clean(verdict)) => {
            Ok(format!("accepted, {}", verdict.examined()))
        }
        (Verdict::MustPass, Outcome::Failed(text)) => Err(format!(
            "the gate rejected a tree it must accept.\n  report was:\n{text}"
        )),
        (Verdict::MustPass, Outcome::Unavailable(precondition)) => Err(format!(
            "NOT JUDGED: the gate could not read the planted tree ({precondition})."
        )),
        (
            Verdict::MustNotJudge {
                missing,
                required_at,
            },
            Outcome::Unavailable(precondition),
        ) => {
            if !precondition.missing.contains(missing) {
                return Err(format!(
                    "the gate declined to judge, but not for the planted reason: \
                     its precondition does not name {missing:?}.\n  it said: {precondition}"
                ));
            }
            if precondition.required_at != *required_at {
                return Err(format!(
                    "the gate declined to judge and named the wrong TIER: the \
                     absence is required from {} upward and the probe expects \
                     {required_at}. A skip at the pre-push gate and a skip in \
                     the inner loop are different operator actions.",
                    precondition.required_at
                ));
            }
            Ok(format!("declined to judge, required at {required_at}"))
        }
        (Verdict::MustNotJudge { missing, .. }, Outcome::Clean(verdict)) => Err(format!(
            "THE GATE SKIPPED ITSELF: it reported CLEAN over a tree missing \
                 {missing:?}, which is a pass that checked nothing.\n  it said: \
                 {verdict}"
        )),
        (Verdict::MustNotJudge { missing, .. }, Outcome::Failed(text)) => Err(format!(
            "the gate reported a DEFECT over a tree that merely lacks \
             {missing:?}. A filtered clone is not a broken repository.\n  \
             report was:\n{text}"
        )),
    }
}
