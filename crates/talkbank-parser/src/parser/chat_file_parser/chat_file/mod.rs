//! Entry points for complete CHAT-file parsing.
//!
//! This directory provides both strict (`ParseResult`) and streaming (`ErrorSink`)
//! parse modes over the same CST traversal and recovery logic.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

mod document_lowering;
mod helpers;
mod lowering_lines;
pub(crate) mod normalize;
mod parse;
mod product;
pub(crate) mod replacement;
mod streaming;
#[cfg(test)]
mod tests;
pub(crate) mod tier_plan;
pub(crate) mod word_timing_plan;

pub use product::ParseProduct;
