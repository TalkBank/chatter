//! Supertype checking for tree-sitter grammar.
//!
//! Tree-sitter supertypes are abstract node categories that group related concrete types.
//! When a supertype is defined, tree-sitter returns the concrete type name, not the
//! supertype name. These helpers check if a node kind is one of the concrete subtypes.
//!
//! Generated from tree-sitter-talkbank grammar supertypes.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Terminators>

mod ca;
mod headers;
mod linkers;
mod overlap;
mod terminators;
mod tiers;

pub use headers::{is_header, is_pre_begin_header};
pub use linkers::is_linker;

#[cfg(test)]
mod tests;
