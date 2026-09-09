//! Parsing for regular bracketed groups in main-tier content.
//!
//! The group hierarchy described in the manual (Scoped Symbols + Main Tier sections) uses `<...>` blocks
//! with optional nested content and annotations. This module parses the angle group over the typed
//! traversal, and owns the two pieces every bracketed construct shares: the `contents` slot reader
//! and the conversion of `UtteranceContent` into `BracketedItem`. Until 2026-09-08 it also held
//! `nested.rs`, a second `node.kind()` dispatcher over `content_item` children that the group,
//! quotation, pho and sin parsers all walked through; the shared typed walker replaced it.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#OverlapMarkers>

mod contents;
mod parser;

pub(crate) use contents::{contents_of, convert_to_group_content, parse_group_contents};
pub(crate) use parser::parse_group_content;
