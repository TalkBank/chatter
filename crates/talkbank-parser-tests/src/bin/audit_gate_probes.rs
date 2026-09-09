//! Plant a violation into every registered gate and report what each did.
//!
//! A renderer, nothing else. The probes, the plants and the judging live in
//! `talkbank_parser_tests::gate::probe`; the GATE that fails CI is
//! `tests/integration/gates.rs`, which asserts on the same `ProbeReport` this
//! prints, so the two cannot disagree.
//!
//! It exists because the per-probe narration is what a human wants when
//! writing or re-anchoring a probe, which is the one thing a test assertion is
//! bad at: a passing test prints nothing, and a failing one prints only the
//! probes that went wrong.
//!
//! Nothing here writes to the repository. Every plant lands in an in-memory
//! overlay of the checkout.
//!
//! Usage:
//!   cargo run -p talkbank-parser-tests --bin audit_gate_probes

use std::process::ExitCode;

use talkbank_parser_tests::gate::{run_all, unproven_listing};

fn main() -> ExitCode {
    println!("{}", unproven_listing());
    println!();

    let report = run_all();
    for observation in report.observations() {
        println!("{observation}");
    }
    println!();

    report.outcome().into_exit_code()
}
