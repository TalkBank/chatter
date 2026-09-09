//! Public API for TalkBank parsing
//!
//! This module provides parsing APIs organized by level:
//! - `parser_api`, Fragment-aware parsing methods on `TreeSitterParser`
//! - `tiers`, Granular tier parsing modules
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

mod chat_parser_impl;
pub(crate) mod fragment;
mod parser_api;
mod parser_impl;
pub mod tiers;

// Re-export dependent tier parsing at module level
pub use tiers::*;
