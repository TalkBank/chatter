//! Cardinality admission for a resolved mixed or ambiguous language set.

use crate::LanguageCode;

/// At least two language codes, in their authored order.
///
/// This proves cardinality only. Code shape and registry membership retain
/// their separate shared validation policy. Recovery markers may carry shorter
/// lists, but those lists cannot be represented as a resolved multi-code set.
/// The JSON representation remains an array, with the same admission on import.
///
/// ```compile_fail
/// use talkbank_model::MultipleLanguageCodes;
/// let empty = MultipleLanguageCodes(Vec::new());
/// ```
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, talkbank_derive::SpanShift,
)]
#[serde(try_from = "Vec<LanguageCode>")]
pub struct MultipleLanguageCodes(Vec<LanguageCode>);

impl schemars::JsonSchema for MultipleLanguageCodes {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "MultipleLanguageCodes".into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let mut schema = <Vec<LanguageCode> as schemars::JsonSchema>::json_schema(generator);
        schema.insert("minItems".into(), 2.into());
        schema
    }
}

/// A mixed or ambiguous marker did not provide at least two language codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("mixed or ambiguous language marker requires at least two codes; found {observed}")]
pub struct TooFewLanguageCodes {
    /// Number of codes that were actually supplied.
    pub observed: usize,
}

impl TryFrom<Vec<LanguageCode>> for MultipleLanguageCodes {
    type Error = TooFewLanguageCodes;

    fn try_from(codes: Vec<LanguageCode>) -> Result<Self, Self::Error> {
        if codes.len() < 2 {
            Err(TooFewLanguageCodes {
                observed: codes.len(),
            })
        } else {
            Ok(Self(codes))
        }
    }
}

impl MultipleLanguageCodes {
    /// Borrow the admitted codes without granting length-changing access.
    pub fn as_slice(&self) -> &[LanguageCode] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_and_wire_share_cardinality_admission() {
        for wire in ["[]", "[\"eng\"]"] {
            let codes: Vec<LanguageCode> = serde_json::from_str(wire).expect("raw codes");
            let count = codes.len();
            assert_eq!(
                MultipleLanguageCodes::try_from(codes),
                Err(TooFewLanguageCodes { observed: count })
            );
            assert!(serde_json::from_str::<MultipleLanguageCodes>(wire).is_err());
            for variant in ["Multiple", "Ambiguous"] {
                let resolution = format!("{{\"{variant}\":{wire}}}");
                assert!(
                    serde_json::from_str::<crate::validation::LanguageResolution>(&resolution)
                        .is_err()
                );
                assert!(serde_json::from_str::<crate::WordLanguages>(&resolution).is_err());
            }
        }
        for wire in ["[\"eng\",\"spa\"]", "[\"eng\",\"spa\",\"fra\"]"] {
            let codes: MultipleLanguageCodes = serde_json::from_str(wire).expect("multiple codes");
            assert_eq!(serde_json::to_string(&codes).expect("array wire"), wire);
            assert_eq!(codes.as_slice()[0].as_str(), "eng");
            for variant in ["Multiple", "Ambiguous"] {
                let wire = format!("{{\"{variant}\":{wire}}}");
                let resolution: crate::validation::LanguageResolution =
                    serde_json::from_str(&wire).expect("resolved languages");
                let metadata: crate::WordLanguages =
                    serde_json::from_str(&wire).expect("language metadata");
                assert_eq!(
                    serde_json::to_string(&resolution).expect("resolution wire"),
                    wire
                );
                assert_eq!(
                    serde_json::to_string(&metadata).expect("metadata wire"),
                    wire
                );
            }
        }
    }
}
