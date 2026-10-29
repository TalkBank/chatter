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
/// retain an unsupported state so validation can report actionable errors.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Age_Field>
///
/// Parsed components cannot be paired with unrelated serialized text.
///
/// ```compile_fail
/// use talkbank_model::model::AgeValue;
/// let contradictory = AgeValue::Valid {
///     years: 1, months: Some(0), days: None, raw: "60;00.".into(),
/// };
/// ```
#[derive(Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift)]
pub struct AgeValue {
    state: AgeState,
}

/// Numeric components admitted from the same text that an age serializes.
/// Presence is distinct from zero; representable out-of-range values remain
/// available for contextual validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SemanticEq, SpanShift)]
pub struct AgeComponents {
    #[span_shift(skip)]
    years: u16,
    #[span_shift(skip)]
    months: Option<u8>,
    #[span_shift(skip)]
    days: Option<u8>,
}

impl AgeComponents {
    /// Declared whole years.
    pub fn years(&self) -> u16 {
        self.years
    }
    /// Declared months, or absence when the component was empty.
    pub fn months(&self) -> Option<u8> {
        self.months
    }
    /// Declared days, or absence when the component was empty.
    pub fn days(&self) -> Option<u8> {
        self.days
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum AgeSpelling {
    YearsOnly,
    MonthsOnly,
    MonthsAndDays {
        month_width: usize,
        day_width: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift, ValidationTagged)]
enum AgeState {
    Valid {
        components: AgeComponents,
        #[semantic_eq(skip)]
        #[span_shift(skip)]
        spelling: AgeSpelling,
        #[semantic_eq(skip)]
        #[span_shift(skip)]
        raw: SmolStr,
    },
    Unsupported(String),
}

impl std::fmt::Debug for AgeValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.state {
            AgeState::Valid {
                components, raw, ..
            } => f
                .debug_struct("Valid")
                .field("years", &components.years)
                .field("months", &components.months)
                .field("days", &components.days)
                .field("raw", raw)
                .finish(),
            AgeState::Unsupported(raw) => f.debug_tuple("Unsupported").field(raw).finish(),
        }
    }
}

impl crate::model::ValidationTagged for AgeValue {
    fn validation_tag(&self) -> crate::model::ValidationTag {
        crate::model::ValidationTagged::validation_tag(&self.state)
    }
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
/// not in consequence: neither can be stored, and the unsupported age state
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

impl AgeState {
    fn from_text(value: &str) -> Self {
        let Some((years_str, rest)) = value.split_once(';') else {
            return Self::Unsupported(value.to_string());
        };
        if years_str.is_empty() || !years_str.bytes().all(|b| b.is_ascii_digit()) {
            return Self::Unsupported(value.to_string());
        }
        let Ok(years) = years_str.parse::<u16>() else {
            return Self::Unsupported(value.to_string());
        };
        let (months, days, spelling) = match rest.split_once('.') {
            Some((months_str, days_str)) => {
                match (age_component(months_str), age_component(days_str)) {
                    (Ok(months), Ok(days)) => (
                        months,
                        days,
                        AgeSpelling::MonthsAndDays {
                            month_width: months_str.len(),
                            day_width: days_str.len(),
                        },
                    ),
                    _ => return Self::Unsupported(value.to_string()),
                }
            }
            None => match age_component(rest) {
                Ok(months) => (
                    months,
                    None,
                    if rest.is_empty() {
                        AgeSpelling::YearsOnly
                    } else {
                        AgeSpelling::MonthsOnly
                    },
                ),
                Err(Unrepresentable) => return Self::Unsupported(value.to_string()),
            },
        };
        Self::Valid {
            components: AgeComponents {
                years,
                months,
                days,
            },
            spelling,
            raw: SmolStr::from(value),
        }
    }
}

impl AgeValue {
    /// Admit numeric components and spelling together, retaining unsupported
    /// input without inventing numeric values. Range validation is separate.
    pub fn from_text(value: &str) -> Self {
        Self {
            state: AgeState::from_text(value),
        }
    }

    /// Read admitted components; unsupported input has no numeric projection.
    ///
    /// Years fit in `u16`, months and days in `u8`. Unrepresentable spellings
    /// retain their complete text through [`Self::as_str`]. Numeric admission
    /// alone does not establish contextual validity or legal CHAT spelling.
    pub fn components(&self) -> Option<&AgeComponents> {
        match &self.state {
            AgeState::Valid { components, .. } => Some(components),
            AgeState::Unsupported(_) => None,
        }
    }

    /// Returns the age as a string.
    ///
    /// Returns the original text for both valid and unsupported ages.
    pub fn as_str(&self) -> &str {
        match &self.state {
            AgeState::Valid { raw, .. } => raw.as_str(),
            AgeState::Unsupported(s) => s.as_str(),
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
        let AgeState::Valid { spelling, .. } = &self.state else {
            return false;
        };
        match spelling {
            AgeSpelling::YearsOnly => false,
            AgeSpelling::MonthsOnly => true,
            AgeSpelling::MonthsAndDays {
                month_width,
                day_width,
            } => *month_width != 2 || !matches!(day_width, 0 | 2),
        }
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
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
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
        for text in [
            "2;300.",
            "2;300",
            "3;06.999",
            "1;256.",
            "1;06.256",
            "65536;",
            "65536;06.05",
            "999999999999999999999999;00.00",
        ] {
            let age = AgeValue::from_text(text);
            assert!(age.components().is_none(), "{text}");
            assert_eq!(age.as_str(), text);
            assert!(
                !age.violates_depfile_pattern(),
                "unsupported values use their own validation tag"
            );
            let wire = serde_json::to_string(&age).unwrap();
            assert_eq!(wire, serde_json::to_string(text).unwrap());
            let restored: AgeValue = serde_json::from_str(&wire).unwrap();
            assert!(restored.components().is_none());
            assert_eq!(restored.as_str(), text);
        }
    }

    #[test]
    fn the_largest_representable_component_still_parses() {
        let age = AgeValue::from_text("65535;255.255");
        let components = age.components().unwrap();
        assert_eq!(
            (components.years(), components.months(), components.days()),
            (65535, Some(255), Some(255))
        );
    }

    #[test]
    fn conventional_ages_are_unaffected() {
        for (text, y, m, d) in [
            ("3;06.15", 3, Some(6), Some(15)),
            ("2;08.", 2, Some(8), None),
            ("1;", 1, None, None),
            ("0;11.30", 0, Some(11), Some(30)),
        ] {
            let age = AgeValue::from_text(text);
            let components = age.components().unwrap();
            assert_eq!(
                (components.years(), components.months(), components.days()),
                (y, m, d)
            );
            assert_eq!(age.as_str(), text);
        }
    }
    #[test]
    fn admitted_spelling_keeps_the_depfile_policy_and_wire() {
        for text in ["1;", "2;08.", "3;06.15", "0003;00.00"] {
            let age = AgeValue::from_text(text);
            assert!(age.components().is_some(), "{text}");
            assert!(!age.violates_depfile_pattern(), "{text}");
            let restored: AgeValue =
                serde_json::from_str(&serde_json::to_string(&age).unwrap()).unwrap();
            assert_eq!(restored, age);
        }
        for text in [
            "1;.", "1;.03", "3;0", "2;06", "3;0.15", "3;06.5", "3;006.", "3;06.005",
        ] {
            let age = AgeValue::from_text(text);
            assert!(age.components().is_some(), "representable spelling: {text}");
            assert!(age.violates_depfile_pattern(), "{text}");
            let restored: AgeValue =
                serde_json::from_str(&serde_json::to_string(&age).unwrap()).unwrap();
            assert_eq!(restored, age);
        }
        for text in ["", ";", "x;", "1;x.", "1;00.x", "1;00.00.", "65536;"] {
            let age = AgeValue::from_text(text);
            assert!(age.components().is_none(), "{text}");
            assert_eq!(age.as_str(), text);
            let restored: AgeValue =
                serde_json::from_str(&serde_json::to_string(&age).unwrap()).unwrap();
            assert_eq!(restored, age);
        }
    }
}
