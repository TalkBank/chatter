//! Error code definitions and temporal validation constants.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

/// The per-code facts that are not generated: [`CheckStatus`] and the Phon
/// `%x` group.
mod error_code;
/// GENERATED from `spec/codes/error-codes.toml`: the `ErrorCode` enum. Do not
/// hand-edit; change the registry and run `just spec-gen`.
mod generated_error_code;
/// Stable fingerprint of the active validation rule set (for cache keying).
mod rules_fingerprint;
/// Temporal/media bullet validation constants.
pub mod temporal;

pub use error_code::{CheckStatus, XPHON_ERROR_CODES};
pub use generated_error_code::ErrorCode;
pub use rules_fingerprint::validation_rules_fingerprint;
pub use temporal::*;
