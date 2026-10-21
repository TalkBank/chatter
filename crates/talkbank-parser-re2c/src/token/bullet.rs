//! Numeric admission is distinct from spelling, ordering and inline policy.

use serde::Serialize;

/// Millisecond counts admitted together, without inventing a value on overflow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct BulletTimes {
    start: u64,
    end: u64,
}

impl BulletTimes {
    /// The admitted counts; zero and non-increasing pairs remain policy questions.
    pub fn pair(self) -> (u64, u64) {
        (self.start, self.end)
    }
}

/// A lexical bullet retains its spelling even when numeric admission fails.
/// Private fields bind the checked result to the original lexer payload.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct MediaBulletToken<'a> {
    raw_text: &'a str,
    start_time: &'a str,
    end_time: &'a str,
    #[serde(skip)]
    admission: BulletAdmission,
}

/// The numeric admission of a bullet's two times. Source spelling and
/// locations stay with the owning token; rejected and absent slots stay
/// distinct.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
struct BulletAdmission(Result<BulletTimes, Overflow>);

impl BulletAdmission {
    /// The checked pair, if numeric admission succeeded.
    fn times(self) -> Option<BulletTimes> {
        self.0.ok()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
enum Overflow {
    Start,
    End,
    Both,
}

impl<'a> MediaBulletToken<'a> {
    pub(crate) fn from_lexed(raw_text: &'a str, start_time: &'a str, end_time: &'a str) -> Self {
        let start = start_time.parse::<u64>();
        let end = end_time.parse::<u64>();
        let admission = match (start, end) {
            (Ok(start), Ok(end)) => Ok(BulletTimes { start, end }),
            (Err(_), Ok(_)) => Err(Overflow::Start),
            (Ok(_), Err(_)) => Err(Overflow::End),
            (Err(_), Err(_)) => Err(Overflow::Both),
        };
        Self {
            raw_text,
            start_time,
            end_time,
            admission: BulletAdmission(admission),
        }
    }

    /// Original token bytes, including delimiters.
    pub fn text(self) -> &'a str {
        self.raw_text
    }

    /// Numeric admission, not CHAT validity. Failure remains an occupied slot.
    pub fn times(self) -> Option<BulletTimes> {
        self.admission.times()
    }

    /// Original component spellings, for source diagnostics.
    pub fn spellings(self) -> [(&'a str, &'static str); 2] {
        [(self.start_time, "start"), (self.end_time, "end")]
    }

    /// Exactly the components refused by numeric admission, without reparsing.
    pub(crate) fn overflows(self) -> impl Iterator<Item = (&'a str, &'static str)> {
        let failed = match self.admission.0 {
            Ok(_) => [false, false],
            Err(Overflow::Start) => [true, false],
            Err(Overflow::End) => [false, true],
            Err(Overflow::Both) => [true, true],
        };
        self.spellings()
            .into_iter()
            .zip(failed)
            .filter_map(|(component, failed)| failed.then_some(component))
    }
}
