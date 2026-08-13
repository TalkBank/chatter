//! What one spec case looks like once both backends have been run over it.
//!
//! Split out of `error_parity.rs` when that file passed the workspace's 800
//! line hard limit. The types travel together because every one of them exists
//! to make some illegal state unrepresentable in [`CaseReport`], and the
//! reasoning for each is in its own doc comment.

use std::collections::BTreeSet;
use std::fmt;

use talkbank_model::ErrorCode;

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

/// Which spec case a measurement is about: a file, plus an index when the file
/// holds more than one example.
///
/// A type rather than a formatted string so the "suffix only when the file has
/// several examples" rule lives at ONE construction site. It had two, and the
/// second existed only to re-derive labels for a debug pass.
#[derive(Clone, Debug)]
pub(super) struct SpecLabel {
    pub(super) file: String,
    /// `None` when the file holds exactly one example.
    pub(super) case: Option<usize>,
}

impl SpecLabel {
    /// Label the `index`th example of a file holding `in_file` examples.
    pub(super) fn new(file: &str, index: usize, in_file: usize) -> Self {
        Self {
            file: file.to_owned(),
            case: match in_file {
                1 => None,
                _ => Some(index),
            },
        }
    }
}

impl fmt::Display for SpecLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.case {
            Some(index) => write!(f, "{}#{index}", self.file),
            None => f.write_str(&self.file),
        }
    }
}

/// A set of diagnostic codes that CANNOT be empty.
///
/// One non-emptiness proof with two holders ([`Expected`] and
/// [`Reported::Spoke`]), because the first draft re-proved it in each with its
/// own `.first()?`. It had a third holder, `Conformance::Misses { absent }`,
/// which could hold the empty set: not "missed some expected codes" but exactly
/// [`Conformance::Meets`], one state with two representations. That payload has
/// since gone, for the separate reason that nothing read it.
///
/// # Every way to obtain one
///
/// [`Codes::new`] is the only constructor, the field is never written from
/// outside it, and there is no `Default`: an empty `Codes` is unrepresentable
/// rather than merely discouraged. That is what lets its `Display` have no empty
/// case and lets every holder skip the "and what if it is empty" branch.
#[derive(Clone, Debug)]
pub(super) struct Codes(BTreeSet<ErrorCode>);

impl Codes {
    /// `None` for the empty set, which is a different fact and gets a different
    /// type at every call site: no expectation, or a silent backend.
    pub(super) fn new(codes: BTreeSet<ErrorCode>) -> Option<Self> {
        codes.first()?;
        Some(Self(codes))
    }

    pub(super) fn as_set(&self) -> &BTreeSet<ErrorCode> {
        &self.0
    }
}

/// For an operator to read, never for a machine to read back.
impl fmt::Display for Codes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            &self
                .0
                .iter()
                .map(|code| code.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        )
    }
}

/// The codes a spec says its input must produce.
///
/// A wrapper over [`Codes`] for two reasons, and the swap-safety one DOES hold,
/// contrary to an earlier draft of this comment. `Reported` is an enum, but
/// `Reported::Spoke(codes)` hands out a `&Codes`, so without this wrapper
/// [`Conformance::of`] would take two interchangeable `&Codes` and the compiler
/// could not tell the expectation from the observation.
///
/// Second: [`Expected::new`] returns `Option`, which is how "this block
/// declares no codes, so it is untestable" travels from the spec reader to the
/// UNTESTED line of the report instead of becoming a case that passes by
/// expecting nothing.
#[derive(Clone, Debug)]
pub(super) struct Expected(Codes);

impl Expected {
    /// `None` when the spec declared no codes. An expectation of nothing is not
    /// a test, so such a block is reported as untested rather than counted as
    /// a pass.
    pub(super) fn new(codes: BTreeSet<ErrorCode>) -> Option<Self> {
        Codes::new(codes).map(Self)
    }
}

impl fmt::Display for Expected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// What one backend actually said about one input.
///
/// Silence is a VARIANT, not an empty collection, so "reported nothing" cannot
/// be forgotten by a caller who only inspects the codes it finds.
#[derive(Clone, Debug)]
pub(super) enum Reported {
    /// No diagnostic at all, on input a spec says is invalid.
    Silent,
    /// At least one diagnostic.
    Spoke(Codes),
}

impl Reported {
    pub(super) fn of(codes: BTreeSet<ErrorCode>) -> Self {
        match Codes::new(codes) {
            None => Self::Silent,
            Some(codes) => Self::Spoke(codes),
        }
    }
}

impl fmt::Display for Reported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Silent => f.write_str("(silent)"),
            Self::Spoke(codes) => codes.fmt(f),
        }
    }
}

/// What one backend did about one spec's expectation.
///
/// Three states, not two booleans. The previous version carried
/// `has_expected` and `has_any_error` side by side, which admits the impossible
/// pair (`has_expected` true, `has_any_error` false) and took a five-branch
/// `if`/`else` chain ending in a catch-all to read back out.
#[derive(Clone, Debug)]
pub(super) enum Conformance {
    /// Reported every expected code.
    Meets,
    /// Reported diagnostics, but not all the expected ones.
    ///
    /// This variant used to carry the absent set, on the argument that "missed
    /// the expectation" and "missed E316 specifically" are different amounts of
    /// information and the second was free. Nothing ever read it, and the only
    /// thing that made it look used was a derived `PartialEq` that nothing read
    /// either; dropping that derive turned the field into a compiler warning
    /// within the minute. The information is not lost: `CaseReport::detail`
    /// prints both backends' full code sets beside the expectation, which is
    /// strictly more than the absent set and is what an operator reads.
    Misses,
    /// Reported nothing at all.
    Silent,
}

impl Conformance {
    pub(super) fn of(expected: &Expected, reported: &Reported) -> Self {
        match reported {
            Reported::Silent => Self::Silent,
            // The set difference decides the variant directly, so there is no
            // separate emptiness test that could disagree with it.
            Reported::Spoke(codes) => match expected.0.as_set().difference(codes.as_set()).next() {
                None => Self::Meets,
                Some(_) => Self::Misses,
            },
        }
    }
}

/// How ONE backend fared against the whole spec suite.
///
/// This replaced an `Outcome` enum that collapsed the nine
/// `(Conformance, Conformance)` pairs into five joint buckets. Two of those
/// five, `Re2cSilent` and `TreeSitterSilent`, were the SAME predicate as the
/// [`Divergence`] variants of the same names, computed a second time from the
/// same fields, and the summary duly printed "11" on two adjacent lines. One
/// fact, two owners.
///
/// Splitting the axes costs less code and says more: how each backend does
/// against the spec on its own is a question the joint buckets could not
/// answer, and "how often does tree-sitter fail its OWN spec" is worth knowing.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ConformanceTally {
    pub(super) meets: usize,
    pub(super) misses: usize,
    pub(super) silent: usize,
}

impl ConformanceTally {
    /// Fold conformances into counts. The exhaustive match is the ONLY place a
    /// field is incremented, so three same-typed counters cannot be crossed
    /// without editing the arm that names them.
    pub(super) fn of(conformances: impl Iterator<Item = Conformance>) -> Self {
        let mut tally = Self::default();
        for conformance in conformances {
            match conformance {
                Conformance::Meets => tally.meets += 1,
                Conformance::Misses => tally.misses += 1,
                Conformance::Silent => tally.silent += 1,
            }
        }
        tally
    }
}

impl fmt::Display for ConformanceTally {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:>4} meet the spec, {:>3} miss it, {:>3} SILENT on invalid input",
            self.meets, self.misses, self.silent
        )
    }
}

/// How the two backends differ on one case. `None` from
/// [`CaseReport::divergence`] means they produced identical diagnostics, the
/// only result this gate accepts without a baseline entry.
///
/// The three "both spoke" variants are separated because they are three
/// different jobs, and a single `DifferentCodes` bucket hid that: 88 of the 99
/// divergences found on 2026-08-09 were in it, spanning "re2c has not
/// implemented this rule", "re2c over-reports" and "re2c names a different
/// code for the same fault", which no reader could tell apart from the entry.
///
/// Deliberately payload-free: this value is written by hand into
/// [`super::baseline::KNOWN_DIVERGENCES`], so it has to stay short enough to
/// type. The codes
/// themselves go in the printed detail line, where a reader wants them and no
/// maintainer has to keep them current.
///
/// Rendered through the derived `Debug`, which for a fieldless enum already
/// emits the variant's own identifier. A hand-written `Display` did the same
/// job by listing all five names again, which is a second place to make a typo
/// and produce a baseline line that does not compile when pasted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Divergence {
    /// re2c reported nothing where tree-sitter reported something.
    Re2cSilent,
    /// tree-sitter reported nothing where re2c reported something.
    TreeSitterSilent,
    /// re2c reported a strict subset: tree-sitter caught more.
    Re2cIncomplete,
    /// re2c reported everything tree-sitter did, and more besides.
    Re2cExtra,
    /// Each backend reported something the other did not.
    Conflicting,
}

/// One spec case, run through both backends.
///
/// Four recorded facts and no stored verdict: everything the report says about
/// a case is a method over these fields.
pub(super) struct CaseReport {
    pub(super) label: SpecLabel,
    pub(super) expected: Expected,
    pub(super) tree_sitter: Reported,
    pub(super) re2c: Reported,
}

impl CaseReport {
    pub(super) fn tree_sitter_conformance(&self) -> Conformance {
        Conformance::of(&self.expected, &self.tree_sitter)
    }

    pub(super) fn re2c_conformance(&self) -> Conformance {
        Conformance::of(&self.expected, &self.re2c)
    }

    /// The question this crate exists to answer, and the one the old audit
    /// never asked: did the two backends produce the SAME diagnostics?
    ///
    /// The both-spoke arm reads the two set differences rather than an equality
    /// test, so the four ways they can relate fall out as an exhaustive match
    /// and the identical case needs no separate pre-check.
    pub(super) fn divergence(&self) -> Option<Divergence> {
        match (&self.tree_sitter, &self.re2c) {
            (Reported::Silent, Reported::Silent) => None,
            (Reported::Spoke(_), Reported::Silent) => Some(Divergence::Re2cSilent),
            (Reported::Silent, Reported::Spoke(_)) => Some(Divergence::TreeSitterSilent),
            (Reported::Spoke(tree_sitter), Reported::Spoke(re2c)) => {
                let only_tree_sitter = tree_sitter.as_set().difference(re2c.as_set()).next();
                let only_re2c = re2c.as_set().difference(tree_sitter.as_set()).next();
                match (only_tree_sitter, only_re2c) {
                    (None, None) => None,
                    (Some(_), None) => Some(Divergence::Re2cIncomplete),
                    (None, Some(_)) => Some(Divergence::Re2cExtra),
                    (Some(_), Some(_)) => Some(Divergence::Conflicting),
                }
            }
        }
    }

    /// The operator-facing detail line for a diverging case.
    pub(super) fn detail(&self) -> String {
        format!(
            "tree-sitter [{}] vs re2c [{}]; spec expects [{}]",
            self.tree_sitter, self.re2c, self.expected,
        )
    }
}

/// A case that DOES diverge, and the shape of its divergence.
///
/// # Every way to obtain one
///
/// [`DivergingCase::of`] is the only constructor, and it can only succeed by
/// asking the report itself, so the shape cannot disagree with the case it
/// describes and no downstream code has to re-handle the `None` that would mean
/// "this agreeing case is in the divergence list".
///
/// It borrows the report rather than copying a rendered detail line beside the
/// shape, which is what the first draft did: two representations of one fact,
/// the second able to go stale the moment anything about the case changed.
pub(super) struct DivergingCase<'a> {
    pub(super) report: &'a CaseReport,
    pub(super) shape: Divergence,
}

impl<'a> DivergingCase<'a> {
    pub(super) fn of(report: &'a CaseReport) -> Option<Self> {
        Some(Self {
            shape: report.divergence()?,
            report,
        })
    }
}
