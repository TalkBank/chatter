//! Typed calendar date for CHAT `@Date` and `@Birth of` headers.
//!
//! Format: `DD-MMM-YYYY` (e.g., `01-JAN-2024`).
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>

use crate::model::{HeaderText, NonEmptyString};
use smol_str::SmolStr;
use talkbank_derive::{SemanticEq, SpanShift, ValidationTagged};

/// Date components contain digits, not the wider signed-integer syntax.
/// Width and alphabet are admitted together before numeric interpretation.
struct DateDigits<const WIDTH: usize>([u8; WIDTH]);

/// Why a day or year component is not its fixed-width digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DateDigitProblem {
    /// Not exactly the component's width.
    Width,
    /// The right width, but not only ASCII digits.
    NonDigit,
}

impl<const WIDTH: usize> DateDigits<WIDTH> {
    fn parse(text: &str) -> Result<Self, DateDigitProblem> {
        let bytes: [u8; WIDTH] = text
            .as_bytes()
            .try_into()
            .map_err(|_| DateDigitProblem::Width)?;
        if bytes.iter().all(u8::is_ascii_digit) {
            Ok(Self(bytes.map(|byte| byte - b'0')))
        } else {
            Err(DateDigitProblem::NonDigit)
        }
    }
}

impl DateDigits<2> {
    /// Two admitted decimal digits fit in u8 without parsing or narrowing.
    fn day(self) -> u8 {
        self.0[0] * 10 + self.0[1]
    }
}

impl DateDigits<4> {
    /// Four admitted decimal digits, as a year in 0-9999.
    fn year(self) -> u16 {
        self.0
            .into_iter()
            .fold(0, |value, digit| value * 10 + u16::from(digit))
    }
}

/// Three-letter month abbreviations used in CHAT dates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SemanticEq, SpanShift)]
pub enum Month {
    /// January.
    Jan,
    /// February.
    Feb,
    /// March.
    Mar,
    /// April.
    Apr,
    /// May.
    May,
    /// June.
    Jun,
    /// July.
    Jul,
    /// August.
    Aug,
    /// September.
    Sep,
    /// October.
    Oct,
    /// November.
    Nov,
    /// December.
    Dec,
}

impl Month {
    /// Every month, in calendar order.
    const ALL: [Self; 12] = [
        Self::Jan,
        Self::Feb,
        Self::Mar,
        Self::Apr,
        Self::May,
        Self::Jun,
        Self::Jul,
        Self::Aug,
        Self::Sep,
        Self::Oct,
        Self::Nov,
        Self::Dec,
    ];

    /// Parse a three-letter month abbreviation (case-sensitive, uppercase),
    /// saying why anything else is not one.
    fn from_text(s: &str) -> Result<Self, MonthProblem> {
        if let Some(month) = Self::ALL.into_iter().find(|month| month.as_str() == s) {
            return Ok(month);
        }
        if s.len() != 3 {
            return Err(MonthProblem::NotAbbreviation);
        }
        match Self::ALL
            .into_iter()
            .find(|month| month.as_str().eq_ignore_ascii_case(s))
        {
            Some(month) => Err(MonthProblem::NotUppercase(month)),
            None => Err(MonthProblem::Unknown),
        }
    }

    /// Returns the canonical three-letter uppercase abbreviation.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Jan => "JAN",
            Self::Feb => "FEB",
            Self::Mar => "MAR",
            Self::Apr => "APR",
            Self::May => "MAY",
            Self::Jun => "JUN",
            Self::Jul => "JUL",
            Self::Aug => "AUG",
            Self::Sep => "SEP",
            Self::Oct => "OCT",
            Self::Nov => "NOV",
            Self::Dec => "DEC",
        }
    }
}

/// Calendar date value for `@Date` and `@Birth of` headers.
///
/// Successfully parsed dates store typed components; malformed dates are
/// preserved as `Unsupported` with the model's own reading of what is wrong
/// ([`InvalidChatDate`]), which the validator only renders. A date is never
/// blank: a date header with nothing after its colon is
/// `Header::EmptyContent`.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift, ValidationTagged)]
pub enum ChatDate {
    /// Successfully admitted `DD-MMM-YYYY` date components.
    Valid(CheckedChatDate),
    /// A nonblank value that is not a `DD-MMM-YYYY` date, preserved with
    /// the reason.
    Unsupported(InvalidChatDate),
}

/// Date components admitted together with their original spelling.
///
/// Obtain this payload through [`ChatDate::from_content`] or
/// [`ChatDate::try_new`]. Admission checks format and the day range 01-31,
/// not calendar or leap-year validity. Components cannot be changed
/// independently of the stored text.
///
/// ```compile_fail,E0616
/// # use talkbank_model::model::ChatDate;
/// # if let Ok(ChatDate::Valid(mut date)) = ChatDate::try_new("01-JAN-2024") {
/// date.day = 0;
/// # }
/// ```
///
/// ```compile_fail,E0451
/// # use talkbank_model::model::{CheckedChatDate, Month};
/// let date = CheckedChatDate { day: 0, month: Month::Jan, year: 2024, raw: "01-JAN-2024".into() };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift)]
pub struct CheckedChatDate {
    /// Day of month (1-31).
    #[span_shift(skip)]
    day: u8,
    /// Month abbreviation.
    #[span_shift(skip)]
    month: Month,
    /// Four-digit year.
    #[span_shift(skip)]
    year: u16,
    /// Original text preserved for roundtrip.
    #[semantic_eq(skip)]
    #[span_shift(skip)]
    raw: SmolStr,
}

impl CheckedChatDate {
    /// Admitted day of month, in 1-31.
    pub fn day(&self) -> u8 {
        self.day
    }

    /// Admitted uppercase month abbreviation.
    pub fn month(&self) -> Month {
        self.month
    }

    /// Admitted four-digit year, in 0-9999.
    pub fn year(&self) -> u16 {
        self.year
    }

    /// Original spelling admitted with these components.
    pub fn as_str(&self) -> &str {
        self.raw.as_str()
    }
}

/// A nonblank date that is not `DD-MMM-YYYY`: its text as written, and the
/// model's reading of what is wrong, issued only by [`ChatDate::from_content`].
///
/// Each refused component keeps its own text, captured when
/// `from_content` split the date, so the validator renders this refusal
/// (E518 for `@Date`, E545 for `@Birth of`) through
/// [`InvalidChatDate::refusals`] and nothing splits the date a second time.
///
/// ```compile_fail,E0451
/// # use talkbank_model::model::header::InvalidChatDate;
/// let forged = InvalidChatDate { raw: todo!(), problem: todo!() };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift)]
pub struct InvalidChatDate {
    /// The text as written.
    #[span_shift(skip)]
    raw: NonEmptyString,
    /// What is wrong with it.
    #[semantic_eq(skip)]
    #[span_shift(skip)]
    problem: DateProblem,
}

/// What is wrong with a date that is not `DD-MMM-YYYY`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum DateProblem {
    /// The text is not three hyphen-separated components.
    Shape,
    /// Three components, at least one of them wrong (established by
    /// [`ChatDate::from_content`], the only place this is built), each
    /// refusal with its component's text.
    Components {
        day: Option<RefusedComponent<DayProblem>>,
        month: Option<RefusedComponent<MonthProblem>>,
        year: Option<RefusedComponent<DateDigitProblem>>,
    },
}

/// One refused date component: its text as written and why.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RefusedComponent<P> {
    text: SmolStr,
    problem: P,
}

impl<P> RefusedComponent<P> {
    /// The component's refusal, when `admitted` refused it.
    fn of<T>(text: &str, admitted: Result<T, P>) -> Option<Self> {
        admitted.err().map(|problem| Self {
            text: SmolStr::from(text),
            problem,
        })
    }
}

/// Why a day component is not `01`-`31`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DayProblem {
    /// Not two ASCII digits.
    Digits(DateDigitProblem),
    /// Two digits outside `01`-`31`.
    OutOfRange,
}

/// Why a month component is not an uppercase abbreviation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MonthProblem {
    /// A month abbreviation in the wrong case; the uppercase one is `Month`.
    NotUppercase(Month),
    /// Three characters, but no month's abbreviation in any case.
    Unknown,
    /// Not three characters long.
    NotAbbreviation,
}

/// One thing wrong with an invalid date, with the component text it is
/// about: what the validator renders, one diagnostic each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateRefusal<'a> {
    /// The whole text is not three hyphen-separated components.
    Shape(&'a str),
    /// The day component.
    Day(&'a str, DayProblem),
    /// The month component.
    Month(&'a str, MonthProblem),
    /// The year component.
    Year(&'a str, DateDigitProblem),
}

impl InvalidChatDate {
    /// The text as written.
    pub fn as_str(&self) -> &str {
        self.raw.as_str()
    }

    /// Each thing wrong with the date, with its component text, in the
    /// order day, month, year: only iterates what `from_content` recorded.
    pub fn refusals(&self) -> impl Iterator<Item = DateRefusal<'_>> {
        let (shape, components) = match &self.problem {
            DateProblem::Shape => (Some(DateRefusal::Shape(self.raw.as_str())), None),
            DateProblem::Components { day, month, year } => (
                None,
                Some([
                    day.as_ref()
                        .map(|day| DateRefusal::Day(&day.text, day.problem)),
                    month
                        .as_ref()
                        .map(|month| DateRefusal::Month(&month.text, month.problem)),
                    year.as_ref()
                        .map(|year| DateRefusal::Year(&year.text, year.problem)),
                ]),
            ),
        };
        shape
            .into_iter()
            .chain(components.into_iter().flatten().flatten())
    }
}

impl ChatDate {
    /// Classify date content (`DD-MMM-YYYY`).
    ///
    /// Returns `Valid` for well-formed dates, `Unsupported` with the reason
    /// otherwise. The argument is the proof that the one emptiness decision
    /// found content.
    pub fn from_content(value: HeaderText<'_>) -> Self {
        let text = value.as_str();
        let unsupported = |problem| {
            Self::Unsupported(InvalidChatDate {
                raw: value.as_non_empty().into_owned(),
                problem,
            })
        };
        let mut parts = text.split('-');
        let (Some(day_str), Some(month_str), Some(year_str), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return unsupported(DateProblem::Shape);
        };
        match (
            parse_day(day_str),
            Month::from_text(month_str),
            DateDigits::<4>::parse(year_str),
        ) {
            (Ok(day), Ok(month), Ok(year)) => Self::Valid(CheckedChatDate {
                day,
                month,
                year: year.year(),
                raw: SmolStr::from(text),
            }),
            // At least one component was refused, or the first arm would
            // have matched: the "at least one problem" invariant holds by
            // construction, here, the only place the value is built.
            (day, month, year) => unsupported(DateProblem::Components {
                day: RefusedComponent::of(day_str, day),
                month: RefusedComponent::of(month_str, month),
                year: RefusedComponent::of(year_str, year),
            }),
        }
    }

    /// Returns the date as a string.
    ///
    /// Returns the original text for both valid and unsupported dates.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Valid(date) => date.as_str(),
            Self::Unsupported(invalid) => invalid.as_str(),
        }
    }

    /// The model's refusal of this date, when it is not `DD-MMM-YYYY`.
    pub fn invalidity(&self) -> Option<&InvalidChatDate> {
        match self {
            Self::Valid(_) => None,
            Self::Unsupported(invalid) => Some(invalid),
        }
    }
}

crate::header_content_admission!(ChatDate, render, schema);

/// Parse a two-digit day (01-31).
fn parse_day(s: &str) -> Result<u8, DayProblem> {
    let day = DateDigits::<2>::parse(s).map_err(DayProblem::Digits)?.day();
    if (1..=31).contains(&day) {
        Ok(day)
    } else {
        Err(DayProblem::OutOfRange)
    }
}

#[cfg(test)]
mod tests {
    use super::{ChatDate, DateDigitProblem, DateRefusal, DayProblem, Month, MonthProblem};
    use crate::model::BlankContent;

    #[test]
    fn checked_date_preserves_format_only_admission() {
        // Pin the existing policy: no calendar validation is introduced by the
        // checked payload, including for year zero and impossible month days.
        for text in ["31-FEB-0000", "29-FEB-2023", "01-JAN-9999"] {
            let date = ChatDate::try_new(text).expect("nonblank");
            let ChatDate::Valid(checked) = &date else {
                panic!("format admitted")
            };
            assert_eq!(checked.as_str(), text);
            assert_eq!(
                format!(
                    "{:02}-{}-{:04}",
                    checked.day(),
                    checked.month().as_str(),
                    checked.year()
                ),
                text
            );
            assert_eq!(serde_json::to_string(&date).unwrap(), format!("\"{text}\""));
            assert_eq!(
                serde_json::from_str::<ChatDate>(&format!("\"{text}\"")).unwrap(),
                date
            );
        }
        let Ok(ChatDate::Valid(date)) = ChatDate::try_new("31-FEB-0000") else {
            panic!("format admitted")
        };
        assert_eq!((date.day(), date.month(), date.year()), (31, Month::Feb, 0));
        assert_eq!(ChatDate::try_new(""), Err(BlankContent));
        assert_eq!(ChatDate::try_new("  "), Err(BlankContent));
        assert!(serde_json::from_str::<ChatDate>("\"\"").is_err());
        assert!(serde_json::from_str::<ChatDate>("\" \"").is_err());
    }

    /// The model's refusal names every wrong component, with its text, so
    /// the validator only renders it.
    #[test]
    fn invalid_date_refusal_names_each_wrong_component() {
        let refusals = |text: &str| {
            let date = ChatDate::try_new(text).expect("nonblank");
            assert_eq!(date.as_str(), text);
            date.invalidity()
                .expect("unsupported")
                .refusals()
                .map(|refusal| format!("{refusal:?}"))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            refusals("01-JAN-2024-extra"),
            [format!("{:?}", DateRefusal::Shape("01-JAN-2024-extra"))]
        );
        assert_eq!(
            refusals("00-jan-24"),
            [
                format!("{:?}", DateRefusal::Day("00", DayProblem::OutOfRange)),
                format!(
                    "{:?}",
                    DateRefusal::Month("jan", MonthProblem::NotUppercase(Month::Jan))
                ),
                format!("{:?}", DateRefusal::Year("24", DateDigitProblem::Width)),
            ]
        );
        assert_eq!(
            refusals("1-XYZ-20a4"),
            [
                format!(
                    "{:?}",
                    DateRefusal::Day("1", DayProblem::Digits(DateDigitProblem::Width))
                ),
                format!("{:?}", DateRefusal::Month("XYZ", MonthProblem::Unknown)),
                format!(
                    "{:?}",
                    DateRefusal::Year("20a4", DateDigitProblem::NonDigit)
                ),
            ]
        );
        assert_eq!(
            refusals("32-JANUARY-2024"),
            [
                format!("{:?}", DateRefusal::Day("32", DayProblem::OutOfRange)),
                format!(
                    "{:?}",
                    DateRefusal::Month("JANUARY", MonthProblem::NotAbbreviation)
                ),
            ]
        );
        assert!(
            ChatDate::try_new("01-JAN-2024")
                .expect("nonblank")
                .invalidity()
                .is_none()
        );
    }
}
