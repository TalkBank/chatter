//! Content identity carried with admitted diagnostic coordinates.

use serde::{Deserialize, Serialize};

/// Opaque identity of the exact UTF-8 source used to present a diagnostic.
/// Fixed-width deserialization refuses missing or malformed fingerprints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRevision {
    digest: [u8; 32],
}

impl SourceRevision {
    pub fn from_source(source: &str) -> Self {
        Self {
            digest: *blake3::hash(source.as_bytes()).as_bytes(),
        }
    }

    pub(crate) fn matches(self, source: &str) -> bool {
        self == Self::from_source(source)
    }
}
