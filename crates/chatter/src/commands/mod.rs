//! Command implementations for the CLI.
//!
//! Each command has its own module:
//! - `validate` - File and directory validation
//! - `normalize` - CHAT normalization
//! - `json` - JSON conversion (to-json, from-json)
//! - `alignment` - Alignment visualization
//! - `watch` - Continuous validation on file changes
//! - `clean` - Cleaned-text inspection
//! - `fix` - Apply catalog fixes at exact byte spans
//! - `cache` - Cache management (stats, clear)
//! - `debug` - Debug-family commands
//! - `list_checks` - `validate --list-checks` output
//! - `new_file` - Create new minimal valid CHAT files
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

pub mod adjudicate;
pub mod alignment;
pub mod batch;
pub mod cache;
pub mod clean;
pub mod debug;
mod dispatch;
mod error_codes;
pub mod fix;
pub mod json;
pub mod list_checks;
pub mod merge_preflight;
pub mod new_file;
pub mod normalize;
pub mod pipeline;
pub mod rediarize;
pub mod sanity_scan;
pub mod schema;
pub mod speaker_id;
pub mod transcript_merge;
pub mod update;
pub mod validate;
pub mod validate_parallel;
pub mod watch;

pub use alignment::show_alignment;
pub use clean::clean_file;
pub use dispatch::{CommandContext, dispatch_command};
pub use fix::run_fix;
pub use json::{chat_to_json, json_to_chat};
pub use new_file::create_new_file;
pub use normalize::normalize_chat;
pub use schema::run_schema;
pub use update::run_update;
pub use validate::validate_file;
pub use validate_parallel::{
    AlignmentValidationMode, CacheRefreshMode, RoundtripValidationMode, ValidationInterface,
};
pub use watch::watch_files;
