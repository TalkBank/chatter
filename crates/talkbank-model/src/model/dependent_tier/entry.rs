//! Line wrapper pairing a [`DependentTier`] with its source separator.
//!
//! Mirrors the [`Linker`](crate::model::Linker) pattern: the tier's
//! classification and payload live in [`DependentTier`], and
//! [`DependentTierEntry`] pairs it with the `colon tab trailing_space?`
//! separator between the `%label:` and the tier content. The separator is
//! provenance only (`#[serde(skip)]` / `#[schemars(skip)]` /
//! `#[semantic_eq(skip)]`), so the serialized form stays exactly that of
//! [`DependentTier`] and two entries carrying the same tier at different
//! separators compare semantically equal.
//!
//! # CHAT Format References
//!
//! - [Dependent Tiers](https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers)

use super::DependentTier;
use crate::model::{TierSeparator, WriteChat};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// One dependent-tier source line: a [`DependentTier`] plus the separator
/// between its `%label:` and its content.
///
/// The serialized form (JSON wire and schema) is exactly that of
/// [`DependentTier`] (`#[serde(transparent)]`); `separator` is provenance
/// used only by source-spacing validation (E758) to detect illegal trailing
/// whitespace after the tab. Deliberately no `Deref` to `DependentTier`:
/// callers access `.tier` explicitly, and pattern-matching a dependent tier
/// kind always needs the concrete enum regardless.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct DependentTierEntry {
    /// The dependent tier's classification and payload.
    pub tier: DependentTier,
    /// The `colon tab trailing_space?` separator between the `%label:` and
    /// the tier content. Provenance only: E758 (illegal trailing space) is
    /// detected from this field; the JSON wire and schema are unchanged.
    #[serde(skip)]
    #[schemars(skip)]
    #[semantic_eq(skip)]
    pub separator: TierSeparator,
}

impl DependentTierEntry {
    /// Wraps `tier` with a clean separator (single tab, no trailing spaces).
    pub fn new(tier: DependentTier) -> Self {
        Self {
            tier,
            separator: TierSeparator::CLEAN,
        }
    }

    /// Wraps `tier` with an explicit `separator` (parser provenance).
    pub fn with_separator(tier: DependentTier, separator: TierSeparator) -> Self {
        Self { tier, separator }
    }
}

impl From<DependentTier> for DependentTierEntry {
    /// Wraps `tier` with a clean separator, for programmatic construction
    /// (tests, builders) where the source separator is not meaningful.
    fn from(tier: DependentTier) -> Self {
        Self::new(tier)
    }
}

impl WriteChat for DependentTierEntry {
    /// Serializes this entry's tier line (`%label:\tcontent`), canonicalizing
    /// the separator to a single tab regardless of source trailing spaces.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        self.tier.write_chat(w)
    }
}

impl std::fmt::Display for DependentTierEntry {
    /// Formats this entry's tier line using its canonical CHAT serialization.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Span;
    use crate::model::{SemanticEq, TextTier};

    /// The separator is provenance only: two entries wrapping semantically
    /// equal tiers at different separators compare semantically equal.
    #[test]
    fn separator_is_not_semantic() {
        use crate::model::NonEmptyString;

        let content = NonEmptyString::new("hello").expect("literal is non-empty");
        let a = DependentTierEntry::with_separator(
            DependentTier::Eng(TextTier::new(content.clone())),
            TierSeparator::CLEAN,
        );
        let b = DependentTierEntry::with_separator(
            DependentTier::Eng(TextTier::new(content)),
            TierSeparator::with_trailing_space(Span::new(3, 5)),
        );
        assert!(
            a.semantic_eq(&b),
            "entry separator must not affect semantic eq"
        );
    }
}
