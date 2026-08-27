//! Typed age value for `@ID` header field 4.
//!
//! Format: `years;months.days` (e.g., `3;06.15`, `2;08`, `1;04.`).
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#Age_Field>

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use talkbank_derive::{SemanticEq, SpanShift, ValidationTagged};

/// Age string recorded in `@ID` (field 4, format `years;months.days`).
///
/// Successfully parsed ages store typed numeric components; malformed ages
/// are preserved as `Unsupported` so the validator can report actionable errors.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Age_Field>
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift, ValidationTagged)]
pub enum AgeValue {
    /// Successfully parsed age.
    Valid {
        /// Years component.
        #[span_shift(skip)]
        years: u16,
        /// Months component. Conventionally 0-11, and deliberately NOT
        /// range-checked at parse time: `13` is preserved as `13` so the
        /// validator, not the parser, decides what to say about it.
        ///
        /// REPRESENTABILITY is a different question from range, and is
        /// checked: a value above `u8::MAX` cannot be preserved here at all,
        /// so the whole age becomes [`AgeValue::Unsupported`] rather than
        /// some other number. Until 2026-08-26 it became `0`.
        #[span_shift(skip)]
        months: Option<u8>,
        /// Days component. Conventionally 0-30, with the same split between
        /// range (not checked, preserved) and representability (checked) as
        /// [`Self::Valid::months`].
        #[span_shift(skip)]
        days: Option<u8>,
        /// Original text preserved for exact roundtrip.
        #[semantic_eq(skip)]
        #[span_shift(skip)]
        raw: SmolStr,
    },
    /// Unrecognized value preserved for validation.
    Unsupported(String),
}

/// An age component whose digits do not fit the field that must hold them.
///
/// Its own type rather than `()` so the `Err` arm at each call site reads as
/// the fact it is, and so a reader of the signature learns that failure here
/// is about REPRESENTABILITY rather than about syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Unrepresentable;

/// Parse one `;`/`.` separated age component.
///
/// Three outcomes, and collapsing them is the defect this exists to prevent:
/// ABSENT is legal CHAT (`1;` has no months, `2;08.` no days) and yields
/// `Ok(None)`; a component that is not digits, or whose digits exceed
/// `u8::MAX`, is `Err(Unrepresentable)` and must sink the whole age.
///
/// The two error cases are deliberately one variant. They differ in cause but
/// not in consequence: neither can be stored, and [`AgeValue::Unsupported`]
/// keeps the original text byte for byte either way, so nothing a caller
/// could act on is lost by joining them.
fn age_component(text: &str) -> Result<Option<u8>, Unrepresentable> {
    if text.is_empty() {
        return Ok(None);
    }
    if !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Unrepresentable);
    }
    // All digits by the check above, so `parse` can now fail for exactly one
    // reason: the value is above `u8::MAX`. That used to be `unwrap_or(0)`.
    text.parse::<u8>().map(Some).map_err(|_| Unrepresentable)
}

impl AgeValue {
    /// Parse a CHAT age string (`years;months.days`).
    ///
    /// Returns `Valid` for well-formed ages, `Unsupported` otherwise.
    pub fn from_text(value: &str) -> Self {
        let Some((years_str, rest)) = value.split_once(';') else {
            return Self::Unsupported(value.to_string());
        };

        if years_str.is_empty() || !years_str.bytes().all(|b| b.is_ascii_digit()) {
            return Self::Unsupported(value.to_string());
        }

        let Ok(years) = years_str.parse::<u16>() else {
            return Self::Unsupported(value.to_string());
        };

        let (months, days) = match rest.split_once('.') {
            Some((months_str, days_str)) => {
                match (age_component(months_str), age_component(days_str)) {
                    (Ok(months), Ok(days)) => (months, days),
                    _ => return Self::Unsupported(value.to_string()),
                }
            }
            // No period: everything after the semicolon is the months field,
            // and an empty `rest` (`1;`) is the legal no-months form, which
            // `age_component` reports as `Ok(None)`.
            None => match age_component(rest) {
                Ok(months) => (months, None),
                Err(Unrepresentable) => return Self::Unsupported(value.to_string()),
            },
        };

        Self::Valid {
            years,
            months,
            days,
            raw: SmolStr::from(value),
        }
    }

    /// Returns the age as a string.
    ///
    /// Returns the original text for both valid and unsupported ages.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Valid { raw, .. } => raw.as_str(),
            Self::Unsupported(s) => s.as_str(),
        }
    }

    /// Returns true when a structurally-parseable age does not match any
    /// of the three date patterns that CLAN's authoritative `depfile.cut`
    /// declares legal for `@ID` field 4:
    ///
    /// ```text
    /// @d<yy;>  @d<yy;mm.>  @d<yy;mm.dd>
    /// ```
    ///
    /// Concretely, the raw text must be exactly one of:
    ///
    /// - `YY;`, year, semicolon, nothing else
    /// - `YY;MM.`, year, semicolon, two-digit month, trailing period
    /// - `YY;MM.DD`, year, semicolon, two-digit month, period, two-digit day
    ///
    /// Anything else, one-digit month (`3;0`), two-digit month without
    /// period (`2;06`), single-digit month with period (`3;0.15`),
    /// single-digit day (`3;06.5`), is rejected by CLAN CHECK as error 34
    /// ("Illegal date representation"). This predicate exists to make
    /// Rust chatter match that behavior.
    ///
    /// Note: `Unsupported` is already caught by `has_validation_issue()`
    /// (the derive-macro-generated predicate on the `Valid` vs
    /// `Unsupported` tag), so this method returns `false` for
    /// `Unsupported` to avoid double-reporting. The two checks are
    /// chained in `check_id_header`.
    pub fn violates_depfile_pattern(&self) -> bool {
        let Self::Valid { raw, .. } = self else {
            return false;
        };

        let raw = raw.as_str();
        let Some((years, rest)) = raw.split_once(';') else {
            return true;
        };
        if years.is_empty() || !years.bytes().all(|b| b.is_ascii_digit()) {
            return true;
        }

        // Matches `yy;`, year plus semicolon, nothing after.
        if rest.is_empty() {
            return false;
        }

        // Anything non-empty after the semicolon must contain a period,
        // depfile.cut has no template for `yy;mm` without trailing dot.
        let Some((months, days)) = rest.split_once('.') else {
            return true;
        };

        // `mm` must be exactly two digits.
        if months.len() != 2 || !months.bytes().all(|b| b.is_ascii_digit()) {
            return true;
        }

        // Matches `yy;mm.`, year, two-digit month, trailing period.
        if days.is_empty() {
            return false;
        }

        // `dd` (when present) must be exactly two digits.
        if days.len() != 2 || !days.bytes().all(|b| b.is_ascii_digit()) {
            return true;
        }

        // Matches `yy;mm.dd`.
        false
    }

    /// Backward-compatible constructor matching the old `string_newtype` API.
    pub fn new(value: impl AsRef<str>) -> Self {
        Self::from_text(value.as_ref())
    }
}

impl std::fmt::Display for AgeValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl crate::model::WriteChat for AgeValue {
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(self.as_str())
    }
}

impl Serialize for AgeValue {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AgeValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_text(&s))
    }
}

impl JsonSchema for AgeValue {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "AgeValue".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string" })
    }
}

impl From<String> for AgeValue {
    fn from(value: String) -> Self {
        Self::from_text(&value)
    }
}

impl From<&str> for AgeValue {
    fn from(value: &str) -> Self {
        Self::from_text(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An age component too large for its type must not come back as a
    /// different number.
    ///
    /// `from_text` guards each component with an all-ASCII-digits test, so
    /// `parse::<u8>()` on it can fail for exactly ONE reason: the digits
    /// denote a value above 255. The three call sites answered that with
    /// `unwrap_or(0)`, so `2;300.` parsed as `Valid { months: Some(0) }`:
    /// two years and no months, reported as a successful parse, in the field
    /// that is the primary variable of most CHILDES research.
    ///
    /// The blast radius was bounded and is worth stating exactly, because
    /// overclaiming a fix is its own defect: a component of three or more
    /// digits also fails `violates_depfile_pattern`, which requires exactly
    /// two, so such a file was always REPORTED invalid. What was wrong is
    /// what the typed model then said about it. A library caller, or anyone
    /// reading `chatter to-json`, saw a fabricated `0` presented as parsed
    /// truth, and the variant's own docstring promised the opposite: "the raw
    /// parsed value is preserved".
    ///
    /// `Unsupported` is the answer the enum already had for this. It keeps
    /// the original text byte for byte, so nothing is lost and the validator
    /// still reports it.
    #[test]
    fn a_component_too_large_for_its_type_is_unsupported_not_zero() {
        for text in ["2;300.", "2;300", "3;06.999", "1;256.", "1;06.256"] {
            match AgeValue::from_text(text) {
                AgeValue::Unsupported(raw) => assert_eq!(raw, text, "text preserved verbatim"),
                other => panic!("{text} parsed as {other:?}, expected Unsupported"),
            }
        }
    }

    /// The boundary the case above turns on, so a future widening of the
    /// component type cannot quietly move it: 255 fits and 256 does not.
    #[test]
    fn the_largest_representable_component_still_parses() {
        match AgeValue::from_text("1;255.255") {
            AgeValue::Valid {
                years,
                months,
                days,
                ..
            } => {
                assert_eq!((years, months, days), (1, Some(255), Some(255)));
            }
            other => panic!("expected Valid, got {other:?}"),
        }
    }

    /// The ordinary forms keep working, including the three shapes
    /// `depfile.cut` declares legal. Pins that the fix above did not make the
    /// parser stricter about anything except representability.
    #[test]
    fn conventional_ages_are_unaffected() {
        let cases = [
            ("3;06.15", 3u16, Some(6u8), Some(15u8)),
            ("2;08.", 2, Some(8), None),
            ("1;", 1, None, None),
            ("0;11.30", 0, Some(11), Some(30)),
        ];
        for (text, y, m, d) in cases {
            match AgeValue::from_text(text) {
                AgeValue::Valid {
                    years,
                    months,
                    days,
                    raw,
                } => {
                    assert_eq!((years, months, days), (y, m, d), "{text}");
                    assert_eq!(raw.as_str(), text, "roundtrip text for {text}");
                }
                other => panic!("{text} parsed as {other:?}, expected Valid"),
            }
        }
    }
}
