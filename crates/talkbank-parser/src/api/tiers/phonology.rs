//! Public `%pho` parsing entrypoint from a source-bound CST node.
//!
//! `%pho` and `%mod` share the same internal item model (`PhoTier`) and this
//! module re-exports the `%pho` adapter used by dependent-tier dispatch.
//! Bind through the owning `ParsedSource`; internal source/reconstruction faults
//! return `CstFailure`. String-based fragment parser signatures are unchanged.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Model_Phonology>

pub use crate::parser::tier_parsers::pho::parse_pho_tier;
