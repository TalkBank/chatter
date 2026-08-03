//! Re-export of the rule-selection type from the errors module.
//!
//! `validation::RuleSelection` and `errors::RuleSelection` are the same type;
//! the alias exists because callers reach for it from whichever module they
//! are already importing from.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

pub use crate::RuleSelection;
