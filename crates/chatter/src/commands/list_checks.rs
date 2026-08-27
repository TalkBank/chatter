//! Implementation of `chatter validate --list-checks`.
//!
//! Prints every known error code together with its implementation status
//! (Active vs Planned). This gives users and successors a machine-readable
//! view of which validation checks the running binary enforces and which
//! are only documented in `spec/errors/`.
//!
//! The status comes from `ErrorCode::check_status()`, read off the
//! `#[status(planned)]` attributes on the variants. Since R1 those attributes
//! are not authored: the enum is GENERATED from each code's `status` in
//! `spec/codes/error-codes.toml`, so this command cannot disagree with the
//! specs and there is no gate reconciling it. It used to be a hand-maintained
//! list of code strings in this file, which had drifted from the specs on 15
//! of 225 codes, in both directions; then an attribute plus a 180-line gate;
//! now one owner and neither.

use talkbank_model::{CheckStatus, ErrorCode};

/// Every error code the binary knows about, in code order.
///
/// Relies on `ErrorCode::iter()` from the `#[error_code_enum]` macro, which
/// guarantees one entry per variant AND, since the macro enforces ascending
/// declaration order, that the sequence is already sorted by code.
pub fn all_error_codes() -> Vec<ErrorCode> {
    ErrorCode::iter().copied().collect()
}

/// Print the list of all error checks with their status.
///
/// Output format is stable-ish but intended for human consumption. It is
/// deliberately NOT machine-parseable JSON, downstream tooling should read
/// the spec files directly instead.
pub fn print_check_list() {
    // No sort: the macro rejects a descending declaration, so `all()` is in
    // code order by construction. This used to be `sort_by_key(|c| c.as_str())`
    // under a doc comment claiming declaration order, which was a hand-written
    // re-derivation of an ordering the enum did not yet provide, and a
    // lexicographic one at that.
    let codes = all_error_codes();

    let active_count = codes
        .iter()
        .filter(|c| c.check_status() == CheckStatus::Active)
        .count();
    let planned_count = codes.len() - active_count;

    println!("Validation checks (Active / Planned):");
    println!();
    for code in &codes {
        let (badge, label) = match code.check_status() {
            CheckStatus::Active => ("[Active] ", "Active"),
            CheckStatus::Planned => ("[Planned]", "Planned"),
        };
        // Debug print of the variant gives the canonical Rust name
        // (e.g., `UnclosedBracket`) which is more informative than the
        // raw code alone.
        println!("  {}  {}  {:?}  ({})", badge, code.as_str(), code, label);
    }
    println!();
    println!(
        "Total: {} checks ({} Active, {} Planned)",
        codes.len(),
        active_count,
        planned_count
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_error_codes_is_nonempty() {
        assert!(!all_error_codes().is_empty());
    }

    // DELETED: `planned_codes_are_known_variants`. It checked that every
    // string in a hand-written list named a real variant. The list is gone;
    // `#[status(planned)]` sits on the variant itself, so an entry for a code
    // that does not exist, or a misspelled one, is now a compile error rather
    // than a test failure. The type obsoleted the test.

    /// SURVIVES: wiring. It asserts that `check_status` reads the emitted
    /// attribute at all, i.e. that BOTH arms are reachable, which no type
    /// states.
    ///
    /// It used to name three codes and their statuses, under a docstring
    /// calling that "policy". It was not policy: those are three copies of a
    /// per-code fact whose single owner, since R1, is
    /// `spec/codes/error-codes.toml`. Flipping any of them there would turn
    /// this red with nothing wrong, which is the second-copy shape
    /// `SpecStatusGate` was deleted for, one layer down, in the very change
    /// that deleted it. The wiring can be checked without naming a single
    /// code's status.
    #[test]
    fn both_check_status_arms_are_reachable() {
        let planned = ErrorCode::planned();
        assert!(
            !planned.is_empty(),
            "no code is marked planned, so the Planned arm is unreachable and \
             this command can only ever print Active"
        );
        assert!(
            planned.len() < ErrorCode::iter().len(),
            "every code is marked planned, so the Active arm is unreachable"
        );
        for code in planned {
            assert_eq!(code.check_status(), CheckStatus::Planned);
        }
        for code in ErrorCode::iter().filter(|code| !planned.contains(code)) {
            assert_eq!(code.check_status(), CheckStatus::Active);
        }
    }
}
