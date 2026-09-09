//! Helper functions for dispatching based on tree-sitter node kinds
//!
//! These functions convert tree-sitter nodes to model types using structural dispatch
//! instead of text parsing.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Pauses>
//! - <https://talkbank.org/0info/manuals/CHAT.html#OverlapMarkers>

// `ca` dispatches CA element and delimiter tokens through the GENERATED
// symbol tables; the word converter is its caller (word-internal tokens are
// the only place they occur). Overlap-point parsing lives in
// `main_tier/content/base`, shared by the standalone and word-internal token.
mod ca;
mod pause;
mod separator;

pub(crate) use ca::{parse_ca_delimiter_node, parse_ca_element_node};
pub(crate) use pause::parse_pause_node;
pub(crate) use separator::parse_separator_like;
