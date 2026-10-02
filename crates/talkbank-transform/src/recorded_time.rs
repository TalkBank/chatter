//! [`RecordedTime`]: when a decision or record was made, in the form TalkBank's
//! TOML decision files (pending adjudications, merge overrides) store it.
//!
//! # Why a type, and why this form
//!
//! These files are read and edited by people as well as by chatter, so the
//! value is written for a person: New York time, whole seconds, with its UTC
//! offset, e.g. `2026-09-28T12:00:00-04:00`. The offset makes it an exact
//! instant; New York is fixed rather than the writing host's zone, so the same
//! decision reads the same whichever machine wrote it.
//!
//! Reading accepts every RFC 3339 spelling a tool has written or a person is
//! likely to type (`Z`, `+00:00`, any offset, any fraction, a space for `T`),
//! and in TOML a native datetime as well as a quoted string. A time with no
//! offset names no instant and is refused. Fractions are dropped on reading,
//! so a value always has whole seconds, and writing then reading is exact.
//!
//! The New York rules are compiled in (`jiff::tz::get!`), so formatting needs
//! no time zone database on the host and cannot fail; a change to New York's
//! rules needs a rebuild, and the written offset keeps every stored instant
//! exact regardless.

use std::fmt;
use std::str::FromStr;

use jiff::Timestamp;
use jiff::tz::{self, TimeZone};

/// New York's rules, compiled in.
static NEW_YORK: TimeZone = tz::get!("America/New_York");

/// An instant with whole seconds, written in New York time. See the module
/// docs for the form and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecordedTime(Timestamp);

impl RecordedTime {
    /// The current instant, to the second.
    pub fn now() -> Self {
        Self::from_timestamp(Timestamp::now())
    }

    /// `timestamp` with its fraction dropped (towards the Unix epoch).
    pub fn from_timestamp(timestamp: Timestamp) -> Self {
        // `constant` panics only out of range, and the whole seconds of a
        // valid instant are in range (`Timestamp::MIN` is a whole second).
        Self(Timestamp::constant(timestamp.as_second(), 0))
    }

    /// The instant.
    pub fn timestamp(self) -> Timestamp {
        self.0
    }
}

impl fmt::Display for RecordedTime {
    /// RFC 3339 in New York time with its offset: `2026-09-28T12:00:00-04:00`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let offset = NEW_YORK.to_offset(self.0);
        write!(f, "{}", self.0.display_with_offset(offset))
    }
}

/// A text that is not an RFC 3339 instant.
#[derive(Debug, thiserror::Error)]
#[error("not an RFC 3339 time with an offset: {text:?}")]
pub struct RecordedTimeError {
    text: String,
    #[source]
    source: jiff::Error,
}

impl FromStr for RecordedTime {
    type Err = RecordedTimeError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        text.parse::<Timestamp>()
            .map(Self::from_timestamp)
            .map_err(|source| RecordedTimeError {
                text: text.to_owned(),
                source,
            })
    }
}

impl serde::Serialize for RecordedTime {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// What a decision file may hold: a quoted string, or (in TOML) a native
/// datetime, which the `toml` crate hands over as its own type.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum WrittenTime {
    Text(String),
    TomlDatetime(toml::value::Datetime),
}

impl<'de> serde::Deserialize<'de> for RecordedTime {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = match WrittenTime::deserialize(deserializer)? {
            WrittenTime::Text(text) => text,
            WrittenTime::TomlDatetime(datetime) => datetime.to_string(),
        };
        text.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Written in New York time with its offset, whole seconds, whatever
    /// zone the writing host is in: summer and winter offsets both appear.
    #[test]
    fn written_in_new_york_time_with_its_offset_in_whole_seconds() {
        let summer: RecordedTime = "2026-09-28T16:00:00.987654Z".parse().unwrap();
        assert_eq!(summer.to_string(), "2026-09-28T12:00:00-04:00");
        let winter: RecordedTime = "2026-01-15T17:30:00Z".parse().unwrap();
        assert_eq!(winter.to_string(), "2026-01-15T12:30:00-05:00");
    }

    /// Every spelling a TalkBank tool has written, or a person is likely to
    /// type, reads as the same instant; fractions are dropped.
    #[test]
    fn every_rfc_3339_spelling_reads_as_the_same_instant() {
        let expected: RecordedTime = "2026-09-28T12:00:00-04:00".parse().unwrap();
        for spelling in [
            "2026-09-28T16:00:00Z",
            "2026-09-28T16:00:00+00:00",
            "2026-09-28T16:00:00.123456Z",
            "2026-09-28T16:00:00.5+00:00",
            "2026-09-28 16:00:00Z",
        ] {
            assert_eq!(
                spelling.parse::<RecordedTime>().unwrap(),
                expected,
                "{spelling}"
            );
        }
    }

    /// A time without an offset names no instant, so it is refused.
    #[test]
    fn a_time_without_an_offset_is_refused() {
        assert!("2026-09-28T12:00:00".parse::<RecordedTime>().is_err());
    }

    /// `now` is already whole seconds, so writing it and reading it back
    /// gives the same value.
    #[test]
    fn now_round_trips_through_its_written_form() {
        let now = RecordedTime::now();
        assert_eq!(now.to_string().parse::<RecordedTime>().unwrap(), now);
    }

    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Entry {
        at: RecordedTime,
    }

    /// In TOML it is written as a quoted string, and read back unchanged.
    #[test]
    fn toml_writes_a_quoted_string_and_reads_it_back() {
        let entry = Entry {
            at: "2026-09-28T16:00:00Z".parse().unwrap(),
        };
        let text = toml::to_string(&entry).unwrap();
        assert_eq!(text, "at = \"2026-09-28T12:00:00-04:00\"\n");
        assert_eq!(toml::from_str::<Entry>(&text).unwrap(), entry);
    }

    /// A person editing the file may write a native TOML datetime, as the
    /// book's examples do; that reads too.
    #[test]
    fn toml_reads_a_native_datetime() {
        let entry: Entry = toml::from_str("at = 2026-09-28T12:00:00-04:00\n").unwrap();
        assert_eq!(entry.at, "2026-09-28T16:00:00Z".parse().unwrap());
    }

    /// A native TOML datetime without an offset is refused like a string one.
    #[test]
    fn toml_refuses_a_native_datetime_without_an_offset() {
        assert!(toml::from_str::<Entry>("at = 2026-09-28T12:00:00\n").is_err());
    }
}
