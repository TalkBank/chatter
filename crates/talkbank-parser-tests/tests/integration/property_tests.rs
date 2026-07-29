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

//! Property-based tests for CHAT parser
//!
//! These tests use proptest to verify properties that should hold for ALL inputs,
//! not just hand-picked examples. This finds edge cases that example-based tests miss.
//!
//! **IMPORTANT**: These tests run on BOTH TreeSitterParser and TreeSitterParser
//! to ensure behavioral equivalence through the ChatParser API.
//!
//! This verifies that both parser implementations produce identical results
//! for the same inputs, catching any divergences in behavior.

// proptest macro module imports are pulled in via individual test modules

// Import the property_tests_modules which contains all the actual tests
mod property_tests_modules;
