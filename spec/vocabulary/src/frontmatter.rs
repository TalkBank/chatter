//! The `+++` TOML frontmatter block: the ONE reader of a spec file's metadata.
//!
//! # What this replaced, and why the replacement is a deletion
//!
//! Until Phase 1b the metadata was `- **Field**: value` bullets inside a
//! `## Metadata` section, plus `**Field**: value` lines that had to sit BEFORE
//! an example's ```` ```chat ```` fence. That format had no schema, so:
//!
//! - an unrecognised label was retained silently, which is how five of them
//!   accumulated (`Status note`, `Last updated`, `Severity`, `Root Cause`,
//!   `Note`) alongside near-miss spellings of real fields;
//! - a field written AFTER the fence was invisible to the loader, so an
//!   example asserted nothing while reading as fully specified. E757 did
//!   exactly that in two examples, and the cure was a hand-written
//!   `raw_after_fence_declares_codes` guard: 30 lines of runtime check
//!   standing in for a type;
//! - the same field was matched by four regexes in two cargo workspaces.
//!
//! Frontmatter deletes all three at once. An example is now ONE value carrying
//! its own input, so there is no fence for a field to be on the wrong side of,
//! and [`serde`]'s `deny_unknown_fields` makes an unrecognised key a load
//! error rather than a retained map entry.
//!
//! # Why the schema lives in the vocabulary crate
//!
//! Both cargo workspaces read these files. The markdown READERS were allowed
//! to stay separate because a markdown parser does not belong in the main
//! workspace's test graph; a TOML deserialization of a struct defined here is
//! not that, and it is the whole metadata half of the format. One schema, two
//! callers, and `deny_unknown_fields` holds for both.

use serde::Deserialize;

use crate::registry::{CodeEntry, CodeRegistry, UnregisteredCode};
use crate::{SpecErrorCode, SpecLevel};

/// The delimiter line opening and closing the frontmatter block.
///
/// TOML frontmatter is conventionally `+++`, as `---` is YAML's. The choice
/// matters here beyond convention: a spec's example is deliberately INVALID
/// CHAT, so a future example could legitimately contain a line that collides
/// with the delimiter. [`split`] refuses that file rather than guessing, which
/// is why this is a constant a diagnostic can name.
pub const DELIMITER: &str = "+++";

/// A spec file failed to decompose into frontmatter and body.
#[derive(Debug, thiserror::Error)]
pub enum FrontmatterError {
    /// The file does not open with the delimiter.
    #[error(
        "no `{DELIMITER}` frontmatter: a spec file opens with a TOML block \
         declaring at least a code and a name"
    )]
    Missing,
    /// The opening delimiter is never closed.
    #[error("the opening `{DELIMITER}` is never closed by a line reading `{DELIMITER}`")]
    Unterminated,
    /// A `+++` line appears in the body, so the split point is ambiguous.
    ///
    /// The one case where scanning for a delimiter could silently pick the
    /// wrong line. Refused rather than resolved by a rule, because any rule
    /// would be guessing which occurrence the author meant.
    #[error(
        "line {line} of the body reads `{DELIMITER}`, so where the frontmatter \
         ends is ambiguous. A spec file may contain exactly one `{DELIMITER}` \
         pair, at the top"
    )]
    AmbiguousDelimiter {
        /// 1-based line number within the body.
        line: usize,
    },
    /// The frontmatter is not well-formed TOML, or violates the schema.
    #[error("frontmatter: {0}")]
    Toml(#[from] toml::de::Error),
}

/// A spec file's two halves: its declared metadata, and its prose.
#[derive(Debug)]
pub struct SpecSource<'a> {
    /// The TOML text between the delimiters, unparsed.
    pub frontmatter: &'a str,
    /// Everything after the closing delimiter, VERBATIM.
    ///
    /// Borrowed from the original text rather than rebuilt, so a reader cannot
    /// accidentally normalize a spec's prose while looking at its metadata.
    pub body: &'a str,
}

/// Split a spec file into its frontmatter and its body.
///
/// # Errors
///
/// When the file does not open with a `+++` line, when that line is never
/// closed, or when the body contains a further `+++` line, which would make
/// the split point ambiguous.
pub fn split(text: &str) -> Result<SpecSource<'_>, FrontmatterError> {
    let after_open = text
        .strip_prefix(DELIMITER)
        .and_then(|rest| rest.strip_prefix('\n'))
        .ok_or(FrontmatterError::Missing)?;

    // ONE pass, and one spelling of "is this line the delimiter". It was two
    // of each: a scan for the closing line, then a second full walk of the
    // body looking for a further one, with the two tests trimming different
    // characters. The body's line number falls out of the indices, so the
    // second walk bought nothing but a way for the two tests to drift.
    let mut offset = 0;
    let mut closing: Option<(usize, usize, usize)> = None;
    for (index, line) in after_open.split_inclusive('\n').enumerate() {
        if line.trim_end_matches(['\r', '\n']) == DELIMITER {
            match closing {
                None => closing = Some((offset, offset + line.len(), index)),
                // A second one makes the split point ambiguous. Refused rather
                // than resolved by a first-match rule, because any rule would
                // be guessing which occurrence the author meant.
                Some((_, _, first)) => {
                    return Err(FrontmatterError::AmbiguousDelimiter {
                        line: index - first,
                    });
                }
            }
        }
        offset += line.len();
    }
    let (end, body_start, _) = closing.ok_or(FrontmatterError::Unterminated)?;

    Ok(SpecSource {
        frontmatter: &after_open[..end],
        body: &after_open[body_start..],
    })
}

/// Read a spec file: its frontmatter, deserialized, and its prose body.
///
/// # The verb this crate was missing
///
/// [`split`] is a well-typed noun with no verb, so both readers of these files
/// wrote the same two steps by hand and each invented its own error text for
/// them. That left [`FrontmatterError::Toml`] unconstructible: the variant
/// describing the second half existed while nothing performed the second half
/// here. One function makes the sequence one decision, and the variant
/// reachable.
///
/// # Errors
///
/// When the file carries no well-formed `+++` block, or its contents violate
/// the schema.
pub fn read(text: &str) -> Result<(SpecFrontmatter, &str), FrontmatterError> {
    let source = split(text)?;
    Ok((toml::from_str(source.frontmatter)?, source.body))
}

/// Read a spec file AND resolve the code it names, in one decision.
///
/// # The same verb, one step further on
///
/// [`read`] exists because `split` was a well-typed noun with no verb, so both
/// readers of these files wrote the same two steps by hand and each invented
/// its own error text. R1 added a third step, resolving the declared code
/// against the registry, and both readers immediately wrote THAT by hand too,
/// in the same two files, with two more error spellings. One function again.
///
/// It also states something the split form could not: the returned
/// [`SpecFrontmatter`] and [`CodeEntry`] came from the SAME read of the same
/// text against the registry the caller supplied. Two values returned
/// separately are related only by the caller's discipline.
///
/// # Errors
///
/// When the file carries no well-formed `+++` block, its contents violate the
/// schema, or `registry` does not declare the code it names.
pub fn read_resolved<'a>(
    text: &'a str,
    registry: &'a CodeRegistry,
) -> Result<(SpecFrontmatter, &'a CodeEntry, &'a str), ResolvedReadError> {
    let (front, body) = read(text)?;
    let entry = registry.resolve(&front.code)?;
    Ok((front, entry, body))
}

/// A spec file could not be read, or names a code nothing declares.
#[derive(Debug, thiserror::Error)]
pub enum ResolvedReadError {
    /// The `+++` block is absent, malformed, or violates the schema.
    #[error(transparent)]
    Frontmatter(#[from] FrontmatterError),
    /// The file parses and names a code the registry does not declare.
    #[error(transparent)]
    Unregistered(#[from] UnregisteredCode),
}

/// What a spec file's frontmatter declares.
///
/// # A spec file describes a code; it does not DEFINE one
///
/// `kind` and `status` were fields here until R1. They are facts about the
/// CODE, so every one of a code's spec files carried a copy and a generator
/// had to check they agreed; eleven codes have more than one file. They live
/// in [`crate::registry`] now, and [`Self::code`] is the foreign key that
/// reaches them. The `Kind` type parameter went with them: it existed only
/// because the format could not name `kind`'s value type, and there is
/// nothing left for it to abstract over.
///
/// # Every field is REQUIRED unless its type says otherwise
///
/// No `#[serde(default)]` on `code`, nor on the per-example `level` and
/// `claim`. Each was made required in the bullet format one at a time, each
/// time after a default had silently answered for a spec nobody had written
/// yet; see [`crate::Status`], which has no `Default` for that reason.
/// (`layer` was
/// in this list until R4 deleted the field outright, and `kind`/`status`
/// until R1 moved them.)
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecFrontmatter {
    /// The code this spec DOCUMENTS, and the key into [`crate::registry`].
    ///
    /// Several spec files may name one code; none of them owns the code's own
    /// facts. Resolving this against the registry is what proves the file
    /// describes a code that exists, and a loader that resolves it can offer
    /// `kind` and `status` as accessors rather than as fields anyone can set.
    ///
    /// Declared, never derived from the filename or the heading. The bullet
    /// format had three routes to it (the `- **Error Code**:` bullet, the H1,
    /// and an `unwrap_or_default()` that once produced the EMPTY code), and
    /// `parse_spec_title` existed to absorb three heading separator dialects.
    /// One declaration deletes all of it.
    pub code: SpecErrorCode,
    /// The spec's short name, which was the remainder of its H1.
    pub name: String,
    /// A human's adjudication of the code's current state.
    ///
    /// The one unmodelled label worth keeping of the five the bullet format
    /// accumulated: which code fires instead, what was tested, and in one case
    /// a correction recorded in place. `Last updated` and `Severity` were
    /// dropped (git holds the first; the second is computed from kind and
    /// profile, so a spec cannot state it), and `Root Cause` and `Note` are
    /// prose that belongs in the body.
    #[serde(default)]
    pub status_note: Option<String>,
    /// The examples, in file order.
    ///
    /// `example` singular in the file, because TOML spells an array of tables
    /// `[[example]]` and the format should read the way it is written.
    #[serde(default, rename = "example")]
    pub examples: Vec<ExampleFrontmatter>,
}

/// One example: an input, and everything asserted about it.
///
/// # The whole point of Phase 1b is that this is ONE value
///
/// The input and its expectations used to be two things joined by POSITION:
/// a paragraph, and the fence below it. Nothing in any type said they belonged
/// together, so a field written below the fence type-checked, parsed, and
/// asserted nothing.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExampleFrontmatter {
    /// A short label for this example, when it has one.
    ///
    /// 30 of the 334 `## Example N` headings carried a title after the
    /// ordinal (`Simple 2-Node Cycle`, `lowercase month`, `Truncated file`).
    /// The bullet loader read the heading only far enough to know the section
    /// was an example and threw the rest away, so this is information the
    /// format already held that no consumer could reach.
    #[serde(default)]
    pub title: Option<String>,
    /// The CHAT input, verbatim.
    ///
    /// A whole CHAT file, always. The bullet format carried a fence info
    /// string that a generator interpolated into a method name, so every value
    /// but a `chat` fence named a parser method that does not exist; all 333
    /// examples were `chat`, making it an invariant rather than data. With the
    /// input as a field there is no fence and no info string to get wrong.
    pub chat: BlockText,
    /// Where in a transcript THIS example's fault occurs.
    ///
    /// Per-example since the `level` move completed Phase 2: E519 is the
    /// proof case, one rule firing at a header site in one example and a word
    /// site in another, which a per-FILE level could not write as one spec
    /// and which kept seven of the eleven duplicate pairs apart. Phase 5's
    /// merges stand on this field.
    pub level: SpecLevel,
    /// What this example ASSERTS, as one of the ruled claim vocabulary.
    ///
    /// REQUIRED: an example that claims nothing is unwritable, which is R2
    /// promoting the self-demonstration gate from a 36-entry baseline into a
    /// type. This replaces `expected_error_codes`, a free-text code list that
    /// mixed the normative claim with incidental observations; the
    /// observations live in the snapshot now, and the claim is one decision.
    pub claim: Claim,
    /// The fixture this example was taken from, when it names one.
    ///
    /// Its STEM is the transcript's name, which decides whether rules about a
    /// file's own name run. An example with no source is genuinely anonymous.
    #[serde(default)]
    pub source: Option<String>,
    /// Prose that sat inside this example's `## Example N` section.
    ///
    /// Not decoration. 190 lines across roughly 50 specs, and it holds the
    /// most valuable adjudication in the corpus: which code fires instead and
    /// why, what changed on what date, and in several cases a correction
    /// recorded in place. With the input moved into frontmatter that prose had
    /// nowhere to go, and deleting it would have been exactly the silent loss
    /// this whole redesign exists to stop.
    #[serde(default)]
    pub notes: Option<BlockText>,
}

/// One example's claim: `violates`, `legal`, or `subsumed_by <codes>`.
///
/// The vocabulary was ruled 2026-08-15; `subsumed_by` is parameterised (four
/// of the eight original cases are not E316) and accepts a LIST, because two
/// real examples are subsumed by two codes at once, each named in their spec's
/// own Notes. The written forms:
///
/// ```toml
/// claim = 'violates'
/// claim = 'legal'
/// claim = { subsumed_by = 'E316' }
/// claim = { subsumed_by = ['E246', 'E249'] }
/// ```
///
/// # What each asserts, status permitting
///
/// `Violates`: the spec's own code MUST appear (either stage; the runner is
/// total since R4). `Legal`: the own code MUST NOT appear, which is the
/// capability the old format could not express at all ("a spec cannot be used
/// to assert that a code is NOT emitted"). `SubsumedBy`: every listed code
/// appears and the own code does not, which is simultaneously an honest
/// statement of today and the parser-specificity worklist, verifiable against
/// the snapshot rather than merely asserted.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "ClaimRepr")]
// Serialized by the manual impl below, so the written forms are exactly the
// readable ones and a round-trip is the identity; a derive would invent a
// tagged representation nothing reads.
pub enum Claim {
    /// This input breaks THIS spec's rule.
    Violates,
    /// Valid CHAT that looks like a violation; the own code must not fire.
    Legal,
    /// Breaks this rule, but chatter reports the listed code(s) today.
    SubsumedBy(SubsumptionTargets),
}

impl Claim {
    /// Is this claim satisfied, given which codes fired?
    ///
    /// # The one owner of the claim's MEANING
    ///
    /// `violates`: the spec's own code fired. `legal`: it did not.
    /// `subsumed_by`: every target fired AND the own code did not. The R2
    /// commit claimed that sharing the claim TYPE meant the semantics could
    /// not be re-derived differently across the workspaces; that was wrong
    /// (a shared type pins the wire, not the verb), and the review found the
    /// same three-arm match written out in both workspaces' runners within
    /// hours of the type landing. Shape F: a well-typed noun whose verb every
    /// caller respelled. This method is the verb; renderers stay local.
    pub fn satisfied_by(
        &self,
        own_code: &SpecErrorCode,
        fired: impl Fn(&SpecErrorCode) -> bool,
    ) -> bool {
        match self {
            Self::Violates => fired(own_code),
            Self::Legal => !fired(own_code),
            Self::SubsumedBy(targets) => targets.as_slice().iter().all(&fired) && !fired(own_code),
        }
    }

    /// The codes this claim POSITIVELY asserts.
    ///
    /// `violates` asserts the spec's own code, `subsumed_by` its targets, and
    /// `legal` nothing positive: its whole content is the negative half,
    /// which [`Self::satisfied_by`] enforces. One owner; the two structs that
    /// carry a claim delegate here.
    #[must_use]
    pub fn positive_codes(&self, own_code: &SpecErrorCode) -> Vec<SpecErrorCode> {
        match self {
            Self::Violates => vec![own_code.clone()],
            Self::SubsumedBy(targets) => targets.as_slice().to_vec(),
            Self::Legal => Vec::new(),
        }
    }
}

impl serde::Serialize for Claim {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        match self {
            Self::Violates => serializer.serialize_str("violates"),
            Self::Legal => serializer.serialize_str("legal"),
            Self::SubsumedBy(targets) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("subsumed_by", targets.as_slice())?;
                map.end()
            }
        }
    }
}

/// The two spellings TOML admits for a claim: a bare keyword, or the
/// `subsumed_by` table. Private: it exists only to give serde a shape to read,
/// and [`Claim`] is what everything holds.
#[derive(Deserialize)]
#[serde(untagged)]
enum ClaimRepr {
    Keyword(String),
    Subsumed { subsumed_by: OneOrMany },
}

/// `subsumed_by = 'E316'` and `subsumed_by = ['E246', 'E249']` are both legal
/// spellings; the single form exists because it is the overwhelmingly common
/// one and a one-element array reads as ceremony.
#[derive(Deserialize)]
#[serde(untagged)]
enum OneOrMany {
    One(SpecErrorCode),
    Many(Vec<SpecErrorCode>),
}

impl TryFrom<ClaimRepr> for Claim {
    type Error = String;

    fn try_from(repr: ClaimRepr) -> Result<Self, Self::Error> {
        match repr {
            ClaimRepr::Keyword(word) => match word.as_str() {
                "violates" => Ok(Self::Violates),
                "legal" => Ok(Self::Legal),
                other => Err(format!(
                    "unrecognized claim {other:?}: expected 'violates', 'legal', \
                     or {{ subsumed_by = ... }}"
                )),
            },
            ClaimRepr::Subsumed { subsumed_by } => {
                let codes = match subsumed_by {
                    OneOrMany::One(code) => vec![code],
                    OneOrMany::Many(codes) => codes,
                };
                Ok(Self::SubsumedBy(SubsumptionTargets::try_from(codes)?))
            }
        }
    }
}

/// The non-empty code list a `subsumed_by` claim names.
///
/// # Every route in, enumerated
///
/// `TryFrom<Vec<SpecErrorCode>>` is the only constructor, so possession proves
/// the list has something in it: a proof whose invariant any caller could
/// assert from the parts would be a label. There is deliberately no serde
/// derive: the wire route runs through [`Claim`]'s own deserialization, which
/// calls this `TryFrom`, and a second door existed briefly with nothing
/// walking through it. (This type was `DeclaredCodes`, guarding the deleted
/// `expected_error_codes` field; the invariant survived the field.)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubsumptionTargets(Vec<SpecErrorCode>);

impl TryFrom<Vec<SpecErrorCode>> for SubsumptionTargets {
    type Error = String;

    fn try_from(codes: Vec<SpecErrorCode>) -> Result<Self, Self::Error> {
        if codes.is_empty() {
            return Err(
                "`subsumed_by` names no codes, so the claim asserts nothing while \
                 reading as fully specified. Name the code(s) that fire instead"
                    .to_owned(),
            );
        }
        Ok(Self(codes))
    }
}

impl SubsumptionTargets {
    /// The codes, which are never none.
    #[must_use]
    pub fn as_slice(&self) -> &[SpecErrorCode] {
        &self.0
    }
}

impl ExampleFrontmatter {
    /// The codes this example POSITIVELY asserts, derived from its claim.
    ///
    /// # One owner, because the predecessor rule was written twice within the hour
    ///
    /// Both cargo workspaces need this derivation (the fixture manifest and
    /// the re2c parity suite), so it lives on the schema. Under R2 it is a
    /// pure function of the claim: `violates` asserts the spec's own code,
    /// `subsumed_by` asserts its targets, and `legal` asserts nothing
    /// positive, since its whole content is the NEGATIVE half, which
    /// [`Claim`] carries and the runners enforce separately.
    #[must_use]
    pub fn effective_codes(&self, spec_code: &SpecErrorCode) -> Vec<SpecErrorCode> {
        self.claim.positive_codes(spec_code)
    }
}

/// Text written as a block string, carrying the convention its delimiter implies.
///
/// # Why a type and not three `deserialize_with` attributes
///
/// The newline before a block's closing delimiter is not part of the value,
/// exactly as the closing line of a fenced code block was not part of it. That
/// convention was first written as two accessors a caller had to remember to
/// call, then as an attribute on each of the three fields needing it, under a
/// doc claiming "a field that carries the convention in its type cannot be
/// read the wrong way". It did not carry it in its type: it carried it in an
/// attribute BESIDE the field, which the author of the fourth prose field has
/// to remember to write, and forgetting compiles and deserializes.
///
/// Apply the decision test to the attribute form: name a wrong value it
/// permits (a prose field declared `String`, silently keeping its trailing
/// newline) and ask what notices. Nothing did. Here the type is the answer,
/// `Option` composes for free, and there is no attribute to omit.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "String")]
pub struct BlockText(String);

impl From<String> for BlockText {
    fn from(text: String) -> Self {
        // Infallible on purpose: every string is a legal block, and the
        // convention only ever REMOVES a delimiter's own newline. A `TryFrom`
        // would promise a rejection that cannot happen.
        match text
            .strip_suffix("\r\n")
            .or_else(|| text.strip_suffix('\n'))
        {
            Some(stripped) => Self(stripped.to_owned()),
            None => Self(text),
        }
    }
}

impl BlockText {
    /// The text, with the block convention already applied.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for BlockText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<BlockText> for String {
    fn from(text: BlockText) -> Self {
        text.0
    }
}

#[cfg(test)]
mod tests {
    use super::{BlockText, Claim, DELIMITER, FrontmatterError, SpecFrontmatter, split};

    /// The ordinary shape: a block, then prose.
    #[test]
    fn splits_frontmatter_from_body() {
        let text = "+++\ncode = \"E256\"\n+++\n\n## Description\n\ntext\n";
        let source = split(text).expect("well-formed");
        assert_eq!(source.frontmatter, "code = \"E256\"\n");
        assert_eq!(source.body, "\n## Description\n\ntext\n");
    }

    /// A file with no frontmatter is refused, not treated as all body. Under
    /// the bullet format an absent `## Metadata` section produced a spec with
    /// defaulted fields.
    #[test]
    fn a_file_without_frontmatter_is_refused() {
        assert!(matches!(
            split("# E256: something\n"),
            Err(FrontmatterError::Missing)
        ));
    }

    /// The case that makes scanning for the closing delimiter safe. A spec's
    /// example is deliberately invalid CHAT, so a line reading `+++` is a
    /// thing an author could genuinely write; when they do, the split point is
    /// ambiguous and the file is refused rather than cut at the first match.
    ///
    /// Measured 2026-08-21 over all 236 specs: ZERO contain such a line, so
    /// this is prevention, and it is covered by this test rather than by data.
    #[test]
    fn a_delimiter_line_in_the_body_is_ambiguous_and_refused() {
        let text = format!("+++\ncode = \"E256\"\n+++\n\nprose\n{DELIMITER}\nmore\n");
        assert!(matches!(
            split(&text),
            Err(FrontmatterError::AmbiguousDelimiter { line: 3 })
        ));
    }

    /// An unclosed block is a load error, not a file whose body is empty.
    #[test]
    fn an_unclosed_block_is_refused() {
        assert!(matches!(
            split("+++\ncode = \"E256\"\n"),
            Err(FrontmatterError::Unterminated)
        ));
    }

    /// An unrecognised key is the whole reason the format changed. Five
    /// unmodelled labels accumulated under the bullet format precisely because
    /// nothing refused them.
    #[test]
    fn an_unknown_key_is_a_load_error() {
        let toml = "code = \"E256\"\nname = \"n\"\n\
                    last_updated = \"2026-04-04\"\n";
        let parsed = toml::from_str::<SpecFrontmatter>(toml);
        let message = parsed.expect_err("unknown key must be refused").to_string();
        assert!(
            message.contains("last_updated"),
            "the message must name the offending key, got: {message}"
        );
    }

    /// The three claims parse to their three variants, and a subsumed claim
    /// takes one code or several.
    #[test]
    fn the_claim_vocabulary_parses() {
        let toml = "code = \"E256\"\nname = \"n\"\n\
                    [[example]]\nlevel = \"word\"\nchat = \"a\"\nclaim = \"violates\"\n\
                    [[example]]\nlevel = \"word\"\nchat = \"b\"\nclaim = \"legal\"\n\
                    [[example]]\nlevel = \"word\"\nchat = \"c\"\nclaim = { subsumed_by = \"E316\" }\n\
                    [[example]]\nlevel = \"word\"\nchat = \"d\"\nclaim = { subsumed_by = [\"E246\", \"E249\"] }\n";
        let parsed: SpecFrontmatter = toml::from_str(toml).expect("well-formed");
        assert!(matches!(parsed.examples[0].claim, Claim::Violates));
        assert!(matches!(parsed.examples[1].claim, Claim::Legal));
        let Claim::SubsumedBy(one) = &parsed.examples[2].claim else {
            panic!("expected subsumed")
        };
        assert_eq!(one.as_slice().len(), 1);
        let Claim::SubsumedBy(two) = &parsed.examples[3].claim else {
            panic!("expected subsumed")
        };
        assert_eq!(two.as_slice().len(), 2);
    }

    /// An example with NO claim does not load: the state the old
    /// self-demonstration gate baselined is unwritable, which was R2's point.
    /// A wire-format property serde owns, so it survives as a test.
    #[test]
    fn an_example_without_a_claim_is_refused() {
        let toml = "code = \"E256\"\nname = \"n\"\n\
                    [[example]]\nlevel = \"word\"\nchat = \"a\"\n";
        let why = toml::from_str::<SpecFrontmatter>(toml)
            .expect_err("a claimless example must be refused")
            .to_string();
        assert!(why.contains("claim"), "{why}");
    }

    /// An empty `subsumed_by` list is REFUSED, because it asserts nothing
    /// while reading as fully specified.
    ///
    /// A WIRE-FORMAT property: no type of ours pins what serde accepts from a
    /// TOML array, so it survives the "could a type delete this test"
    /// question. What the type does pin is that no [`SubsumptionTargets`]
    /// value can be empty once one exists.
    #[test]
    fn an_empty_subsumed_by_list_is_refused() {
        let toml = "code = \"E256\"\nname = \"n\"\n\
                    [[example]]\nlevel = \"word\"\nchat = \"a\"\nclaim = { subsumed_by = [] }\n";
        let why = toml::from_str::<SpecFrontmatter>(toml)
            .expect_err("an empty subsumed_by must be refused")
            .to_string();
        assert!(why.contains("names no codes"), "{why}");
    }

    /// The block convention is carried by [`BlockText`], so a caller cannot
    /// read a prose field the wrong way by forgetting an attribute.
    #[test]
    fn a_block_value_drops_the_newline_before_its_delimiter() {
        assert_eq!(BlockText::from("a\nb\n".to_owned()).as_str(), "a\nb");
        assert_eq!(BlockText::from("a\nb".to_owned()).as_str(), "a\nb");
        assert_eq!(BlockText::from(String::new()).as_str(), "");
    }
}
