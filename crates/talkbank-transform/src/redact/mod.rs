//! Structural CHAT-file sanitizer.
//!
//! Replaces supported lexical fields in a parsed `ChatFile` while
//! preserving timing, structure, speaker codes, and CHAT validity. The
//! output is intended for engineering use (debugging, validator
//! reproduction, structural analysis) where the original transcript is
//! protected by contributor consent. This is not a complete de-identification
//! guarantee: unsupported fields and preserved metadata require privacy review
//! before any disclosure.
//!
//! # Quick Start
//!
//! ```no_run
//! use talkbank_transform::redact::{sanitize, SanitizationPolicy};
//! # fn parse_input() -> talkbank_model::ChatFile { unimplemented!() }
//! let input = parse_input();
//! let policy = SanitizationPolicy::strict();
//! let sanitized = sanitize(input, &policy).expect("structurally sound");
//! let chat_text = sanitized.to_chat_string();
//! ```
//!
//! # What Is Preserved vs. Replaced
//!
//! See the chatter book chapter `user-guide/sanitize.md` for the full leak-surface inventory. The
//! short version:
//!
//! - **Preserved byte-exact**: timing bullets, `%wor` per-word bullets,
//!   speaker codes, `@Languages`, `@Birth`, `@Date`, `@Media`, `@PID`,
//!   `@L1Of`, structural markers (`+`, `~`, CA elements, `@n`, POS tags).
//! - **Replaced**: every `WordContent::Text`, `Shortening` text, `%wor`
//!   words (the paired main-tier word's placeholder when the tier
//!   corroborates the main tier, fresh ones otherwise; `wor.rs`), `%mor`
//!   lemmas, `%pho`/`%sin`/`%mod` tiers (dropped), free-text dependent
//!   tiers, the free-text header payloads listed in the sanitizer guide,
//!   `@Participants` names, `@ID` `custom_field`/`education`, free-text
//!   annotations.
//!
//! # Determinism + Idempotence
//!
//! Placeholders come from one monotonic counter advanced in document
//! order (`placeholder.rs`), so sanitizing the same input always produces
//! byte-identical output, and re-sanitizing a sanitized file reproduces
//! it.
//!
//! Out of scope for v1: speaker-code anonymization, `@Birth`/`@Date`
//! fuzzing, `@Media` filename redaction, audio-side sanitization,
//! unsanitize/round-trip mapping. See the chatter book chapter `user-guide/sanitize.md` for the full inventory.

mod dependent_tier;
mod document;
mod error;
mod header;
mod placeholder;
mod policy;
mod wor;
mod word;

/// Marker text emitted in place of redacted free-text content.
pub(crate) const REDACTED_TEXT: &str = "[redacted]";

pub use document::{SanitizedDocument, sanitize};
pub use error::RedactError;
pub use placeholder::{PlaceholderIndex, PlaceholderToken};
pub use policy::SanitizationPolicy;
