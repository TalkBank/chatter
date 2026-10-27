//! @Types header components.
//!
//! The @Types header marks classes of groups, activities, and experimental
//! design for child language corpora. It has three mandatory fields.
//! There is no fixed vocabulary; fields are nonempty ASCII alphanumeric atoms.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#Types_Header>

use super::WriteChat;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use talkbank_derive::{SemanticEq, SpanShift};

/// A Types field is empty or contains a byte outside the CHAT atom alphabet.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("Types fields must be nonempty ASCII alphanumeric atoms")]
pub struct InvalidTypesField;

fn admit_field(value: &str) -> Result<SmolStr, InvalidTypesField> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err(InvalidTypesField);
    }
    Ok(SmolStr::new(value))
}

fn field_schema() -> schemars::Schema {
    schemars::json_schema!({
        "type": "string",
        "minLength": 1,
        "not": { "pattern": "[^A-Za-z0-9]" }
    })
}

// ---------------------------------------------------------------------------
// DesignType
// ---------------------------------------------------------------------------

/// Design type for @Types header (string newtype, no fixed vocabulary).
///
/// Common values: `cross`, `long`, `observ`.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Types_Header>
///
/// ```compile_fail
/// use talkbank_model::model::DesignType;
/// let field: DesignType = "not an atom".into();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift)]
pub struct DesignType(#[span_shift(skip)] SmolStr);

impl DesignType {
    /// Admit one complete atom without trimming or changing its spelling.
    pub fn from_text(value: &str) -> Result<Self, InvalidTypesField> {
        admit_field(value).map(Self)
    }

    /// Returns the value as a string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Serialize for DesignType {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for DesignType {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_text(&s).map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for DesignType {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "DesignType".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        field_schema()
    }
}

impl std::fmt::Display for DesignType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<String> for DesignType {
    type Error = InvalidTypesField;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_text(&value)
    }
}

impl TryFrom<&str> for DesignType {
    type Error = InvalidTypesField;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::from_text(value)
    }
}

// ---------------------------------------------------------------------------
// ActivityType
// ---------------------------------------------------------------------------

/// Activity type for @Types header (string newtype, no fixed vocabulary).
///
/// Common values: `toyplay`, `narrative`, `meal`, `pictures`, `book`,
/// `interview`, `tests`, `preverbal`, `group`, `classroom`, `reading`, `everyday`.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Types_Header>
///
/// ```compile_fail
/// use talkbank_model::model::ActivityType;
/// let field: ActivityType = "not an atom".into();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift)]
pub struct ActivityType(#[span_shift(skip)] SmolStr);

impl ActivityType {
    /// Admit one complete atom without trimming or changing its spelling.
    pub fn from_text(value: &str) -> Result<Self, InvalidTypesField> {
        admit_field(value).map(Self)
    }

    /// Returns the value as a string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Serialize for ActivityType {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ActivityType {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_text(&s).map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for ActivityType {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "ActivityType".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        field_schema()
    }
}

impl std::fmt::Display for ActivityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<String> for ActivityType {
    type Error = InvalidTypesField;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_text(&value)
    }
}

impl TryFrom<&str> for ActivityType {
    type Error = InvalidTypesField;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::from_text(value)
    }
}

// ---------------------------------------------------------------------------
// GroupType
// ---------------------------------------------------------------------------

/// Group type for @Types header (string newtype, no fixed vocabulary).
///
/// Common values: `TD`, `biling`, `AAE`, `L2`, `SLI`, `HL`, `CI`, `PD`,
/// `ASD`, `LT`, `DS`, `MR`, `ADHD`, `CWS`, `AWS`.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Types_Header>
///
/// ```compile_fail
/// use talkbank_model::model::GroupType;
/// let field: GroupType = "not an atom".into();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift)]
pub struct GroupType(#[span_shift(skip)] SmolStr);

impl GroupType {
    /// Admit one complete atom without trimming or changing its spelling.
    pub fn from_text(value: &str) -> Result<Self, InvalidTypesField> {
        admit_field(value).map(Self)
    }

    /// Returns the value as a string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Serialize for GroupType {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for GroupType {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_text(&s).map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for GroupType {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "GroupType".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        field_schema()
    }
}

impl std::fmt::Display for GroupType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<String> for GroupType {
    type Error = InvalidTypesField;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_text(&value)
    }
}

impl TryFrom<&str> for GroupType {
    type Error = InvalidTypesField;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::from_text(value)
    }
}

// ---------------------------------------------------------------------------
// TypesHeader
// ---------------------------------------------------------------------------

/// Parsed representation of the mandatory `@Types` header triple.
///
/// The @Types header marks classes of groups, activities, and experimental
/// design. It has three mandatory fields in order: design, activity, and group.
///
/// **Format:** `@Types:\tdesign, activity, group`
/// **Example:** `@Types:\tlong, toyplay, TD`
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Types_Header>
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct TypesHeader {
    /// Design type (cross-sectional, longitudinal, observational, etc.)
    pub design: DesignType,

    /// Activity type (toyplay, narrative, meal, etc.)
    pub activity: ActivityType,

    /// Group type (TD, SLI, ASD, etc.)
    pub group: GroupType,
}

impl TypesHeader {
    /// Constructs a `@Types` header value in canonical field order.
    ///
    /// Callers should preserve this order (`design, activity, group`) to stay
    /// compatible with CHAT validators and downstream tooling expectations.
    pub fn new(design: DesignType, activity: ActivityType, group: GroupType) -> Self {
        Self {
            design,
            activity,
            group,
        }
    }

    /// Admit all three fields from untrusted text, preserving their spelling.
    pub fn parse(design: &str, activity: &str, group: &str) -> Result<Self, InvalidTypesField> {
        Ok(Self::new(
            DesignType::from_text(design)?,
            ActivityType::from_text(activity)?,
            GroupType::from_text(group)?,
        ))
    }
}

impl WriteChat for TypesHeader {
    /// Serializes as `@Types:\tdesign, activity, group`.
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        {
            w.write_str("@Types:\t")?;
            crate::model::WriteChat::write_chat(&self.design.as_str(), w)?;
            w.write_str(", ")?;
            crate::model::WriteChat::write_chat(&self.activity.as_str(), w)?;
            w.write_str(", ")?;
            crate::model::WriteChat::write_chat(&self.group.as_str(), w)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_field_json_rejects_non_atoms() {
        for text in [
            "", " ", "toy play", "T-D", "TDé", "TD\n", "TD\r", "TD\t", "TD\0", "long,", "１",
        ] {
            assert!(DesignType::from_text(text).is_err());
            assert!(ActivityType::try_from(text).is_err());
            assert!(GroupType::try_from(text.to_owned()).is_err());
            assert!(TypesHeader::parse(text, "toyplay", "TD").is_err());
            assert!(TypesHeader::parse("long", text, "TD").is_err());
            assert!(TypesHeader::parse("long", "toyplay", text).is_err());
            let wire = serde_json::json!(text);
            assert!(
                serde_json::from_value::<DesignType>(wire.clone()).is_err(),
                "design admitted {text:?}"
            );
            assert!(
                serde_json::from_value::<ActivityType>(wire.clone()).is_err(),
                "activity admitted {text:?}"
            );
            assert!(
                serde_json::from_value::<GroupType>(wire).is_err(),
                "group admitted {text:?}"
            );
        }
    }

    #[test]
    fn types_fields_preserve_custom_alphanumeric_spelling() {
        for text in ["cross", "toyplay", "TD", "L2", "Custom42", "0007"] {
            let wire = serde_json::json!(text);
            assert_eq!(
                serde_json::from_value::<DesignType>(wire.clone())
                    .unwrap()
                    .as_str(),
                text
            );
            assert_eq!(
                serde_json::from_value::<ActivityType>(wire.clone())
                    .unwrap()
                    .as_str(),
                text
            );
            assert_eq!(
                serde_json::from_value::<GroupType>(wire).unwrap().as_str(),
                text
            );
        }
    }
}
