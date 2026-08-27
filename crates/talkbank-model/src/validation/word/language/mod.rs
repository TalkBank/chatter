//! Word-language resolution helpers used by word validation.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker>
//!
//! This module is split so callers can reuse a single entrypoint
//! (`resolve_word_language`) while keeping digit-policy and marker-parsing logic
//! isolated in focused submodules.

mod digits;
mod governing;
mod helpers;
mod resolve;

#[cfg(test)]
mod tests;

pub(crate) use digits::check_word_digits_multi;
pub(crate) use governing::GoverningMarker;
pub use governing::{GoverningMark, GoverningMarkKind};
pub(crate) use helpers::mixed_language_allows_prefix_marker;
pub use resolve::{LanguageResolution, LanguageResolutionOutcome};
pub(crate) use resolve::{resolve_marker_at, resolve_without_marker};
