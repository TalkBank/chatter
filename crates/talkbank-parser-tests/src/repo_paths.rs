//! Shared runtime root for the parser audits and gates.
//!
//! Admission is owned by `talkbank-repo`; a missing checkout fails the gate
//! instead of letting an audit inspect an empty or unrelated tree.

use std::path::Path;

/// The admitted runtime checkout, shared for this test/audit process.
///
/// # Panics
/// When checkout admission fails. These source-based audits cannot run without it.
#[allow(clippy::expect_used)] // Test/audit infrastructure, never a shipped consumer.
pub fn workspace_root() -> &'static Path {
    talkbank_repo::root()
        .expect("runtime chatter checkout")
        .as_path()
}
