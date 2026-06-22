//! Error code definitions and temporal validation constants.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

mod error_code;
/// Stable fingerprint of the active validation rule set (for cache keying).
mod rules_fingerprint;
/// Temporal/media bullet validation constants.
pub mod temporal;

pub use error_code::{ErrorCode, XPHON_ERROR_CODES};
pub use rules_fingerprint::validation_rules_fingerprint;
pub use temporal::*;
