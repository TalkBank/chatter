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

/// Every derive macro's compile-failure message, pinned against a fixture.
///
/// Surviving category: behaviour a signature cannot describe. What a proc macro
/// emits when it REJECTS its input is not in any type, and these are the only
/// tests that read it.
///
/// Ignored unless the `ui-tests` feature is on, because trybuild spawns a real
/// cargo build per fixture and costs about 16 seconds, over half of the whole
/// workspace's test execution. `just test-all` and CI turn it on; the inner
/// loop does not pay for it. Marked with `cfg_attr` rather than moved to its
/// own `[[test]]` target with `required-features`, because this crate keeps
/// ONE integration binary so tests stay selectable by name filter.
#[cfg_attr(
    not(feature = "ui-tests"),
    ignore = "trybuild spawns a cargo build per fixture (~16 s); enable with --features ui-tests"
)]
#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/pass_*.rs");
    t.compile_fail("tests/ui/fail_*.rs");
}
