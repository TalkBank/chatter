//! The input files of a command that reads transcripts.
//!
//! Every such command expands its path arguments the same way, through
//! `talkbank_transform::paths`, and refuses an incomplete input: an argument
//! or directory entry that cannot be read is reported, and the command stops
//! before processing anything, rather than presenting part of a corpus as
//! the whole of it. The refusal is a value ([`InputRefusal`]); each command
//! reports it and exits at its own boundary.

use std::path::{Path, PathBuf};

use talkbank_transform::paths::{
    FoundTranscript, StoredTranscript, WalkFailure, expand_transcript_arguments, walk_transcripts,
};

/// Why a command's input cannot be processed.
#[derive(Debug)]
pub enum InputRefusal {
    /// These paths could not be read or named.
    Unreadable(Vec<WalkFailure>),
    /// The arguments named no transcript at all.
    NoTranscripts,
    /// A directory input held no transcript at all (an empty tree, or an
    /// unmounted mount point).
    NoTranscriptsIn(PathBuf),
}

impl std::fmt::Display for InputRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable(failures) => {
                for failure in failures {
                    writeln!(f, "{failure}")?;
                }
                write!(
                    f,
                    "{} input path(s) could not be read; nothing was processed",
                    failures.len()
                )
            }
            Self::NoTranscripts => f.write_str("no .cha files found in the provided paths"),
            Self::NoTranscriptsIn(root) => {
                write!(f, "no .cha files found in {}", root.display())
            }
        }
    }
}

/// At least one value: a list whose emptiness was decided where it was
/// built, so no consumer checks it again.
#[derive(Debug, Clone)]
pub struct NonEmpty<T> {
    first: T,
    rest: Vec<T>,
}

impl<T> NonEmpty<T> {
    /// The values, or `None` when there are none.
    pub fn new(values: impl IntoIterator<Item = T>) -> Option<Self> {
        let mut values = values.into_iter();
        let first = values.next()?;
        Some(Self {
            first,
            rest: values.collect(),
        })
    }

    /// The first value.
    pub fn first(&self) -> &T {
        &self.first
    }

    /// How many values there are: at least one.
    pub fn count(&self) -> std::num::NonZeroUsize {
        std::num::NonZeroUsize::MIN.saturating_add(self.rest.len())
    }

    /// The values, in order.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        std::iter::once(&self.first).chain(&self.rest)
    }
}

impl<T> IntoIterator for NonEmpty<T> {
    type Item = T;
    type IntoIter = std::iter::Chain<std::iter::Once<T>, std::vec::IntoIter<T>>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(self.first).chain(self.rest)
    }
}

/// At least one transcript, each under its stored name: what a command that
/// reads transcripts may process. Built by [`readable_transcripts`].
pub type Transcripts = NonEmpty<StoredTranscript>;

impl Transcripts {
    /// The transcripts' paths, for the `debug` tools, which read files by
    /// path.
    pub fn into_paths(self) -> Vec<PathBuf> {
        self.into_iter().map(StoredTranscript::into_path).collect()
    }
}

/// The transcripts named by `paths`, under their stored names: each file
/// argument, and the transcripts under each directory. Refused when any path
/// could not be read or named, or when there is no transcript at all.
pub fn readable_transcripts(paths: &[PathBuf]) -> Result<Transcripts, InputRefusal> {
    let (files, failures) = expand_transcript_arguments(paths).into_parts();
    match failures.is_empty() {
        true => {}
        false => return Err(InputRefusal::Unreadable(failures)),
    }
    NonEmpty::new(files).ok_or(InputRefusal::NoTranscripts)
}

/// The transcripts under `root`, each with its path relative to `root`: at
/// least one. Refused when any entry could not be read, and when there is
/// none (an empty tree, or a mount point with nothing mounted), the failure
/// `validate` reports for an input with nothing to validate.
pub fn readable_transcripts_under(root: &Path) -> Result<NonEmpty<FoundTranscript>, InputRefusal> {
    let (found, failures) = walk_transcripts(root).into_parts();
    match failures.is_empty() {
        true => NonEmpty::new(found).ok_or_else(|| InputRefusal::NoTranscriptsIn(root.to_owned())),
        false => Err(InputRefusal::Unreadable(failures)),
    }
}
