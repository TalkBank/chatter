//! Shared helpers for single-item parse entry points.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

/// Minimal valid CHAT file headers required for parsing isolated items
///
/// A truly minimal valid CHAT file must have:
/// 1. @UTF8
/// 2. @Begin
/// 3. @Languages: (at least one language)
/// 4. @Participants: (at least one participant)
/// 5. @ID: (matching the participant)
/// 6. @End
pub(crate) const MINIMAL_CHAT_PREFIX: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n";
/// Closing `@End` marker that terminates the synthetic wrapper document.
pub(crate) const MINIMAL_CHAT_SUFFIX: &str = "@End\n";
