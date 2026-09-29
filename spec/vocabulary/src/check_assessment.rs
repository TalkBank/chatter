//! Shared CHECK assessment vocabulary; CHECK is evidence, not a language oracle.
use serde::Deserialize;

/// How chatter is expected to behave on a fixture relative to CLAN CHECK.
///
/// Internally tagged on the manifest's own `status` field and flattened into
/// [`ParityEntry`], so the PAYLOAD belongs to the variant that needs it.
///
/// `no_obligation_reason` used to be an `Option<NoObligationReason>` beside
/// `status`, which made two meaningless states representable, and the gate spent
/// two match arms and sixteen lines of prose rejecting them at test time.
/// Neither arm exists now, but the two states did NOT both become
/// unrepresentable, and the difference is worth knowing:
///
/// - A `no_obligation` entry with no reason is REFUSED while reading the file.
///   Verified by deleting a reason from the manifest and watching the load fail.
/// - A verdict entry carrying a stray `no_obligation_reason` still loads.
///   `#[serde(flatten)]` cannot be combined with `deny_unknown_fields`, so the
///   field is simply not deserialized into anything. Verified the same way: it
///   is accepted. It became UNREACHABLE rather than unrepresentable, which is a
///   real downgrade in severity (no code path can read it, so it cannot produce
///   a wrong verdict) but is not the same thing, and saying so is the point.
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ParityStatus {
    /// chatter flags an equivalent error: `expected_chatter_codes` must appear.
    Parity,
    /// CLAN flags it but chatter does not yet: the fixture must validate clean.
    Gap,
    /// Intentional divergence from CLAN, in one of two shapes:
    /// - chatter rejects differently or more strictly: `expected_chatter_codes`
    ///   lists the codes it must emit (asserted present, like `Parity`).
    /// - chatter intentionally ACCEPTS what CLAN rejects, because the CHECK code
    ///   is a CLAN-internal concern rather than a CHAT-validity rule (e.g. CHECK
    ///   109, a postcode on a dependent tier: CLAN's own analysis tools do not
    ///   choke on it, only CHECK flags it): `expected_chatter_codes` is empty and
    ///   the fixture must validate clean. Unlike `Gap`, this clean state is a
    ///   permanent intentional choice, not a defect to close.
    Divergence,
    /// Unix-build CLAN CHECK cannot emit this code at the current synced CLAN
    /// HEAD, so there is nothing to reach parity WITH. WHY it cannot is carried
    /// by [`NoObligationReason`], not by prose in `note`, because two of the
    /// four sub-shapes are checkable against the generated CLAN reference and
    /// free text is not.
    ///
    /// A fixture is OPTIONAL here (the only status where it is): when present
    /// it pins the construct anyway. The CI gate asserts chatter's recorded
    /// behaviour on it (`expected_chatter_codes` present, or clean if empty),
    /// and the CLAN-gated grounding test asserts the code is still NOT
    /// emitted, a tripwire that fires if a future CLAN bundle revives the
    /// rule and the entry needs re-adjudication.
    NoObligation {
        /// Why unix CLAN cannot emit it. Required by construction here: an
        /// entry cannot reach this variant without stating a checkable reason.
        no_obligation_reason: NoObligationReason,
    },
}

/// Why unix-build CLAN CHECK cannot emit a `NoObligation` code.
///
/// This is a typed discriminant rather than a sentence in `note` because the
/// first two variants are DERIVABLE from `check.cpp` and can therefore be
/// gated: the generated reference records how many live `check_err` call sites
/// each code has, so "the source no longer emits this" is a fact a test can
/// check instead of a claim a reader has to trust. The last two are not
/// derivable from source text and rest on the empirical grounding run.
///
/// The distinction is not academic. Before 2026-08-11 the reference generator
/// stripped `//` line comments but not `/* ... */` blocks, so ELEVEN entries
/// asserted "call site commented out" while the reference still reported live
/// call sites, and nothing compared the two. CHECK 76's retirement (an upstream
/// `/* 2026-08-07 */` block) would have been the twelfth.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum NoObligationReason {
    /// The `check_err(N, ...)` call site is commented out in `check.cpp`.
    /// DERIVABLE: the reference must show zero live call sites for this code.
    CommentedOut,
    /// No emission path exists anywhere in `clan/` or `lib/`: the code is
    /// defined in the message switch but nothing ever raises it.
    /// DERIVABLE: the reference must show zero live call sites for this code.
    NoEmissionPath,
    /// The call site is compiled, but unreachable in file mode because an
    /// earlier check preempts it or the stock depfile's pattern shapes exclude
    /// it. NOT derivable from source text: the call site is real, so the
    /// reference still counts it; only running CLAN can show it never fires.
    UnreachableInFileMode,
    /// Every call site lives in an `#ifndef UNX` region, so the code exists
    /// only in the GUI builds; unix CHECK is the authoritative parity bar (see
    /// `clan-check-reference/gui-vs-unix-check.md`).
    /// DERIVABLE since 2026-08-11: the generator runs `unifdef -DUNX` before
    /// scanning, so those sites are blanked and the reference reports none.
    /// Note the boundary this draws, which CHECK 151 sits on the far side of:
    /// this means the SITE is excluded, not merely that the code is unreachable
    /// in a unix run. A site that compiles but whose only caller is excluded is
    /// [`Self::UnreachableInFileMode`].
    GuiOnly,
}

/// One CHECK number grounded against a fixture.
#[derive(Deserialize)]
pub struct ParityEntry {
    pub check_code: u16,
    /// Verbatim CLAN CHECK message text (without the trailing `(NN)`),
    /// used for documentation and, when `no_numeric_suffix` is set, as
    /// the grounding match target.
    #[serde(default)]
    pub check_message: String,
    /// The grounding fixture under `tests/check_parity/fixtures/`. Required
    /// for every status except `NoObligation` (where CLAN cannot emit the
    /// code, so there may be no construct to ground); both tests fail closed
    /// on a fixture-less entry of any other status.
    pub fixture: Option<String>,
    #[serde(flatten)]
    pub status: ParityStatus,
    #[serde(default)]
    pub expected_chatter_codes: Vec<String>,
    /// Extra CLAN CHECK command-line flags this fixture's grounding
    /// requires (e.g. `+c0` for option-gated rules); empty for
    /// default-mode rules. The CLAN-gated grounding test passes these
    /// through; the CI-side test ignores them.
    #[serde(default)]
    pub clan_flags: Vec<String>,
    /// True for the rare CHECK messages printed WITHOUT a trailing
    /// `(NN)` (e.g. code 51, whose format string lacks `(%d)`); the
    /// grounding test then matches the message text instead.
    #[serde(default)]
    pub no_numeric_suffix: bool,
    /// True for codes whose unix-build CLAN prints an EMPTY message
    /// line (code 59: `check_mess`'s printf sits behind
    /// `#if _MAC_CODE/#elif _WIN32`), so neither a `(NN)` trailer nor
    /// message text ever appears. Grounding then relies on the
    /// `THERE WERE SOME ERROR(S) FOUND` banner, which is sound ONLY
    /// when `clan_flags` is exactly `+e<check_code>` (CHECK restricted
    /// to this one code); the grounding test enforces that flag shape
    /// and fails the entry otherwise (fail closed, never vacuous).
    #[serde(default)]
    pub banner_only: bool,
    #[serde(default)]
    pub note: String,
}

/// The single authored CHECK assessment record, shared by reports and tests.
#[derive(Deserialize)]
pub struct ParityManifest {
    /// Scope and acceptance policy, rendered verbatim in the book.
    pub policy: AssessmentPolicy,
    /// Adjudicated CHECK obligations.
    pub entries: Vec<ParityEntry>,
}

/// Human policy is authored once; status and counts are derived from entries.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssessmentPolicy {
    /// Public explanation for CHAT users.
    pub user_summary: String,
    /// What the finite baseline does and does not cover.
    pub scope: String,
    /// Conditions that justify reopening an obligation.
    pub reopening: String,
    /// Architecture and evidence rules for developers.
    pub developer_contract: String,
}

impl ParityStatus {
    /// Stable manifest label, shared by reports.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Parity => "parity",
            Self::Gap => "gap",
            Self::Divergence => "divergence",
            Self::NoObligation { .. } => "no_obligation",
        }
    }
}

/// Only a nonempty, uniquely identified inventory can be assessed.
pub struct AssessedInventory<'a> {
    manifest: &'a ParityManifest,
    gaps: usize,
}

impl ParityManifest {
    /// Refuse malformed inventories before deriving a completion claim.
    pub fn assess(&self) -> Result<AssessedInventory<'_>, &'static str> {
        if [
            &self.policy.user_summary,
            &self.policy.scope,
            &self.policy.reopening,
            &self.policy.developer_contract,
        ]
        .iter()
        .any(|s| s.trim().is_empty())
        {
            return Err("missing CHECK assessment policy");
        }
        if self.entries.is_empty() {
            return Err("empty CHECK inventory");
        }
        let mut ids = std::collections::BTreeSet::new();
        for entry in &self.entries {
            if !ids.insert((entry.check_code, entry.fixture.as_deref())) {
                return Err("duplicate CHECK obligation and fixture");
            }
            if entry.note.trim().is_empty() {
                return Err("missing adjudication rationale");
            }
            if !matches!(entry.status, ParityStatus::NoObligation { .. })
                && entry.fixture.as_ref().is_none_or(|f| f.trim().is_empty())
            {
                return Err("missing obligation fixture");
            }
        }
        Ok(AssessedInventory {
            manifest: self,
            gaps: self
                .entries
                .iter()
                .filter(|e| e.status == ParityStatus::Gap)
                .count(),
        })
    }
}

impl AssessedInventory<'_> {
    /// Unresolved, independently justified validation gaps.
    pub const fn gaps(&self) -> usize {
        self.gaps
    }
    /// All entries, including resolved divergences and exclusions.
    pub fn total(&self) -> usize {
        self.manifest.entries.len()
    }
    /// Completion refers only to this inventoried assessment.
    pub const fn is_complete(&self) -> bool {
        self.gaps == 0
    }
}
