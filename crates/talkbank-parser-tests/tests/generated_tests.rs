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

//! Test module for generated tests in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

// Construct + parser-error tests generated from spec/constructs/ and
// spec/errors/ by the gen_rust_tests generator (run via the spec/tools binaries;
// see spec/CLAUDE.md). The generated bodies are included below.
//
// Validation-layer coverage (semantic errors E5xx, E6xx, E7xx) is NOT generated
// here: it is driven separately by gen_validation_corpus, which writes a `.cha`
// fixture corpus + manifest.json that the data-driven runner in
// validation_error_corpus.rs consumes. The reference-corpus roundtrip gate
// (tests/roundtrip_reference_corpus, must pass 100%) is the other half.

// Shared imports
use talkbank_parser::TreeSitterParser;

mod construct_tests {
    use super::*;
    include!("generated/generated_construct_tests_body.rs");
}

#[allow(unused_imports)]
mod error_tests {
    use super::*;
    include!("generated/generated_error_tests_body.rs");
}
