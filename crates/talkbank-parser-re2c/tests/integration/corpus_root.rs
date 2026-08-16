//! Where the wild corpus is, for every test in this binary that needs it.
//!
//! # Why this is shared rather than repeated
//!
//! Five files in this directory each had their own `corpus_base()`, in two
//! return types, all defaulting to `$HOME/talkbank/data`: the retired split
//! layout. On the maintainer's machine that path still resolves through a
//! legacy symlink to the same directory `~/0tb/data` points at, which is
//! precisely why five stale copies survived. It worked in the one place
//! anybody ran it, and silently found nothing anywhere else.
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
    At(PathBuf),
    Missing { looked_at: PathBuf, from_env: bool },
}

impl CorpusRoot {
    /// `$TALKBANK_DATA` if set, else the only supported workspace layout.
    ///
    /// The default is `~/0tb/data`, which is what `tb`'s workspace discovery
    /// resolves; the split layouts it replaced are retired.
    pub fn resolve() -> Self {
        let from_env = std::env::var("TALKBANK_DATA").ok();
        let path = match &from_env {
            Some(value) => PathBuf::from(value),
            None => PathBuf::from(std::env::var("HOME").expect("HOME is set")).join("0tb/data"),
        };
        if path.is_dir() {
            Self::At(path)
        } else {
            Self::Missing {
                looked_at: path,
                from_env: from_env.is_some(),
            }
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
            Self::Missing {
                looked_at,
                from_env,
            } => {
                let source = if from_env {
                    "TALKBANK_DATA points at"
                } else {
                    "the default corpus layout is"
                };
                panic!(
                    "no corpus to compare against: {source} {}, which is not a directory. \
                     This test exists to run over real data; passing without it would report \
                     agreement that was never measured.",
                    looked_at.display()
                )
            }
        }
    }
}
