//! Public entrypoint for parsing `%mor` tiers from CST nodes.
//!
//! This module re-exports the lower-level morphology parser used by
//! `ChatParser::parse_dependent_tier` and typed dispatch in the chat-file parser.
//! Its input is a generated `SourceBound<MorDependentTierNode>` from the
//! existing `ParsedSource` owner, not an independently supplied node/text pair.
//! The string-based morphology fragment APIs are unchanged.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>

pub use crate::parser::tier_parsers::mor::parse_mor_tier;
