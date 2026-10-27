//! Admission for the custom dependent-tier namespace.

use crate::generated::phon_tier_labels::RESERVED_CUSTOM_TIER_LABELS;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use talkbank_derive::{SemanticEq, SpanShift};

/// Why a raw label cannot name a custom dependent tier.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InvalidUserDefinedDependentTierLabel {
    /// Custom labels start with lowercase x, then an ASCII letter and alphanumerics.
    #[error("custom tier label must match x[A-Za-z][A-Za-z0-9]*")]
    InvalidSyntax,
    /// This spelling names a structured tier.
    #[error("custom tier label is reserved for a structured tier")]
    Reserved,
}

/// A complete custom-tier label, including its leading x but no percent sign.
/// Construction and JSON import exclude structured labels and line delimiters.
///
/// Build custom dependent tiers with the full label rather than a separate
/// suffix-only representation:
///
/// ```
/// use talkbank_model::{
///     DependentTier, Location, RecoveryText, UserDefinedDependentTier,
///     UserDefinedDependentTierLabel,
/// };
/// let tier = DependentTier::UserDefined(UserDefinedDependentTier {
///     label: UserDefinedDependentTierLabel::new("xgesture")?,
///     content: Some(RecoveryText::new("points")?),
///     location: Location::Unlocated,
/// });
/// assert_eq!(tier.to_string(), "%xgesture:\tpoints");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SemanticEq, SpanShift)]
#[serde(try_from = "String")]
pub struct UserDefinedDependentTierLabel(SmolStr);

impl UserDefinedDependentTierLabel {
    /// Admit the full label once, before it enters the AST.
    pub fn new(text: impl AsRef<str>) -> Result<Self, InvalidUserDefinedDependentTierLabel> {
        let text = text.as_ref();
        let mut bytes = text.bytes();
        if bytes.next() != Some(b'x')
            || !bytes.next().is_some_and(|byte| byte.is_ascii_alphabetic())
            || !bytes.all(|byte| byte.is_ascii_alphanumeric())
        {
            return Err(InvalidUserDefinedDependentTierLabel::InvalidSyntax);
        }
        if RESERVED_CUSTOM_TIER_LABELS.contains(&text) {
            return Err(InvalidUserDefinedDependentTierLabel::Reserved);
        }
        Ok(Self(SmolStr::new(text)))
    }

    /// Borrow the original admitted spelling.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl TryFrom<String> for UserDefinedDependentTierLabel {
    type Error = InvalidUserDefinedDependentTierLabel;
    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::new(text)
    }
}
impl TryFrom<&str> for UserDefinedDependentTierLabel {
    type Error = InvalidUserDefinedDependentTierLabel;
    fn try_from(text: &str) -> Result<Self, Self::Error> {
        Self::new(text)
    }
}
impl std::ops::Deref for UserDefinedDependentTierLabel {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}
impl AsRef<str> for UserDefinedDependentTierLabel {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl std::fmt::Display for UserDefinedDependentTierLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
impl crate::model::WriteChat for UserDefinedDependentTierLabel {
    fn write_chat<W: crate::model::ChatWriter>(&self, writer: &mut W) -> Result<(), W::Error> {
        writer.write_str(self.as_str())
    }
}
impl JsonSchema for UserDefinedDependentTierLabel {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "UserDefinedDependentTierLabel".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "pattern": "^x[A-Za-z][A-Za-z0-9]*$",
            "not": { "anyOf": [
                { "pattern": "[^A-Za-z0-9]" },
                { "enum": RESERVED_CUSTOM_TIER_LABELS }
            ] }
        })
    }
}
