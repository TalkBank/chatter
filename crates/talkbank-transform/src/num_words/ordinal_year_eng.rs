//! English ordinal + year + decade expansion. The authored generation
//! controls and their acceptance contract are documented in
//! `book/src/contributing/testing.md`.
//!
//! Only English realistically hits these modes: ordinal and decade tokens are
//! detected via English-style suffixes (`"3rd"`, `"1950s"`), so non-English
//! text never reaches this module.
//!
//! Lexical forms were cross-validated against `num2words` for ordinals 0-1234
//! and years 1900-2100. Ordinals intentionally omit prose commas: the output
//! is a spoken word sequence. The retained fixture records that CHAT policy
//! (`data/eng_ordinal_year_fixtures.json`).

/// Cardinal forms 0-19 for composing ordinals 20+ ("twenty-first").
const CARDINAL_TENS: [&str; 10] = [
    "", "ten", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

/// Irregular ordinal forms for 0-19; index N is the ordinal of N.
const ORDINAL_0_19: [&str; 20] = [
    "zeroth",
    "first",
    "second",
    "third",
    "fourth",
    "fifth",
    "sixth",
    "seventh",
    "eighth",
    "ninth",
    "tenth",
    "eleventh",
    "twelfth",
    "thirteenth",
    "fourteenth",
    "fifteenth",
    "sixteenth",
    "seventeenth",
    "eighteenth",
    "nineteenth",
];

/// Tens-only ordinals (20, 30, ..., 90). Index N is the ordinal of N*10.
const ORDINAL_TENS_MULTIPLE: [&str; 10] = [
    "",
    "tenth",
    "twentieth",
    "thirtieth",
    "fortieth",
    "fiftieth",
    "sixtieth",
    "seventieth",
    "eightieth",
    "ninetieth",
];

const CARDINAL_0_19: [&str; 20] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];

/// An ordinal within the composer's supported range; rejection preserves input.
#[derive(Clone, Copy)]
pub(super) struct SupportedOrdinal(u16);

impl SupportedOrdinal {
    pub(super) fn admit(value: u64) -> Option<Self> {
        (value < 10_000).then_some(Self(value as u16))
    }
}

/// English ordinal: 0..=9999, with British-style conjunctions and no prose commas.
///
/// Composition rule:
/// - 0-19: irregular table lookup
/// - 20-99: tens-multiple ordinal OR `cardinal-tens-name + "-" + units-ordinal`
/// - 100-999: `cardinal-N-hundred` then either `"th"` (for `N00`) or
///   `" and "` + sub-100 ordinal
/// - 1000-9999: same shape with thousands
pub(super) fn expand_ordinal_eng(SupportedOrdinal(n): SupportedOrdinal) -> String {
    if n < 20 {
        return ORDINAL_0_19[n as usize].to_string();
    }
    if n < 100 {
        let tens = (n / 10) as usize;
        let ones = (n % 10) as usize;
        if ones == 0 {
            return ORDINAL_TENS_MULTIPLE[tens].to_string();
        }
        return format!("{}-{}", CARDINAL_TENS[tens], ORDINAL_0_19[ones]);
    }
    if n < 1000 {
        let hundreds = n / 100;
        let remainder = n % 100;
        let head = format!("{} hundred", CARDINAL_0_19[hundreds as usize]);
        if remainder == 0 {
            return format!("{head}th");
        }
        return format!(
            "{head} and {}",
            expand_ordinal_eng(SupportedOrdinal(remainder))
        );
    }
    // Admission bounds the thousands index; each remainder stays admitted.
    let thousands = n / 1000;
    let remainder = n % 1000;
    let head = format!("{} thousand", CARDINAL_0_19[thousands as usize]);
    if remainder == 0 {
        return format!("{head}th");
    }
    let connector = if remainder < 100 { " and " } else { " " };
    format!(
        "{head}{connector}{}",
        expand_ordinal_eng(SupportedOrdinal(remainder))
    )
}

/// A year inside the lexical composer's supported domain.
struct SupportedYear(u16);

impl SupportedYear {
    fn admit(n: u64) -> Option<Self> {
        (1100..=2999).contains(&n).then_some(Self(n as u16))
    }
}

enum Decade {
    Shorthand(u8),
    FullYear(SupportedYear),
}

/// Only admitted multiples of ten can reach decade inflection.
pub(super) struct SupportedDecade(Decade);

impl SupportedDecade {
    pub(super) fn admit(n: u64) -> Option<Self> {
        if !n.is_multiple_of(10) {
            return None;
        }
        let decade = if n <= 90 {
            Decade::Shorthand(n as u8)
        } else {
            Decade::FullYear(SupportedYear::admit(n)?)
        };
        Some(Self(decade))
    }
}

/// English year-form: `1950 → "nineteen fifty"`, `2007 → "two
/// thousand and seven"`, `2010 → "twenty ten"`. Mirrors
/// `num2words(n, to="year", lang="en")` for 4-digit years
/// 1100-2999. Unsupported years cannot enter composition.
fn expand_year_eng(SupportedYear(n): SupportedYear) -> String {
    let century = n / 100;
    let two_low = n % 100;

    // Special cases for 2000-2009: "two thousand", "two thousand and one"
    if century == 20 && two_low < 10 {
        if two_low == 0 {
            return "two thousand".to_string();
        }
        return format!("two thousand and {}", CARDINAL_0_19[two_low as usize]);
    }

    // 1900, 2000, 2100: "<century> hundred"
    if two_low == 0 {
        return format!("{} hundred", cardinal_under_100(century as u32));
    }

    // Default: "<century-pair> <decade-pair>"
    format!(
        "{} {}",
        cardinal_under_100(century as u32),
        cardinal_under_100(two_low as u32)
    )
}

/// English decade: `1950s → "nineteen fifties"`, `80s → "eighties"`.
/// Composes year-form for 4-digit, then pluralizes the trailing
/// decade word per English rule (`-y` → `-ies`, otherwise `-s`).
pub(super) fn expand_decade_eng(SupportedDecade(decade): SupportedDecade) -> String {
    // Numeric construction supplies a nonempty phrase with no trailing space.
    // Keep inflection here: an arbitrary-string helper would admit empty input
    // and lose the producer invariant. Decade separates supported full years
    // from the bounded shorthand input consumed by cardinal_under_100.
    let phrase = match decade {
        Decade::FullYear(year_input) => expand_year_eng(year_input),
        Decade::Shorthand(n) => cardinal_under_100(u32::from(n)),
    };
    if let Some(stem) = phrase.strip_suffix('y') {
        format!("{stem}ies")
    } else {
        format!("{phrase}s")
    }
}

fn cardinal_under_100(n: u32) -> String {
    if n < 20 {
        return CARDINAL_0_19[n as usize].to_string();
    }
    let tens = (n / 10) as usize;
    let ones = (n % 10) as usize;
    if ones == 0 {
        return CARDINAL_TENS[tens].to_string();
    }
    format!("{}-{}", CARDINAL_TENS[tens], CARDINAL_0_19[ones])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[test]
    fn decade_admission_preserves_unsupported_spelling_at_the_public_boundary() {
        for input in [
            "1s",
            "21s",
            "99s",
            "100s",
            "1090s",
            "1101s",
            "2001s",
            "2991s",
            "3000s",
            "00021s",
            "02001s",
            "0100s",
            "03000s",
            "18446744073709551615s",
        ] {
            assert_eq!(crate::num_words::expand_number(input, "eng"), input);
        }
        // Raw strings, not CHAT fixtures: initial zero is CHAT omission syntax.
        for (input, expected) in [
            ("0s", "zeros"),
            ("000s", "zeros"),
            ("01950s", "nineteen fifties"),
        ] {
            assert_eq!(crate::num_words::expand_number(input, "eng"), expected);
        }
    }

    #[derive(Deserialize)]
    struct Fixtures {
        ordinal: std::collections::BTreeMap<String, String>,
        year: std::collections::BTreeMap<String, String>,
    }

    fn load_fixtures() -> Fixtures {
        let raw = include_str!("../../data/eng_ordinal_year_fixtures.json");
        serde_json::from_str(raw).expect("parse fixture")
    }

    /// Check the retained lexical fixture, including the reviewed CHAT
    /// punctuation policy rather than general-purpose prose-format parity.
    #[test]
    fn ordinals_match_chat_spelling_fixture() {
        let f = load_fixtures();
        for (k, expected) in &f.ordinal {
            let n: u64 = k.parse().expect("numeric key");
            let actual = expand_ordinal_eng(SupportedOrdinal::admit(n).expect("supported fixture"));
            assert_eq!(
                &actual, expected,
                "ordinal({n}), actual: {actual:?}, CHAT spelling: {expected:?}"
            );
        }
    }

    /// Same cross-check for year forms over 1900-2100.
    #[test]
    fn years_match_num2words_fixture() {
        let f = load_fixtures();
        for (k, expected) in &f.year {
            let n: u64 = k.parse().expect("numeric key");
            let actual = expand_year_eng(SupportedYear::admit(n).expect("supported year fixture"));
            assert_eq!(
                &actual, expected,
                "year({n}), Rust: {actual:?}, num2words: {expected:?}"
            );
        }
    }

    /// Decade composition: year + pluralize. Spot-check the cases
    /// users will actually see (Whisper "1950s" / "80s" output).
    #[test]
    fn decade_pluralizes_year_form() {
        for (n, expected) in [
            (1950, "nineteen fifties"),
            (1920, "nineteen twenties"),
            (1900, "nineteen hundreds"),
            (2010, "twenty tens"),
            (80, "eighties"),
            (20, "twenties"),
            (10, "tens"),
            (90, "nineties"),
            (1100, "eleven hundreds"),
            (2990, "twenty-nine nineties"),
        ] {
            let admitted = SupportedDecade::admit(n).expect("supported decade control");
            assert_eq!(expand_decade_eng(admitted), expected);
        }
    }

    /// Checked admission owns the range; unsupported values cannot reach composition.
    #[test]
    fn ordinal_admission_bounds_composition_and_preserves_rejected_spelling() {
        for value in [0, 19, 99, 999, 9999] {
            assert_eq!(
                SupportedOrdinal::admit(value).map(|n| u64::from(n.0)),
                Some(value)
            );
        }
        for value in [10_000, 1_000_000, u64::MAX] {
            assert!(SupportedOrdinal::admit(value).is_none());
        }
        // A leading zero in CHAT denotes omission; this is the raw string API,
        // not an invented CHAT fixture that bypasses that grammar distinction.
        for input in [
            "010001st",
            "00010002nd",
            "10003rd",
            "18446744073709551615th",
        ] {
            assert_eq!(crate::num_words::expand_number(input, "eng"), input);
        }
    }
}
