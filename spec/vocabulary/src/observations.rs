//! The observation snapshot's schema: what the spec examples actually emit.
//!
//! The FORMAT lives here, in the dependency-light crate both cargo workspaces
//! share, for the same reason [`crate::frontmatter`] does: the snapshot is
//! BUILT by `spec-runtime-tools` (which has the live parser) and READ by
//! `spec/tools` (which decides tree-sitter corpus membership from it) and by
//! Phase 2's R2 tooling. A schema with one owner is the whole lesson of Phase
//! 1b; the builder and the readers each import this instead of keeping a
//! structural copy that drifts.
//!
//! What the snapshot IS, why every example is covered whatever its status, and
//! why a diff in it is adjudicated rather than regenerated blindly, is documented
//! on the BUILDER (`spec-runtime-tools::observations`), which is where a
//! reader tracing a diff will land.

use serde::{Deserialize, Serialize};

/// The snapshot file's name inside `spec/observations/`.
pub const SNAPSHOT_FILE: &str = "example-diagnostics.json";

/// The whole snapshot: one entry per example, in spec-file then file order.
#[derive(Debug, Serialize, Deserialize)]
pub struct ObservationSnapshot {
    /// A fixed sentence naming the command, so a reader of the raw file knows
    /// where it came from. Constant, never a timestamp: the currency gate
    /// byte-compares this file.
    pub generated_by: String,
    /// One entry per example, covering every spec whatever its status.
    pub examples: Vec<ExampleObservation>,
}

/// One example's IDENTITY: the spec file it lives in, and its 1-based position.
///
/// # Why identity is a type
///
/// Three generators and the snapshot key each derived "spec stem plus 1-based
/// index" independently, the 1-basedness was a comment-carried convention at
/// every site, and two of the derivations had already diverged on case. Under
/// R4 the derived names ARE the example's identity (they replaced
/// iteration-order names precisely because those reassigned files across
/// specs), so the identity and every name derived from it get one owner.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExampleId<'a> {
    spec_file: &'a str,
    /// 1-based, matching the snapshot's `example` field.
    position: usize,
}

impl<'a> ExampleId<'a> {
    /// Identity from a spec file's basename and a 0-based iteration index.
    ///
    /// The ONE place the 1-based convention is applied: callers iterate with
    /// `enumerate()` and hand the raw index over, so no call site adds one.
    #[must_use]
    pub fn from_enumerate(spec_file: &'a str, zero_based: usize) -> Self {
        Self {
            spec_file,
            position: zero_based + 1,
        }
    }

    /// The spec file's basename, as the snapshot records it.
    #[must_use]
    pub fn spec_file(&self) -> &'a str {
        self.spec_file
    }

    /// The 1-based position, as the snapshot records it.
    #[must_use]
    pub fn position(&self) -> usize {
        self.position
    }

    /// The spec file's stem: `E316_auto.md` -> `E316_auto`.
    #[must_use]
    pub fn spec_stem(&self) -> &str {
        self.spec_file.strip_suffix(".md").unwrap_or(self.spec_file)
    }

    /// The validation fixture this example generates: `<stem>_<position>.cha`.
    #[must_use]
    pub fn fixture_name(&self) -> String {
        format!("{}_{}.cha", self.spec_stem(), self.position)
    }

    /// The tree-sitter corpus file, when the example qualifies:
    /// `errors/<lowercased stem>_<position>.txt`.
    ///
    /// Lowercased because the corpus tree's files have always been; that rule
    /// was drifting apart from the fixture rule when both were inline.
    #[must_use]
    pub fn corpus_file_name(&self) -> String {
        format!(
            "errors/{}_{}.txt",
            self.spec_stem().to_lowercase(),
            self.position
        )
    }

    /// The corpus test's display name.
    #[must_use]
    pub fn corpus_test_name(&self, spec_name: &str) -> String {
        format!(
            "{}_{} - {}",
            self.spec_stem().to_lowercase(),
            self.position,
            spec_name
        )
    }
}

/// Whether serializing the parsed example reproduces its text.
///
/// A SUM TYPE rather than a bool, so a reader of the snapshot sees a word
/// that says what was measured. `Diverged` is information, not failure:
/// error-recovery examples legitimately lose recovered text, and the snapshot
/// records which ones do TODAY, so that a parser change flipping any of them
/// is a diff to adjudicate rather than a silent loss.
///
/// # Why this field exists
///
/// `chatter normalize` once wrote `the bebe here .` for
/// `the bebe@k@s:spa here .` and exited 0, because a new grammar node had no
/// lowering arm. Nothing in the repository could see it: this snapshot
/// recorded codes only. With this field every spec example is also a
/// roundtrip test, in `just test`, and a change that flips any example from
/// `ByteExact` to `Diverged` is a diff to adjudicate before it lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Roundtrip {
    /// `write_chat(parse(text)) == text`, byte for byte.
    ByteExact,
    /// The serialized form differs from the input.
    Diverged,
}

/// What one example actually triggered, by stage, and whether it survived a
/// parse-and-serialize round trip.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExampleObservation {
    /// The spec file's basename, e.g. `E256.md`.
    pub spec: String,
    /// 1-based position within the spec's `[[example]]` list.
    pub example: usize,
    /// Codes the PARSER emitted, sorted and deduplicated.
    pub parse: Vec<String>,
    /// Codes VALIDATION emitted, sorted and deduplicated.
    pub validation: Vec<String>,
    /// Whether the parsed model serializes back to the input text.
    pub roundtrip: Roundtrip,
}

impl ObservationSnapshot {
    /// Index the entries by [`ExampleId`].
    ///
    /// Built by the READERS (corpus membership, R2 tooling), so the question
    /// "what did this example emit" has one lookup instead of a linear scan
    /// per ask. A missing key means the snapshot predates the spec file, which
    /// is a staleness a caller must refuse, never default.
    #[must_use]
    pub fn by_example(&self) -> std::collections::BTreeMap<ExampleId<'_>, &ExampleObservation> {
        self.examples
            .iter()
            .map(|entry| {
                (
                    ExampleId {
                        spec_file: entry.spec.as_str(),
                        position: entry.example,
                    },
                    entry,
                )
            })
            .collect()
    }
}
