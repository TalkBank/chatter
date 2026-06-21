//! Conversion from our AST to `talkbank-model` types.
//!
//! Each conversion is built from studying the actual JSON output of the
//! TreeSitterParser (via `chatter to-json`), ensuring semantic equivalence.
//!
//! Rules:
//! - No dummy/sentinel values. Every model field must be correct or absent.
//! - No silent drops. Every AST item must be converted or produce an error.
//! - No panics. All conversions are infallible for valid AST input.

mod headers;
mod items;
mod text_tiers;
mod tiers;

pub use headers::*;
pub use items::*;
pub use text_tiers::*;
pub use tiers::*;
