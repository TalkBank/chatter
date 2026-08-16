//! The value types and shared readers a spec file's metadata needs.
//!
//! Two closed enums ([`Status`], [`SpecLayer`]), two validated open newtypes
//! ([`SpecErrorCode`], [`CategoryName`]), and the readers both spec parsers
//! share ([`parse_spec_title`], which reads the H1 rather than the metadata
//! block, and [`expected_error_codes`]).
//!
//! # Why these are not in either parser
//!
//! `Status` lived in [`super::error_corpus`], typed, while [`super::error`] held
//! the SAME field as a raw `String`. SEVEN consumers each decided for themselves
//! what it meant: three enums and four raw string comparisons. One of those
//! comparisons decided whether a GENERATED TEST got `#[ignore]`, so a misspelled
//! `Status` in a spec file silently un-ignored tests instead of failing to load.
//!
//! (An earlier draft of this paragraph said "six consumers" one line above
//! enumerating seven. A count stated beside the list it counts is the first
//! defect shape this workspace names, committed in the module written to remove
//! a milder version of it.)
//!
//! A closed set held as a `String` by its owner is the shape this workspace calls
//! a value proxying for a richer fact. The vocabulary belongs to the FORMAT, not
//! to whichever parser happened to type it first, so it lives here and both
//! SPEC-TOOLS parsers read it from one place.
//!
//! Not repo-wide, and the qualifier is load-bearing: a fourth copy of the same
//! closed set survives at `crates/talkbank-parser-tests/src/error_specs.rs`, in
//! the root workspace, which cannot depend on this crate. The goal is one
//! vocabulary both cargo workspaces can reach; this module is a step toward
//! that within one workspace, not the arrival. (An earlier draft deferred to
//! "the design's R6". That document is not in this repo, and a pointer a
//! stranger who forked this cannot follow is not a pointer.)

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::comrak_text::normalize_whitespace;

/// Raised when a metadata value fails the rule its type states.
#[derive(Debug, thiserror::Error)]
#[error("unknown {field} value {value:?}")]
pub struct UnknownMetadataValue {
    field: &'static str,
    value: String,
}

/// Implementation status of an error spec: whether the validator actually
/// checks its rule. The runner asserts `Implemented` examples fire and skips
/// the rest.
/// # There is deliberately no `Default`
///
/// There was one, `#[default] Implemented`, and removing it is the point of
/// this note. `error.rs` had already made the `**Status**` bullet REQUIRED,
/// naming the file that lacks it, on the reasoning that 104 of 238 specs once
/// declared nothing and had an answer invented for them. When the vocabulary
/// moved here, that `Default` came with it and `error_corpus.rs` kept reaching
/// for it -- so the invented answer had been promoted from one parser's habit
/// into a property of the FORMAT, which is where it would have looked
/// authoritative.
///
/// A `Default` whose wrong value is invisible is banned by name in this
/// project. Both parsers now require the bullet, and they agree by type rather
/// than by two docstrings that said different things.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Rule implemented; each example must produce its declared codes.
    Implemented,
    /// Rule not implemented yet; examples are skipped by the runner.
    NotImplemented,
    /// Code deprecated/replaced by another; examples are skipped.
    Deprecated,
    /// Rule IS implemented, but no CHAT input can trigger it, so it cannot
    /// have a fixture and the corpus gate must not demand one.
    ///
    /// This exists because the honest alternatives were both wrong. Leaving
    /// such a spec as `Implemented` with no example made it vanish from the
    /// loader entirely (see `load_all`), taking it out of reach of the very
    /// gate meant to catch an untested rule; marking it `NotImplemented`
    /// would state something false about a rule that fires. A spec in this
    /// state owes a NAMED out-of-corpus regression test in its status note,
    /// since the corpus cannot carry one.
    ///
    /// First user: E768, whose value cannot appear in a `.cha` file because
    /// both parsers end an `@Media` filename at the comma.
    UnreachableFromChat,
}

impl Status {
    /// The spelling a spec file uses, which is also what `Display` renders.
    ///
    /// ONE table. `FromStr` reads it and `Display` writes it, so the parser and
    /// the reports cannot disagree about what a status is called, and a renamed
    /// variant changes both at once.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Implemented => "implemented",
            Self::NotImplemented => "not_implemented",
            Self::Deprecated => "deprecated",
            Self::UnreachableFromChat => "unreachable_from_chat",
        }
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Status {
    type Err = UnknownMetadataValue;

    /// A plain match, the same shape as [`SpecLayer::from_str`] below.
    ///
    /// This was briefly written as a `find` over a `Status::ALL` array, with a
    /// doc claiming it was "derived from `as_str` so the two cannot drift". That
    /// was false: `ALL` is a second hand-maintained list, of variants instead of
    /// strings, and nothing checks it for completeness. A fifth variant would
    /// break `as_str`'s match at compile time -- which is the guard that matters
    /// -- while `ALL` stayed `[Self; 4]`, so `from_str` would silently reject the
    /// new spelling and every spec declaring it would fail to load. The array
    /// reintroduced one level up exactly the drift it was said to prevent, and
    /// it made the two enums in this file parse two different ways for no
    /// reason a reader could find.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "implemented" => Ok(Self::Implemented),
            "not_implemented" => Ok(Self::NotImplemented),
            "deprecated" => Ok(Self::Deprecated),
            "unreachable_from_chat" => Ok(Self::UnreachableFromChat),
            other => Err(UnknownMetadataValue {
                field: "Status",
                value: other.to_owned(),
            }),
        }
    }
}

/// The layer a spec's rule lives at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecLayer {
    /// Grammar/parser-level error.
    #[default]
    Parser,
    /// Semantic validation-level error.
    Validation,
}

impl SpecLayer {
    /// Whether this spec contributes a validation fixture.
    pub fn is_validation(self) -> bool {
        matches!(self, Self::Validation)
    }

    /// The spelling a spec file uses, which is also what `Display` renders.
    ///
    /// NOT a pure inverse of [`FromStr`], deliberately: that also accepts
    /// `parser|validation` for a spec flagged as both, and maps it to
    /// `Validation`. Writing is one answer, reading accepts two, and the
    /// asymmetry is real rather than an oversight.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Parser => "parser",
            Self::Validation => "validation",
        }
    }
}

impl std::fmt::Display for SpecLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for SpecLayer {
    type Err = UnknownMetadataValue;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "parser" => Ok(Self::Parser),
            // A spec flagged as both is treated as validation for generation.
            "validation" | "parser|validation" => Ok(Self::Validation),
            other => Err(UnknownMetadataValue {
                field: "Layer",
                value: other.to_owned(),
            }),
        }
    }
}

/// A CHAT error/warning code as written in a spec: `E` or `W` followed by three
/// digits.
///
/// # Every route in, enumerated
///
/// [`FromStr`] is the only one that can produce a value. [`Self::parse`] is a
/// yes/no wrapper over it, and `Deserialize` is routed through `TryFrom<String>`
/// so a code read back from `manifest.json` is held to the same rule as one read
/// from markdown. Without that last line the derive would have built one from
/// any JSON string, which is a forgeable proof rather than a proof, and the
/// manifest is exactly the round trip where it would have mattered.
///
/// # Why this lives beside the closed vocabularies
///
/// It is not a closed set, so it is a validated newtype rather than an enum.
/// It is here for the same reason [`Status`] and [`SpecLayer`] are: it is a
/// value type belonging to the spec FORMAT, and it was previously owned by
/// [`super::error_corpus`] while [`super::error`], the OTHER parser of the same
/// files, held the same field as a bare `String` with no validation at all.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SpecErrorCode(String);

/// Is this token the exact `E###` / `W###` shape?
///
/// The single owner of the rule. [`SpecErrorCode::parse`] and [`FromStr`] both
/// ask it, so the yes/no path allocates nothing: asking `FromStr` and dropping
/// its reason built an owned error for every prose word in an
/// `**Expected Error Codes**` line.
fn is_exact_code(token: &str) -> bool {
    let bytes = token.as_bytes();
    bytes.len() == 4 && matches!(bytes[0], b'E' | b'W') && bytes[1..].iter().all(u8::is_ascii_digit)
}

/// Was this token plainly MEANT to be an error code? `E` or `W` then a digit.
///
/// Deliberately looser than [`is_exact_code`]: its job is to separate a typo
/// from an ordinary word, so it must accept exactly the things that are wrong.
/// Also the filename test for "is this file an error spec", which is the same
/// question asked of a stem rather than a token.
pub fn looks_like_a_code(token: &str) -> bool {
    let mut chars = token.chars();
    matches!(chars.next(), Some('E' | 'W')) && chars.next().is_some_and(|c| c.is_ascii_digit())
}

impl SpecErrorCode {
    /// Parse an `E###`/`W###` token; `None` if it is not well formed.
    ///
    /// For the one caller that genuinely wants a yes/no verdict on an arbitrary
    /// token out of prose. A caller reading a spec FILE wants [`FromStr`],
    /// whose error says what was wrong and can be reported against the file.
    pub fn parse(token: &str) -> Option<Self> {
        let token = token.trim();
        is_exact_code(token).then(|| Self(token.to_owned()))
    }

    /// The underlying `E###`/`W###` text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The hundred-block this code sits in, as the docs render it: `E2x`.
    ///
    /// Derived rather than stored. It was a `range: String` field on
    /// `ErrorMetadata`, computed once at load and never authored, whose own doc
    /// claimed `"E200-E299"` while the code produced `"E2x"`. Infallible:
    /// [`FromStr`] admits nothing shorter than two characters.
    pub fn hundred_block(&self) -> String {
        format!("{}x", &self.0[..2])
    }
}

impl FromStr for SpecErrorCode {
    type Err = UnknownMetadataValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value).ok_or_else(|| UnknownMetadataValue {
            field: "error code",
            value: value.trim().to_owned(),
        })
    }
}

impl TryFrom<String> for SpecErrorCode {
    type Error = UnknownMetadataValue;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<SpecErrorCode> for String {
    fn from(code: SpecErrorCode) -> Self {
        code.0
    }
}

impl fmt::Display for SpecErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Read the codes an `**Expected Error Codes**: ...` line declares.
///
/// # Why this is here and not in either parser
///
/// Both spec parsers read the SAME files and, until this function existed, each
/// decided for itself what that line meant. `error.rs` split on `,` and refused
/// any token that did not parse; `error_corpus.rs` split on non-alphanumerics
/// and refused only near-misses. So `E301 and E305` yielded two codes in one
/// reader and a hard load failure in the other, and a spec author's verdict
/// depended on which generator ran. Typing the VALUE while leaving the
/// EXTRACTION forked is the same defect one level up.
///
/// The tolerant rule wins, because the line is prose: tokenize on
/// non-alphanumerics so `E301 and E305` and `E301, E305.` both read correctly,
/// and refuse only a token that plainly MEANT to be a code and is not
/// ([`looks_like_a_code`]). A near-miss is refused rather than dropped: dropping
/// leaves the example asserting less than it appears to.
pub fn expected_error_codes(text: &str) -> Result<Vec<SpecErrorCode>, String> {
    let Some(idx) = text.find("Expected Error Codes") else {
        return Ok(Vec::new());
    };
    text[idx..]
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|token| looks_like_a_code(token))
        .map(|token| {
            token.parse::<SpecErrorCode>().map_err(|_| {
                format!(
                    "`Expected Error Codes` names {token:?}, which is not an E### \
                     or W### code."
                )
            })
        })
        .collect()
}

/// The category grouping a spec declares (`validation`, `header_validation`,
/// ...): an open set, so a validated newtype rather than an enum.
///
/// # The VALUES are not normalised, deliberately
///
/// Measured 2026-08-15: all 236 loaded specs declare one, and there are 39
/// distinct values with mixed conventions. Two of them are the same concept
/// (`header_validation`, 25, and `Header validation`, 13) and at least one is
/// a description rather than a category (`Alignment count mismatch`, 14).
/// `docs/errors/index.md` renders one `##` section per distinct value, so that
/// collision is visible in published output.
///
/// Typing the field is a refactor; deciding what the categories ARE is a
/// CHAT-domain ruling, and the two are separated on purpose so the first does
/// not wait on the second. This type is where the normalisation will go when
/// that ruling exists.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CategoryName(String);

impl FromStr for CategoryName {
    type Err = UnknownMetadataValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let name = value.trim();
        if name.is_empty() {
            return Err(UnknownMetadataValue {
                field: "category",
                value: value.to_owned(),
            });
        }
        Ok(Self(name.to_owned()))
    }
}

impl TryFrom<String> for CategoryName {
    type Error = UnknownMetadataValue;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<CategoryName> for String {
    fn from(name: CategoryName) -> Self {
        name.0
    }
}

impl fmt::Display for CategoryName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a spec file's `# ...` heading declares: a name, and USUALLY a code.
///
/// One owner, because the two parsers of these same files had two rules.
/// `error.rs` took the first whitespace token and stripped a trailing `:`;
/// `error_corpus.rs` split at the first `:` or `,`. Measured over all 236
/// specs on 2026-08-15, they agree on 225 titles and differ on 11, all written
/// `# E209, name`, where the first rule yields `"E209,"` and fails to parse.
/// Those 11 are masked only because each also carries an `- **Error Code**:`
/// bullet that `error.rs` prefers, so its weaker route is never reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecTitle {
    /// The code the heading names, when it names one.
    ///
    /// `None` is a real fact about the input, not a sentinel: a heading may be
    /// a bare name whose code is declared by the `- **Error Code**:` bullet
    /// instead. The caller decides whether that is acceptable, because the two
    /// parsers legitimately differ there.
    pub code: Option<SpecErrorCode>,
    /// The heading's remainder, with the code and its separator removed, or
    /// the whole heading when it declares no code.
    pub name: String,
}

/// Parse a spec's `# E209, Something` heading.
///
/// # The separators, and why the fallback order is what it is
///
/// A code may be followed by `:` (the majority form), `,` (11 specs, from a
/// dash sweep that rewrote the separator) or ` - ` (the historical form). Split
/// at the FIRST of those.
///
/// The leading-token fallback exists for a heading with NO `:` or `,` whose
/// name contains a hyphen, such as `E304 Header out-of-order`: the split would
/// otherwise cut at that hyphen and fail to parse `"E304 Header out"`. It does
/// NOT fire for `E543: Header out-of-order`, which an earlier draft of this
/// paragraph claimed; `find` returns the minimum index, so the colon wins and
/// stage one resolves it. Measured 2026-08-15: of 238 headings, 236 resolve in
/// stage one and 2 (the non-spec files) reach the no-code case, so the
/// fallback is taken by NOTHING in the corpus and is covered by a test written
/// for it rather than by real data.
///
/// This ordering is a POLICY about what an author meant, not an invariant, so
/// it is a runtime rule with tests rather than something a type could hold.
pub fn parse_spec_title(heading: &str) -> SpecTitle {
    let heading = heading.trim();
    // First: a separator split, which handles `E209, name` and `E101: name`.
    if let Some(at) = heading.find([':', ',', '-'])
        && let Ok(code) = heading[..at].trim().parse::<SpecErrorCode>()
    {
        return SpecTitle {
            code: Some(code),
            name: normalize_whitespace(&heading[at + 1..]),
        };
    }

    // Then: a bare leading token, which handles `E304 Something` and lets a
    // hyphen inside the NAME survive.
    if let Some((first, rest)) = heading.split_once(char::is_whitespace)
        && let Ok(code) = first.trim_end_matches([':', ',']).parse::<SpecErrorCode>()
    {
        return SpecTitle {
            code: Some(code),
            name: normalize_whitespace(rest),
        };
    }

    // Otherwise the heading declares no code and is entirely a name.
    SpecTitle {
        code: None,
        name: normalize_whitespace(heading),
    }
}

#[cfg(test)]
mod spec_title_tests {
    use super::{SpecErrorCode, parse_spec_title};

    fn code(t: &str) -> Option<SpecErrorCode> {
        Some(t.parse().expect("test code is well formed"))
    }

    /// These cases moved here from a name-extraction helper in `error.rs`
    /// that `parse_spec_title` subsumed, 2026-08-15. They are kept in intent
    /// because each records a real title form in the corpus and, in two cases,
    /// a parse that has already been got wrong once.
    ///
    /// They are POLICY, not invariant: they pin what a human meant by a
    /// separator, which no type can decide.
    #[test]
    fn colon_is_the_majority_form() {
        let t = parse_spec_title("E101: Invalid line format");
        assert_eq!(t.code, code("E101"));
        assert_eq!(t.name, "Invalid line format");
    }

    /// Eleven specs carry the comma, from a dash sweep that rewrote the
    /// separator. It is what `error.rs`'s old rule could not read.
    #[test]
    fn comma_is_the_form_that_split_the_two_parsers() {
        let t = parse_spec_title("E248, Bare @s shortcut in tertiary language context");
        assert_eq!(t.code, code("E248"));
        assert_eq!(t.name, "Bare @s shortcut in tertiary language context");
    }

    /// The historical hyphen form still parses.
    #[test]
    fn hyphen_is_the_historical_form() {
        let t = parse_spec_title("E304 - Something went wrong");
        assert_eq!(t.code, code("E304"));
        assert_eq!(t.name, "Something went wrong");
    }

    /// A hyphen INSIDE the name must survive. An older implementation split on
    /// the first hyphen anywhere and returned "of" for this input. Resolved in
    /// stage one, because `find` returns the minimum index and the colon
    /// precedes the hyphen; see the sibling test for the case that actually
    /// needs the fallback.
    #[test]
    fn a_hyphen_inside_the_name_is_not_a_separator() {
        let t = parse_spec_title("E543: Header out-of-order");
        assert_eq!(t.code, code("E543"));
        assert_eq!(t.name, "Header out-of-order");
    }

    /// The leading-token fallback, which NOTHING in the corpus exercises: no
    /// `:` or `,`, and a hyphen inside the name. Without the fallback the
    /// separator split cuts at that hyphen, tries to parse `"E304 Header out"`
    /// and fails, and the heading would report no code at all.
    ///
    /// Written 2026-08-15 after a review measured the branch as taken by 0 of
    /// 238 headings and 0 of the five tests then present.
    #[test]
    fn a_heading_with_no_separator_falls_back_to_the_leading_token() {
        let t = parse_spec_title("E304 Header out-of-order");
        assert_eq!(t.code, code("E304"));
        assert_eq!(t.name, "Header out-of-order");
    }

    /// A heading with no code is a NAME, not a failure: the code may be
    /// declared by the `- **Error Code**:` bullet instead. `error.rs` accepts
    /// this and `error_corpus.rs` rejects it, which is why the code is an
    /// `Option` here rather than each caller re-deciding.
    #[test]
    fn a_heading_with_no_code_is_all_name() {
        let t = parse_spec_title("UnknownBaseContent");
        assert_eq!(t.code, None);
        assert_eq!(t.name, "UnknownBaseContent");
    }
}
