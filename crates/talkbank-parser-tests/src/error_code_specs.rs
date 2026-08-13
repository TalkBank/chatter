//! Every declared error code has a spec file under `spec/errors/`.
//!
//! Why gates live in library modules rather than a binary's `main`: see
//! [`crate::gate`]. This one's own history is that it was
//! `error_coverage::test_error_code_spec_coverage`, the most deceptive member
//! of that family, because it genuinely RAN in CI and appeared in the passing
//! list while computing `missing_specs`, printing them, and asserting nothing.

use std::collections::BTreeSet;

use talkbank_model::ErrorCode;

use crate::gate::{Gate, GateOutcome, listing};
use crate::repo_paths::workspace_root;

/// The declared-code to spec-file correspondence.
pub struct ErrorCodeSpecGate;

impl Gate for ErrorCodeSpecGate {
    fn name(&self) -> &'static str {
        "error codes have spec files"
    }

    fn check(&self) -> GateOutcome {
        let spec_dir = workspace_root().join("spec").join("errors");

        // `ErrorCode::iter()` IS the declaration: the derive macro generates it
        // from the same `#[code("...")]` attributes this used to grep for as
        // TEXT, against a hardcoded path to `errors/codes/error_code.rs`. The
        // text scan also needed a guard against its own extraction silently
        // returning the empty set, which is a test guarding an invariant the
        // type already carries. Using the type deletes the path, the regex, the
        // guard, and the failure mode.
        let declared: BTreeSet<ErrorCode> = ErrorCode::iter().copied().collect();

        // The directory walk and the `<CODE>_<slug>.md` convention live in
        // `error_specs`, which owns the reason the split is on the FIRST
        // underscore: `starts_with` would let a hypothetical `E21` claim
        // `E210_auto.md` and report coverage it does not have. That reasoning
        // used to be written out here as well, which is two owners for one
        // rule and two places to change it.
        let specified = crate::error_specs::specified_codes(&crate::error_specs::load(&spec_dir)?);

        let missing: Vec<&str> = declared
            .difference(&specified)
            .map(ErrorCode::as_str)
            .collect();

        // NO exemption list, deliberately. There was one, with eleven entries,
        // and every single one was dead: eight named codes that are declared
        // AND have a spec, three named codes the model no longer declares, nine
        // of them under one comment reading "deprecated". Keeping the empty list
        // plus its both-directions machinery meant thirty-five lines that could
        // not execute. If an exemption is ever genuinely needed, copy the shape
        // from `HARNESS_CANNOT_TRIGGER` in
        // `spec/runtime-tools/tests/error_spec_codes.rs`: a stated reason, and a
        // check in both directions so a dead entry fails.
        if missing.is_empty() {
            return Ok(format!(
                "{} declared error code(s) all have specs",
                declared.len()
            ));
        }

        Err(listing(
            &format!(
                "FAIL: {} declared error code(s) have no spec file in {}.\n\
                 Write `<CODE>_<slug>.md`:",
                missing.len(),
                spec_dir.display()
            ),
            missing,
        ))
    }
}
