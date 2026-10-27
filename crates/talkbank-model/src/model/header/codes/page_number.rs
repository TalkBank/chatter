//! Lexical admission for the decimal digit payload of `@Page`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// A nonempty sequence of ASCII decimal digits, preserving authored spelling.
/// Leading zeroes and arbitrarily long identifiers are retained without numeric
/// conversion. The CHAT `page_number` rule is `[0-9]+`.
///
/// Untrusted strings must cross checked admission.
/// ```compile_fail
/// use talkbank_model::model::PageNumber;
/// let page: PageNumber = "not a page".into();
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SemanticEq, SpanShift)]
#[serde(try_from = "String")]
pub struct PageNumber(smol_str::SmolStr);

/// The payload does not match the CHAT decimal digit syntax.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("page number must be a nonempty sequence of ASCII digits")]
pub struct InvalidPageNumber;

impl PageNumber {
    /// Admit the complete payload without trimming or normalizing its spelling.
    pub fn new(text: impl AsRef<str>) -> Result<Self, InvalidPageNumber> {
        let text = text.as_ref();
        if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(InvalidPageNumber);
        }
        Ok(Self(smol_str::SmolStr::new(text)))
    }
}

crate::string_newtype_read_impls!(PageNumber);

impl TryFrom<String> for PageNumber {
    type Error = InvalidPageNumber;
    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::new(text)
    }
}

impl TryFrom<&str> for PageNumber {
    type Error = InvalidPageNumber;
    fn try_from(text: &str) -> Result<Self, Self::Error> {
        Self::new(text)
    }
}

impl JsonSchema for PageNumber {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "PageNumber".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "minLength": 1,
            "not": { "pattern": "[^0-9]" }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::PageNumber;
    use crate::model::WriteChat;

    #[test]
    fn page_number_wire_rejects_non_decimal_text() {
        for text in [
            "", "abc", "1.5", "-1", "+1", " 1", "1 ", "1\n", "１", "1\t", "1\r", "1\0",
        ] {
            assert!(PageNumber::new(text).is_err(), "admitted {text:?}");
            assert!(
                serde_json::from_value::<PageNumber>(serde_json::json!(text)).is_err(),
                "admitted nonnumeric page {text:?}"
            );
        }
    }

    #[test]
    fn page_number_wire_requires_a_string() {
        for value in [
            serde_json::json!(null),
            serde_json::json!(true),
            serde_json::json!(42),
            serde_json::json!([]),
            serde_json::json!({}),
        ] {
            assert!(serde_json::from_value::<PageNumber>(value).is_err());
        }
    }

    #[test]
    fn page_number_admission_preserves_digit_spelling_without_numeric_limits() {
        for text in [
            "0",
            "42",
            "00042",
            "184467440737095516160000000000000000000000",
        ] {
            let page = PageNumber::new(text).unwrap();
            assert_eq!(page.as_str(), text);
            assert_eq!(page.to_chat_string(), text);
            let wire = serde_json::to_value(&page).unwrap();
            assert_eq!(wire, serde_json::json!(text));
            assert_eq!(serde_json::from_value::<PageNumber>(wire).unwrap(), page);
            assert_eq!(PageNumber::try_from(text).unwrap(), page);
            assert_eq!(PageNumber::try_from(text.to_owned()).unwrap(), page);
        }
    }
}
