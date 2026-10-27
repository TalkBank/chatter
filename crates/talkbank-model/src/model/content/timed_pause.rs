//! Lexical admission for timed CHAT pauses, independent of numeric range.

use super::PauseTimedDuration;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// A timed CHAT pause spelling with an optional machine-sized projection.
///
/// The decimal point is mandatory; minutes and fractional digits are optional.
/// Numeric overflow preserves the admitted spelling without a projection.
///
/// ```compile_fail
/// use talkbank_model::model::{PauseDuration, PauseTimedDuration};
/// let pause = PauseDuration::Timed(PauseTimedDuration::new("arbitrary text"));
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, SemanticEq, SpanShift)]
#[serde(transparent)]
pub struct TimedPause(PauseTimedDuration);

/// Text does not have the lexical form of a timed CHAT pause payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error(
    "expected digits, optional colon and digits, a decimal point, and optional fractional digits"
)]
pub struct InvalidTimedPauseSpelling;

impl TimedPause {
    /// Admit source spelling before attempting its optional numeric projection.
    pub fn new(text: impl Into<smol_str::SmolStr>) -> Result<Self, InvalidTimedPauseSpelling> {
        let text = text.into();
        let (whole, fraction) = text.split_once('.').ok_or(InvalidTimedPauseSpelling)?;
        let digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
        let whole_valid = match whole.split_once(':') {
            Some((minutes, seconds)) => digits(minutes) && digits(seconds),
            None => digits(whole),
        };
        if !whole_valid || !fraction.bytes().all(|b| b.is_ascii_digit()) {
            return Err(InvalidTimedPauseSpelling);
        }
        Ok(Self(PauseTimedDuration::new(text)))
    }

    /// Authored spelling, including all fractional precision.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Numeric projection; unsupported range does not invalidate CHAT syntax.
    pub fn projection(&self) -> &PauseTimedDuration {
        &self.0
    }

    /// Milliseconds when the authored duration has a machine-sized projection.
    pub fn total_millis(&self) -> Option<u64> {
        self.0.total_millis()
    }
}

impl<'de> Deserialize<'de> for TimedPause {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wire {
            seconds: String,
        }
        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.seconds).map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for TimedPause {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "TimedPause".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "object",
            "properties": {
                "seconds": {
                    "type": "string",
                    "pattern": "^[0-9]+(:[0-9]+)?\\.[0-9]*$",
                    "not": { "pattern": "[^0-9:.]" }
                }
            },
            "required": ["seconds"]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admission_and_wire_keep_syntax_separate_from_numeric_range() {
        for (text, millis) in [
            ("0.", Some(0)),
            ("7:1.234567", Some(421_234)),
            ("4294967296.5", None),
            ("71582788:16.0", None),
        ] {
            let pause = TimedPause::new(text).expect("valid CHAT spelling");
            assert_eq!(pause.as_str(), text);
            assert_eq!(pause.total_millis(), millis);
            let wire = serde_json::json!({ "seconds": text });
            assert_eq!(serde_json::to_value(&pause).unwrap(), wire);
            assert_eq!(serde_json::from_value::<TimedPause>(wire).unwrap(), pause);
        }
        for text in [
            "", "2", "1:2", "+1.5", "-1.5", ".5", "1:2:3.4", "1.2.3", "1.5\n", "１.5", "1.5)",
        ] {
            assert!(TimedPause::new(text).is_err(), "admitted {text:?}");
        }
        assert_eq!(PauseTimedDuration::new("2").total_millis(), Some(2000));
    }
}
