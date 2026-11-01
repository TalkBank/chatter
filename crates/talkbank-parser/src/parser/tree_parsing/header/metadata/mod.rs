//! Metadata header parsing (`@Languages`, `@Media`, `@Types`). `@PID` is a
//! text header, read by the shared text-header reader.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Types_Header>

mod id_languages;
mod languages;
mod media;
mod types;

pub(crate) use id_languages::parse_id_languages;
pub use languages::parse_languages_header;
pub(crate) use languages::recover_languages_header;
pub use media::parse_media_header;
pub use types::parse_types_header;
