//! Typed time values for `@Time Duration` and `@Time Start` headers.
//!
//! Shared types: [`TimeValue`] represents a single parsed time point,
//! [`TimeSegment`] represents a single time or a range, and both are
//! re-used by `@Time Duration`, `@Time Start`, and the `%tim` tier.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Time_Duration_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Time_Start_Header>

use crate::model::{HeaderText, NonEmptyString};
use smol_str::SmolStr;
use talkbank_derive::{SemanticEq, SpanShift, ValidationTagged};

// ---------------------------------------------------------------------------
// Shared structured time types
// ---------------------------------------------------------------------------

/// A single parsed time point (e.g. `01:23:45` or `23:45.678`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SemanticEq, SpanShift)]
pub struct TimeValue {
    /// Hours component (0 for `MM:SS` format).
    #[span_shift(skip)]
    pub hours: u32,
    /// Minutes component.
    #[span_shift(skip)]
    pub minutes: u32,
    /// Seconds component.
    #[span_shift(skip)]
    pub seconds: u32,
    /// Optional milliseconds component.
    #[span_shift(skip)]
    pub millis: Option<u32>,
}

impl TimeValue {
    /// Returns true if every component is a legal clock value: hours 0-23,
    /// minutes 0-59, seconds 0-59, milliseconds 0-999.
    ///
    /// CLAN's `@t<hh:mm:ss>` time template treats these fields as clock values,
    /// not free integers: verified empirically, CLAN `check` reports error 35
    /// ("illegal time representation") for `25:00:00`, `12:60:00`, `12:00:60`,
    /// and `99:99:99`, while accepting `23:59:59`. A well-formed shape can still
    /// carry an out-of-range component, so this is checked separately from the
    /// depfile-pattern (shape) check.
    pub fn is_in_clock_range(&self) -> bool {
        self.hours <= 23
            && self.minutes <= 59
            && self.seconds <= 59
            && self.millis.is_none_or(|m| m <= 999)
    }
}

/// A time segment: either a single time or a range of two times.
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift)]
pub enum TimeSegment {
    /// A single time point (e.g. `01:23:45`).
    Single(TimeValue),
    /// A range between two time points (e.g. `00:00:00-01:30:00`).
    Range {
        /// Start of the range.
        start: TimeValue,
        /// End of the range.
        end: TimeValue,
    },
}

// ---------------------------------------------------------------------------
// TimeDurationValue
// ---------------------------------------------------------------------------

/// Duration value from `@Time Duration`.
///
/// Formats: `HH:MM:SS`, `HH:MM:SS-HH:MM:SS`, `HH:MM-HH:MM`,
/// comma-separated combinations, and ranges with `;` separator.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Time_Duration_Header>
/// Constructors own agreement between text and time components.
///
/// ```compile_fail
/// use talkbank_model::model::TimeDurationValue;
/// let value = TimeDurationValue::Parsed {
///     segments: Vec::new(), raw: "25:00:00".into(),
/// };
/// assert!(value.has_out_of_range_component());
/// ```
#[derive(Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift)]
pub struct TimeDurationValue {
    state: TimeDurationState,
}

impl std::fmt::Debug for TimeDurationValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.state, f)
    }
}

impl crate::model::ValidationTagged for TimeDurationValue {
    fn validation_tag(&self) -> crate::model::ValidationTag {
        crate::model::ValidationTagged::validation_tag(&self.state)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift, ValidationTagged)]
enum TimeDurationState {
    /// Successfully parsed time duration with structured segments.
    Parsed {
        /// Structured time segments extracted from the text.
        #[span_shift(skip)]
        segments: ParsedDurationSegments,
        /// Original text preserved for roundtrip.
        #[semantic_eq(skip)]
        #[span_shift(skip)]
        raw: SmolStr,
    },
    /// Unrecognized nonempty value preserved for validation.
    Unsupported(NonEmptyString),
}

/// Refusal evidence issued only after duration shape and clock assessment.
/// Its text cannot be detached from the assessed value or replaced by a caller.
pub(crate) struct InvalidTimeDuration<'a> {
    text: &'a str,
}

impl<'a> InvalidTimeDuration<'a> {
    pub(crate) fn into_text(self) -> &'a str {
        self.text
    }
}

impl TimeDurationValue {
    /// Assess shape and clock range, issuing a refusal for an invalid value.
    pub(crate) fn invalidity(&self) -> Option<InvalidTimeDuration<'_>> {
        let text = self.as_str();
        let invalid = match &self.state {
            TimeDurationState::Unsupported(_) => true,
            TimeDurationState::Parsed { .. } => {
                self.violates_depfile_pattern() || self.has_out_of_range_component()
            }
        };
        invalid.then_some(InvalidTimeDuration { text })
    }

    /// Parse CHAT time duration content.
    ///
    /// The text is parsed or retained as unsupported; a duration header with
    /// nothing after its colon is `Header::EmptyContent`, never this value.
    /// Every comma-separated segment must parse; an empty or malformed segment
    /// retains the entire value as unsupported rather than exposing a partial list.
    pub fn from_content(value: HeaderText<'_>) -> Self {
        let state = match ParsedDurationSegments::parse(value.as_str()) {
            Some(segments) => TimeDurationState::Parsed {
                segments,
                raw: SmolStr::from(value.as_str()),
            },
            None => TimeDurationState::Unsupported(value.as_non_empty().into_owned()),
        };
        Self { state }
    }

    /// Returns the time duration as a string.
    pub fn as_str(&self) -> &str {
        match &self.state {
            TimeDurationState::Parsed { raw, .. } => raw.as_str(),
            TimeDurationState::Unsupported(s) => s.as_str(),
        }
    }

    /// Returns the structured segments (empty for unsupported text).
    pub fn segments(&self) -> &[TimeSegment] {
        match &self.state {
            TimeDurationState::Parsed { segments, .. } => segments.as_slice(),
            TimeDurationState::Unsupported(_) => &[],
        }
    }

    /// Returns true if any parsed time component is outside clock range
    /// (hours 0-23, minutes/seconds 0-59). The shape may be well-formed
    /// (`HH:MM:SS`) yet carry impossible values like `99:99:99`; CLAN flags
    /// these as error 35 ("illegal time representation"), so this is a separate
    /// check from [`violates_depfile_pattern`](Self::violates_depfile_pattern).
    pub fn has_out_of_range_component(&self) -> bool {
        self.segments().iter().any(|segment| match segment {
            TimeSegment::Single(time) => !time.is_in_clock_range(),
            TimeSegment::Range { start, end } => {
                !start.is_in_clock_range() || !end.is_in_clock_range()
            }
        })
    }

    /// Returns true when a structurally-parseable duration does not match
    /// any of the three patterns that CLAN's authoritative `depfile.cut`
    /// declares legal for `@Time Duration`:
    ///
    /// ```text
    /// @t<hh:mm-hh:mm>  @t<hh:mm:ss-hh:mm:ss>  @t<hh:mm:ss>
    /// ```
    ///
    /// Concretely, the raw text must be exactly one of:
    ///
    /// - `HH:MM:SS`
    /// - `HH:MM-HH:MM`
    /// - `HH:MM:SS-HH:MM:SS`
    ///
    /// Anything else, semicolon separator, comma-joined multi-segment
    /// values, `MM:SS` two-component form, is rejected by CLAN CHECK,
    /// and this predicate mirrors that. `Unsupported` is caught by
    /// `has_validation_issue()` so this method returns `false` for it
    /// to avoid double-reporting.
    ///
    /// Component widths are not fixed (`\d+`, not `\d{2}`) because
    /// real corpora use both `1:30` and `01:30`; depfile.cut's slots
    /// admit any digit count.
    pub fn violates_depfile_pattern(&self) -> bool {
        let TimeDurationState::Parsed { raw, .. } = &self.state else {
            return false;
        };
        !matches_duration_depfile(raw.as_str())
    }
}

/// Single-time form `HH:MM:SS`, the `@t<hh:mm:ss>` template.
fn is_hms_strict(s: &str) -> bool {
    let mut parts = s.split(':');
    let Some(h) = parts.next() else { return false };
    let Some(m) = parts.next() else { return false };
    let Some(sec) = parts.next() else {
        return false;
    };
    if parts.next().is_some() {
        return false;
    }
    all_ascii_digits(h) && all_ascii_digits(m) && all_ascii_digits(sec)
}

/// Two-component form `HH:MM`, used on one side of an
/// `HH:MM-HH:MM` range.
fn is_hm_strict(s: &str) -> bool {
    let mut parts = s.split(':');
    let Some(h) = parts.next() else { return false };
    let Some(m) = parts.next() else { return false };
    if parts.next().is_some() {
        return false;
    }
    all_ascii_digits(h) && all_ascii_digits(m)
}

fn all_ascii_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn matches_duration_depfile(raw: &str) -> bool {
    // HH:MM:SS
    if is_hms_strict(raw) {
        return true;
    }
    // HH:MM-HH:MM or HH:MM:SS-HH:MM:SS, a single hyphen at top level
    // with matching component shapes on both sides. Not semicolon,
    // not comma.
    if let Some((left, right)) = raw.split_once('-')
        && !right.contains('-')
    {
        return (is_hm_strict(left) && is_hm_strict(right))
            || (is_hms_strict(left) && is_hms_strict(right));
    }
    false
}

/// Parse comma-separated time segments from a duration string.
/// Complete, nonempty segment admission. No mutable view can empty this proof.
#[derive(Clone, PartialEq, Eq, Hash, SemanticEq)]
struct ParsedDurationSegments(Vec<TimeSegment>);

impl ParsedDurationSegments {
    fn parse(duration: &str) -> Option<Self> {
        let mut parts = duration.split(',');
        let first = parse_duration_segment(parts.next()?)?;
        let mut segments = vec![first];
        for part in parts {
            segments.push(parse_duration_segment(part)?);
        }
        Some(Self(segments))
    }

    fn as_slice(&self) -> &[TimeSegment] {
        &self.0
    }
}

impl std::fmt::Debug for ParsedDurationSegments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}

fn parse_duration_segment(segment: &str) -> Option<TimeSegment> {
    let segment = segment.trim();
    if let Some((left, right)) = segment.split_once('-') {
        Some(TimeSegment::Range {
            start: parse_time_value_with_pair(left, TimePairMeaning::HoursMinutes)?,
            end: parse_time_value_with_pair(right, TimePairMeaning::HoursMinutes)?,
        })
    } else if let Some((left, right)) = segment.split_once(';') {
        Some(TimeSegment::Range {
            start: parse_time_value_with_pair(left, TimePairMeaning::HoursMinutes)?,
            end: parse_time_value_with_pair(right, TimePairMeaning::HoursMinutes)?,
        })
    } else {
        parse_time_value(segment).map(TimeSegment::Single)
    }
}

/// Parse a single `HH:MM:SS`, `MM:SS`, bare `SS`, or any with `.mmm` millis.
///
/// depfile.cut defines `@t<ss>` (bare seconds) as valid for `%tim`, so we
/// accept 1-part, 2-part, and 3-part time values.
pub(crate) fn parse_time_value(s: &str) -> Option<TimeValue> {
    parse_time_value_with_pair(s, TimePairMeaning::MinutesSeconds)
}

/// The owning time field selects the meaning before components are constructed.
/// A two-component duration endpoint cannot silently inherit `%tim` semantics.
#[derive(Clone, Copy)]
enum TimePairMeaning {
    HoursMinutes,
    MinutesSeconds,
}

fn parse_time_value_with_pair(s: &str, pair: TimePairMeaning) -> Option<TimeValue> {
    let (hms_part, millis) = if let Some((hms, ms)) = s.split_once('.') {
        let millis: u32 = ms.parse().ok()?;
        (hms, Some(millis))
    } else {
        (s, None)
    };

    let parts: Vec<&str> = hms_part.split(':').collect();
    match parts.len() {
        1 => {
            let seconds: u32 = parts[0].parse().ok()?;
            Some(TimeValue {
                hours: 0,
                minutes: 0,
                seconds,
                millis,
            })
        }
        2 => {
            let first: u32 = parts[0].parse().ok()?;
            let second: u32 = parts[1].parse().ok()?;
            Some(match pair {
                TimePairMeaning::HoursMinutes => TimeValue {
                    hours: first,
                    minutes: second,
                    seconds: 0,
                    millis,
                },
                TimePairMeaning::MinutesSeconds => TimeValue {
                    hours: 0,
                    minutes: first,
                    seconds: second,
                    millis,
                },
            })
        }
        3 => {
            let hours: u32 = parts[0].parse().ok()?;
            let minutes: u32 = parts[1].parse().ok()?;
            let seconds: u32 = parts[2].parse().ok()?;
            Some(TimeValue {
                hours,
                minutes,
                seconds,
                millis,
            })
        }
        _ => None,
    }
}

crate::header_content_admission!(TimeDurationValue, render, schema);

// ---------------------------------------------------------------------------
// TimeStartValue
// ---------------------------------------------------------------------------

/// Starting time value from `@Time Start`.
///
/// Formats: `MM:SS`, `HH:MM:SS`, or either with `.mmm` milliseconds.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Time_Start_Header>
/// Constructors own agreement between text and time components.
///
/// ```compile_fail
/// use talkbank_model::model::TimeStartValue;
/// let value = TimeStartValue::Parsed {
///     hours: 0, minutes: 0, seconds: 0, millis: None,
///     raw: "25:00:00".into(),
/// };
/// assert_eq!(Ok(value), TimeStartValue::try_new(value.as_str()));
/// ```
#[derive(Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift)]
pub struct TimeStartValue {
    state: TimeStartState,
}

impl std::fmt::Debug for TimeStartValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.state, f)
    }
}

impl crate::model::ValidationTagged for TimeStartValue {
    fn validation_tag(&self) -> crate::model::ValidationTag {
        crate::model::ValidationTagged::validation_tag(&self.state)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift, ValidationTagged)]
enum TimeStartState {
    /// Successfully parsed time start.
    Parsed {
        /// Hours component.
        #[span_shift(skip)]
        hours: u32,
        /// Minutes component.
        #[span_shift(skip)]
        minutes: u32,
        /// Seconds component.
        #[span_shift(skip)]
        seconds: u32,
        /// Optional milliseconds component.
        #[span_shift(skip)]
        millis: Option<u32>,
        /// Original text preserved for roundtrip.
        #[semantic_eq(skip)]
        #[span_shift(skip)]
        raw: SmolStr,
    },
    /// Unrecognized nonempty value preserved for validation.
    Unsupported(NonEmptyString),
}

/// Refusal evidence issued only by the time-start assessment.
/// The borrowed text stays associated with the value that was assessed.
pub(crate) struct InvalidTimeStart<'a> {
    text: &'a str,
}

impl<'a> InvalidTimeStart<'a> {
    pub(crate) fn into_text(self) -> &'a str {
        self.text
    }
}

impl TimeStartValue {
    /// Assess syntax and clock bounds without discarding recovery text.
    pub(crate) fn invalidity(&self) -> Option<InvalidTimeStart<'_>> {
        let text = self.as_str();
        let invalid = match &self.state {
            TimeStartState::Unsupported(_) => true,
            TimeStartState::Parsed {
                hours,
                minutes,
                seconds,
                millis,
                ..
            } => {
                let clock = TimeValue {
                    hours: *hours,
                    minutes: *minutes,
                    seconds: *seconds,
                    millis: *millis,
                };
                self.violates_depfile_pattern() || !clock.is_in_clock_range()
            }
        };
        invalid.then_some(InvalidTimeStart { text })
    }

    /// Parse CHAT time start content (`MM:SS`, `HH:MM:SS`, or either with
    /// `.mmm`).
    ///
    /// The text is parsed or retained as unsupported; a start header with
    /// nothing after its colon is `Header::EmptyContent`, never this value.
    pub fn from_content(value: HeaderText<'_>) -> Self {
        let state = match parse_time_value(value.as_str()) {
            Some(tv) => TimeStartState::Parsed {
                hours: tv.hours,
                minutes: tv.minutes,
                seconds: tv.seconds,
                millis: tv.millis,
                raw: SmolStr::from(value.as_str()),
            },
            None => TimeStartState::Unsupported(value.as_non_empty().into_owned()),
        };
        Self { state }
    }

    /// Parsed components, absent for unsupported text.
    /// The returned copy cannot change the source value's classification.
    pub fn time(&self) -> Option<TimeValue> {
        match &self.state {
            TimeStartState::Parsed {
                hours,
                minutes,
                seconds,
                millis,
                ..
            } => Some(TimeValue {
                hours: *hours,
                minutes: *minutes,
                seconds: *seconds,
                millis: *millis,
            }),
            TimeStartState::Unsupported(_) => None,
        }
    }

    /// Returns the time start as a string.
    pub fn as_str(&self) -> &str {
        match &self.state {
            TimeStartState::Parsed { raw, .. } => raw.as_str(),
            TimeStartState::Unsupported(s) => s.as_str(),
        }
    }

    /// Returns true when a structurally-parseable start time does not
    /// match either of the two patterns that CLAN's authoritative
    /// `depfile.cut` declares legal for `@Time Start`:
    ///
    /// ```text
    /// @t<hh:mm:ss>  @t<mm:ss>
    /// ```
    ///
    /// Concretely, the raw text must be exactly one of:
    ///
    /// - `HH:MM:SS`
    /// - `MM:SS`
    ///
    /// Anything else, millisecond suffix, range form, bare seconds,
    /// is rejected by CLAN CHECK. `Unsupported` is already caught by
    /// `has_validation_issue()`, so this method returns `false` for
    /// it to avoid double-reporting.
    pub fn violates_depfile_pattern(&self) -> bool {
        let TimeStartState::Parsed { raw, .. } = &self.state else {
            return false;
        };
        let raw = raw.as_str();
        // HH:MM:SS or MM:SS, no dots, no hyphens, exactly 2 or 3
        // colon-separated numeric components.
        if raw.contains('.') || raw.contains('-') {
            return true;
        }
        if is_hms_strict(raw) {
            return false;
        }
        if is_hm_strict(raw) {
            return false;
        }
        true
    }
}

crate::header_content_admission!(TimeStartValue, render, schema);

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
