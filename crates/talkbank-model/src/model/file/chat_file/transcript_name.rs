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
/// a path: it has no directory part and no extension, and it is not empty.
/// Both constructors check that, so no stem names a directory or nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStem<'a>(&'a str);

/// Why text is not a file stem.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FileStemError {
    /// A stem names something; the empty string names nothing.
    #[error("a file stem cannot be empty")]
    Empty,
    /// A stem is one path component; this text has a path separator.
    #[error("a file stem is one path component, but {stem:?} has a path separator")]
    PathSeparator {
        /// The text offered.
        stem: String,
    },
}

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
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(|stem| Self::from_stem(stem).ok())
    }

    /// Text as a stem, for a transcript whose name is known without a path
    /// on disk: refused when it is empty or has a path separator.
    ///
    /// NOT `from_str`: that name reads as `std::str::FromStr::from_str`, which
    /// this cannot be. The trait returns `Self` with no lifetime tied to its
    /// input, and this type BORROWS its stem, so implementing it is impossible
    /// rather than merely unimplemented. A name a reader can mistake for a
    /// trait method they can call generically is worse than a longer one.
    pub fn from_stem(stem: &'a str) -> Result<Self, FileStemError> {
        match stem {
            "" => Err(FileStemError::Empty),
            _ if stem.chars().any(std::path::is_separator) => Err(FileStemError::PathSeparator {
                stem: stem.to_owned(),
            }),
            _ => Ok(Self(stem)),
        }
    }

    /// Borrow the stem.
    pub fn as_str(&self) -> &'a str {
        self.0
    }
}

/// A [`FileStem`] that owns its text, for a name that must outlive its
/// source. Built only from a checked stem, so it holds the same guarantee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedFileStem(String);

impl OwnedFileStem {
    /// The stem of `path`; see [`FileStem::from_path`].
    pub fn from_path(path: &Path) -> Option<Self> {
        FileStem::from_path(path).map(Self::from)
    }

    /// Text as a stem; see [`FileStem::from_stem`].
    pub fn new(stem: &str) -> Result<Self, FileStemError> {
        FileStem::from_stem(stem).map(Self::from)
    }

    /// The borrowed stem.
    pub fn as_stem(&self) -> FileStem<'_> {
        FileStem(&self.0)
    }
}

impl From<FileStem<'_>> for OwnedFileStem {
    /// Own a checked stem: no check is skipped, since the stem was checked.
    fn from(stem: FileStem<'_>) -> Self {
        Self(stem.0.to_owned())
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

    /// The same name, owning its stem.
    pub fn to_owned_name(&self) -> OwnedTranscriptName {
        match self {
            Self::Named(stem) => OwnedTranscriptName::Named(OwnedFileStem::from(*stem)),
            Self::Anonymous => OwnedTranscriptName::Anonymous,
        }
    }

    /// The stem, when there is one.
    pub fn stem(&self) -> Option<FileStem<'a>> {
        match self {
            Self::Named(stem) => Some(*stem),
            Self::Anonymous => None,
        }
    }
}

/// A [`TranscriptName`] that owns its stem, for a name that must outlive
/// its caller (validation on another thread). The same choice, as variants,
/// rather than an `Option<String>` whose `None` means "skip the name rules".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnedTranscriptName {
    /// Named by this stem; rules about the name run.
    Named(OwnedFileStem),
    /// No name; rules about it do not run (see
    /// [`TranscriptName::Anonymous`]).
    Anonymous,
}

impl OwnedTranscriptName {
    /// The borrowed name validation takes.
    pub fn borrow(&self) -> TranscriptName<'_> {
        match self {
            Self::Named(stem) => TranscriptName::Named(stem.as_stem()),
            Self::Anonymous => TranscriptName::Anonymous,
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

    /// A stem is one non-empty path component: text that is empty or has a
    /// path separator is refused, so no stem names nothing or a directory.
    #[test]
    fn a_stem_is_one_non_empty_component() {
        assert_eq!(FileStem::from_stem(""), Err(FileStemError::Empty));
        assert_eq!(
            FileStem::from_stem("corpus/foo"),
            Err(FileStemError::PathSeparator {
                stem: "corpus/foo".to_owned()
            })
        );
        assert_eq!(
            FileStem::from_stem("foo.bar").map(|stem| stem.as_str()),
            Ok("foo.bar")
        );
        assert_eq!(
            OwnedFileStem::new("foo").map(|stem| stem.as_stem().as_str().to_owned()),
            Ok("foo".to_owned())
        );
    }
}
