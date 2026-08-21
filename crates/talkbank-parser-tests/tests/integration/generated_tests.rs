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

// Construct tests generated from spec/constructs/ by `just spec-gen`
// (see spec/tools/src/artifacts.rs; see spec/CLAUDE.md). The generated body
// is included below.
//
// ERROR-spec coverage is not generated here at all since R4: every error
// example is a fixture in the validation corpus (manifest + the data-driven
// runner in validation_error_corpus.rs, both stages against a real file),
// and the observation snapshot byte-pins the exact per-stage sets. The
// reference-corpus roundtrip gate (tests/roundtrip_reference_corpus, must
// pass 100%) is the other half.

// Shared imports
use talkbank_parser::TreeSitterParser;

mod construct_tests {
    use super::*;
    include!("generated/generated_construct_tests_body.rs");
}

// There is no `error_tests` module since R4. The string-based error tests
// asserted declared codes among PARSE diagnostics only, with no file context;
// every error example is now a fixture in the validation corpus (whose runner
// checks BOTH stages against a real file), and the observation snapshot
// byte-pins the exact per-stage sets. Deleting a weaker duplicate of two
// gated instruments is the R4 self-check's answer, not a coverage loss.
