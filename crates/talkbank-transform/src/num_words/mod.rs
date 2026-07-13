//! Number-to-word normalization for CHAT content.
//!
//! Spells out digit tokens as language-appropriate number words so generated
//! CHAT satisfies E220 (numeric digits are not allowed in words for languages
//! that do not permit them). This is a general CHAT-generation utility: any
//! tool that emits digit-bearing content into CHAT (ASR pipelines, the
//! MICASE/SBCSAE converters, external generators) needs it.
//!
//! Coverage: 13 languages via lookup tables (`num2text`), CJK via
//! `num2chinese`, and English ordinals/decades/years via `ordinal_year_eng`.
//!
//! The batchalign-specific batch-detection hook (`detect_expansion`, which
//! routes tokens to a Python `num2words` IPC path) is deliberately NOT part of
//! this general utility; it stays in the ASR post-processor.

mod num2chinese;
mod num2text;
mod ordinal_year_eng;

pub use num2text::expand_number;
