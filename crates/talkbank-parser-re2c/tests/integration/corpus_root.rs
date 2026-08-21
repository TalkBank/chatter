//! Where the wild corpus is, for every test in this binary that needs it.
//!
//! # Why this is shared rather than repeated
//!
//! Five files in this directory each had their own `corpus_base()`, in two
//! return types, all defaulting to a hard-coded path under `$HOME`. On the one
//! machine anybody ran them, a legacy symlink made that path resolve to the
//! real corpus, which is precisely why five stale copies survived: it worked
//! there, and silently found nothing anywhere else.
//!
//! The copies also disagreed about failure. Some printed "Skipping: not found"
//! and returned, which cargo reports as a pass; one used
//! `env::var("HOME").unwrap()`; one used `unwrap_or_default()`, so a missing
//! `HOME` yielded the relative path `/talkbank/data`.
//!
//! One owner, one default, one failure mode.

use std::path::PathBuf;

/// Where the wild corpus is, or why it is not here.
///
/// Two variants rather than an `Option<PathBuf>`, because the caller has to
/// tell an operator WHICH path it looked at: "no corpus" and "you pointed me
/// somewhere that does not exist" need different fixes, and an `Option`
/// carries neither.
pub enum CorpusRoot {
    /// The corpus is here.
    At(PathBuf),
    /// `$TALKBANK_DATA` names a path that is not a directory.
    Missing { looked_at: PathBuf },
    /// `$TALKBANK_DATA` is not set, so no corpus location was ever stated.
    ///
    /// A third variant rather than `Missing { from_env: false }`: "you pointed
    /// me somewhere wrong" and "you pointed me nowhere" need different words
    /// from the operator, and the boolean made the caller reconstruct which
    /// one it was holding.
    Unset,
}

impl CorpusRoot {
    /// `$TALKBANK_DATA`, and nothing else.
    ///
    /// # There is deliberately no default
    ///
    /// There was one, a hard-coded directory under `$HOME`, and it was a
    /// default that could only ever be right on ONE machine: this is a public
    /// repository, and a contributor cloning it has no such directory. So the
    /// default silently sent everybody else to a path that does not exist,
    /// while reading, in the source, as though the location were a known fact.
    ///
    /// Requiring the variable makes the requirement visible to the person who
    /// has to satisfy it, and the [`Self::Unset`] arm says so by name rather
    /// than reporting a path nobody chose.
    pub fn resolve() -> Self {
        let Ok(value) = std::env::var("TALKBANK_DATA") else {
            return Self::Unset;
        };
        let path = PathBuf::from(value);
        if path.is_dir() {
            Self::At(path)
        } else {
            Self::Missing { looked_at: path }
        }
    }

    /// The corpus, or a failure naming the path and where it came from.
    ///
    /// Never a silent pass. Every caller is a test that exists to run over real
    /// data, so "there is no corpus" answers a question nobody asked and hides
    /// the one they did.
    pub fn require(self) -> PathBuf {
        match self {
            Self::At(path) => path,
            Self::Missing { looked_at } => panic!(
                "no corpus to compare against: TALKBANK_DATA points at {}, which is not a \
                 directory. This test exists to run over real data; passing without it would \
                 report agreement that was never measured.",
                looked_at.display()
            ),
            Self::Unset => panic!(
                "no corpus to compare against: TALKBANK_DATA is not set. Set it to a directory \
                 containing the `*-data` corpus repositories. This test exists to run over real \
                 data; passing without it would report agreement that was never measured."
            ),
        }
    }
}
