//! What the transcript being validated is CALLED, for the rules that are about
//! its own name.
//!
//! # Why this is a type and not `Option<&str>`
//!
//! Some CHAT rules compare the transcript against its own file name: E531
//! requires the `@Media` header's filename to match the transcript's stem, so
//! `foo.cha` carrying `@Media: bar, audio` is invalid. A validator with no name
//! cannot run them.
//!
//! That was expressed as `filename: Option<&str>`, which made "I have no name"
//! and "silently skip a class of rules" the same value, and made `None` the
//! shorter thing to type. Twenty-eight call sites passed `None`, and the shape
//! produced the same defect three times:
//!
//! - the CLI's validation worker passed `None`, which disabled E531 for the
//!   whole `chatter validate` command until it was found and fixed locally,
//!   with a regression test whose docstring records the incident;
//! - `talkbank-transform`'s pipeline passed `None`, and carried a `NOTE` plus a
//!   `FOLLOW-UP` in production source saying E531 does not run for `to-json`
//!   or any other pipeline consumer. That comment stood in place of a fix for
//!   as long as it existed;
//! - the spec-example runner passed `None`, so a whole class of rule could not
//!   be verified there and E531's own spec was reported as FAILING rather than
//!   as untestable.
//!
//! Each was fixed where it was found, and none of the fixes was visible to the
//! next site. [`TranscriptName`] makes the choice a variant, so a caller must
//! say which case it is in and the compiler asks the question of every new one.
//!
//! [`TranscriptName::Anonymous`] is not a lesser answer. A fragment in a test,
//! a string from a network request, and a buffer being edited in the LSP
//! genuinely have no file name, and saying so is correct. What the old shape
//! could not distinguish is that honest case from an oversight.

use std::path::Path;

/// A file name with its extension removed: the `foo` of `foo.cha`.
///
/// This is what `@Media` must match, and it is a different kind of thing from
/// a path: it has no directory part and no extension. Constructing one from a
/// [`Path`] is the only conversion, and it is fallible, because a path can
/// have no file name at all and a file name need not be UTF-8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStem<'a>(&'a str);

impl<'a> FileStem<'a> {
    /// The stem of `path`, or `None` when it has no file name or the name is
    /// not UTF-8.
    ///
    /// Deliberately fallible rather than defaulting: the caller decides what
    /// an unusable name means. The CLI's worker used to write
    /// `path.file_stem().and_then(|s| s.to_str())` straight into an
    /// `Option<&str>` parameter, so a non-UTF-8 name silently reverted to the
    /// no-name behaviour inside the site that had just been fixed to avoid it.
    pub fn from_path(path: &'a Path) -> Option<Self> {
        path.file_stem().and_then(|stem| stem.to_str()).map(Self)
    }

    /// Treat text as a stem directly, for a transcript whose name is known
    /// without a path on disk.
    pub fn from_str(stem: &'a str) -> Self {
        Self(stem)
    }

    /// Borrow the stem.
    pub fn as_str(&self) -> &'a str {
        self.0
    }
}

/// What the transcript being validated is called.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptName<'a> {
    /// The transcript has a name, and rules about it run.
    Named(FileStem<'a>),
    /// The transcript has no name, and rules about it do not run.
    ///
    /// Choose this deliberately. It is correct for a fragment, a test string,
    /// or an unsaved editor buffer; it is wrong wherever a path was available
    /// and got dropped on the way in.
    Anonymous,
}

impl<'a> TranscriptName<'a> {
    /// Name the transcript after a file on disk, falling back to
    /// [`TranscriptName::Anonymous`] when the path yields no usable stem.
    ///
    /// The fallback is written out here, in one place, rather than left to
    /// each caller's `and_then` chain, so that "this path had no usable name"
    /// reads as a decision instead of as an accident.
    pub fn for_path(path: &'a Path) -> Self {
        FileStem::from_path(path).map_or(Self::Anonymous, Self::Named)
    }

    /// The stem, when there is one.
    pub fn stem(&self) -> Option<FileStem<'a>> {
        match self {
            Self::Named(stem) => Some(*stem),
            Self::Anonymous => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `for_path` reads the stem, not the whole file name.
    ///
    /// A behaviour test, not an invariant one: "the extension is dropped" is a
    /// choice about what `@Media` compares against, and the type cannot say it.
    #[test]
    fn a_path_is_named_by_its_stem() {
        let name = TranscriptName::for_path(Path::new("/corpus/eng/foo.cha"));
        assert_eq!(name.stem().map(|s| s.as_str()), Some("foo"));
    }

    /// A path with no file name is Anonymous rather than a fabricated empty
    /// stem, which `@Media` would then compare against and reject everything.
    #[test]
    fn a_path_with_no_file_name_is_anonymous() {
        assert_eq!(
            TranscriptName::for_path(Path::new("/")),
            TranscriptName::Anonymous
        );
    }
}
