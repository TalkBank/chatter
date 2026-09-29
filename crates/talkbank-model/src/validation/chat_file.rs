//! ChatFile validation entry points.
//!
//! Validation is performed via methods on [`crate::ChatFile`]. See
//! [`crate::ChatFile::validate`] for the compiler-checked streaming example and
//! [`crate::ChatFile::validate_with_alignment`] for alignment-aware validation.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
