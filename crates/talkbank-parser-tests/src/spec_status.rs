//! Does the enum agree with the specs about which checks are enforced?
//!
//! # What this replaced
//!
//! `chatter validate --list-checks` reports every error code as Active or
//! Planned. Until 2026-08-11 that came from a 43-entry list of code STRINGS
//! hand-written in the CLI, whose own doc comment said it "must be kept in sync
//! when a spec changes from `not_implemented` to `implemented`".
//!
//! It was not in sync. Measured against `spec/errors/*.md`, **15 of 225 codes
//! were reported wrongly, in both directions**: seven the specs mark
//! `not_implemented` were shipped as Active, and eight that are implemented
//! were shipped as Planned. The command whose only job is telling users which
//! checks run was wrong about 7% of them, and nothing anywhere would have said
//! so.
//!
//! The list now lives as `#[status(planned)]` on the variants themselves, so
//! it cannot name a code that does not exist and cannot misspell one. This gate
//! covers what the attribute cannot: that the annotation matches what the spec
//! actually says.
//!
//! # Why the default is safe
//!
//! Absence of the attribute means Active, and a silent default is normally the
//! hazard this workspace spends its time removing. It is safe here for one
//! reason only: this gate reads every spec and fails on any disagreement in
//! EITHER direction, so a missing attribute is as loud as a wrong one. Delete
//! this gate and the default becomes a bug generator.

use std::collections::{BTreeMap, BTreeSet};

use talkbank_model::{CheckStatus, ErrorCode};

use crate::error_specs::{self, SpecFile, SpecStatus};
use crate::gate::{Gate, GateOutcome, listing, report};
use crate::repo_paths::workspace_root;

/// This gate's POLICY over the shared vocabulary: only `not_implemented` means
/// unenforced.
///
/// `deprecated` and `unreachable_from_chat` are not "planned"; they are checks
/// that exist. That is the same judgement the CLI's old hand-written list made
/// by omitting them, and it is deliberately NOT baked into `SpecStatus`, which
/// two other callers read with different policies.
fn as_check_status(status: SpecStatus) -> CheckStatus {
    match status {
        SpecStatus::NotImplemented => CheckStatus::Planned,
        SpecStatus::Implemented
        | SpecStatus::Deprecated
        | SpecStatus::UnreachableFromChat
        | SpecStatus::Undeclared => CheckStatus::Active,
    }
}

/// What `spec/errors` says, including what it says inconsistently.
struct Declarations {
    status: BTreeMap<ErrorCode, CheckStatus>,
    /// Filenames naming no known code: not agreed, not disagreed, unresolved.
    unresolved: Vec<String>,
    /// One code, several specs, opposite answers.
    conflicting: Vec<String>,
}

/// Read every spec's declared status, keyed by code.
fn declared_statuses(specs: &[SpecFile]) -> Result<Declarations, String> {
    let mut seen: BTreeMap<ErrorCode, (&str, CheckStatus)> = BTreeMap::new();
    let mut unresolved = Vec::new();
    let mut conflicting = Vec::new();
    for spec in specs {
        // A file that declares nothing has nothing to compare, and that is the
        // majority: 105 of 239, plus `README.md` and `SPEC_ENHANCEMENT_GUIDE.md`
        // which are documentation rather than specs. Skipping them here is why
        // `unresolved` means "declared a status but names no code", which is a
        // real fault, rather than "is not an error spec", which is not.
        let declared = spec.status()?;
        if declared == SpecStatus::Undeclared {
            continue;
        }
        let status = as_check_status(declared);
        let Some(code) = spec.code() else {
            unresolved.push(spec.filename.clone());
            continue;
        };
        match seen.insert(code, (&spec.filename, status)) {
            // Several specs may describe one code; they must agree about
            // whether it is enforced. Inserting blind keeps whichever file the
            // directory listed last, which is how E342 came to be annotated
            // from a stale `_auto` stub while its real spec said the opposite.
            Some((first, before)) if before != status => conflicting.push(format!(
                "{}: {first} says {before:?}, {} says {status:?}",
                code.as_str(),
                spec.filename
            )),
            Some(_) | None => {}
        }
    }
    Ok(Declarations {
        status: seen
            .into_iter()
            .map(|(code, (_, status))| (code, status))
            .collect(),
        unresolved,
        conflicting,
    })
}

/// `#[status(planned)]` on `ErrorCode` must match `spec/errors/*.md`.
pub struct SpecStatusGate;

impl Gate for SpecStatusGate {
    fn name(&self) -> &'static str {
        "error-code status matches spec/errors"
    }

    fn check(&self) -> GateOutcome {
        let dir = workspace_root().join("spec/errors");
        let specs = error_specs::load(&dir)?;
        let Declarations {
            status: declared,
            unresolved,
            conflicting,
        } = declared_statuses(&specs)?;
        if declared.is_empty() {
            // An empty comparison is a broken gate, not a clean one, and the
            // two are indistinguishable in a passing log.
            return Err(format!("no spec statuses found under {}", dir.display()));
        }

        let mut should_be_planned = Vec::new();
        let mut should_be_active = Vec::new();
        for (code, spec_says) in &declared {
            match (code.check_status(), spec_says) {
                (CheckStatus::Active, CheckStatus::Planned) => should_be_planned.push(format!(
                    "{}: spec says not_implemented, enum says Active. Add #[status(planned)].",
                    code.as_str()
                )),
                (CheckStatus::Planned, CheckStatus::Active) => should_be_active.push(format!(
                    "{}: spec says implemented, enum says Planned. Remove #[status(planned)].",
                    code.as_str()
                )),
                (CheckStatus::Active, CheckStatus::Active)
                | (CheckStatus::Planned, CheckStatus::Planned) => {}
            }
        }

        // A variant marked planned whose spec is gone entirely: the attribute
        // is asserting something no spec supports.
        let declared_codes: BTreeSet<ErrorCode> = declared.keys().copied().collect();
        let orphaned: Vec<String> = ErrorCode::planned()
            .iter()
            .filter(|code| !declared_codes.contains(code))
            .map(|code| {
                format!(
                    "{}: marked #[status(planned)] but no spec declares a status for it.",
                    code.as_str()
                )
            })
            .collect();

        let sections = [
            listing(
                "Enum claims ACTIVE, spec says not_implemented:",
                &should_be_planned,
            ),
            listing(
                "Enum claims PLANNED, spec says implemented:",
                &should_be_active,
            ),
            listing("Marked planned with no spec to back it:", &orphaned),
            listing(
                "Spec filenames naming no known code (UNRESOLVED, not compared):",
                &unresolved,
            ),
            listing(
                "One code, several specs, opposite statuses. Fix the specs; the gate\n                 cannot pick a side and must not:",
                &conflicting,
            ),
        ];
        let failures = report(sections);
        if failures.is_empty() {
            return Ok(format!(
                "{} spec status(es) compared; {} planned, {} active",
                declared.len(),
                ErrorCode::planned().len(),
                declared.len() - ErrorCode::planned().len(),
            ));
        }
        Err(failures)
    }
}
