//! Module declarations and re-exports for this subsystem.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

mod convert;
mod error;
mod io;
mod parse;
mod validated;
pub use validated::{ValidatedParseError, parse_validated_with_parser};
pub(crate) mod rewrite;

pub use convert::{chat_to_json, chat_to_json_named, chat_to_json_unvalidated, normalize_chat};
pub use error::PipelineError;
pub use io::parse_file_and_validate;
pub use parse::{
    parse_and_validate, parse_and_validate_named, parse_and_validate_streaming,
    parse_and_validate_streaming_for_path, parse_and_validate_streaming_named,
    parse_and_validate_streaming_with_parser, parse_and_validate_with_parser,
};
pub use rewrite::{DroppedContent, Rewrite};
