//! Sanitization policy.
//!
//! v1 ships exactly one preset, [`SanitizationPolicy::strict`], whose
//! coverage is documented in the book's sanitizer guide. The type is a unit
//! marker rather than a struct of toggles, bundling field-level
//! booleans here would be boolean-blindness for a single preset and
//! would lock the API into a shape future variants are unlikely to
//! want. When v2 adds a second preset, replace this type with an
//! `enum` and dispatch from there.

/// Marker for the active sanitization preset.
///
/// In v1 the only constructible value is [`Self::strict`].
#[derive(Clone, Copy, Debug, Default)]
pub struct SanitizationPolicy(());

impl SanitizationPolicy {
    /// The v1 lexical-redaction policy, with explicitly retained metadata.
    /// This preset is not a complete de-identification guarantee.
    pub fn strict() -> Self {
        Self(())
    }
}
