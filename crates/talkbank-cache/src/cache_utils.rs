//! Utility functions for cache operations.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use std::path::Path;

use talkbank_model::ResolvedPath;

/// The text of the `file_path` column for `path`: what path-prefix
/// maintenance and the missing-file purge read. Keys never use it.
pub(crate) fn column(path: &ResolvedPath) -> String {
    path.as_path().to_string_lossy().into_owned()
}

/// Which family of rows a key belongs to: a validation verdict or a
/// roundtrip verdict, each namespaced by the parser that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyNamespace {
    /// Validation verdicts produced with this parser.
    Validation(talkbank_model::ParserKind),
    /// Roundtrip verdicts produced with this parser.
    Roundtrip(talkbank_model::ParserKind),
    /// The parser-unqualified validation namespace older builds wrote, for
    /// tests proving such rows are never served.
    #[cfg(test)]
    LegacyValidation,
}

impl KeyNamespace {
    /// Every namespace a row can be written under today: what clearing a
    /// path must cover.
    pub(crate) fn every_written() -> [Self; 4] {
        use talkbank_model::ParserKind;
        // Exhaustive over ParserKind: a new parser fails to compile here, so
        // this list is revisited rather than left short.
        const fn listed(parser: ParserKind) {
            match parser {
                ParserKind::TreeSitter | ParserKind::Re2c => {}
            }
        }
        listed(ParserKind::TreeSitter);
        [
            Self::Validation(ParserKind::TreeSitter),
            Self::Validation(ParserKind::Re2c),
            Self::Roundtrip(ParserKind::TreeSitter),
            Self::Roundtrip(ParserKind::Re2c),
        ]
    }

    /// The namespace's label, part of what the key hashes.
    fn label(self) -> &'static str {
        use talkbank_model::ParserKind;
        match self {
            Self::Validation(ParserKind::TreeSitter) => "validation:tree-sitter",
            Self::Validation(ParserKind::Re2c) => "validation:re2c",
            Self::Roundtrip(parser) => parser.cache_label(),
            #[cfg(test)]
            Self::LegacyValidation => "validation",
        }
    }
}

/// The `path_hash` column: a specified, stable hash of a path and a
/// namespace. Made only by [`CacheKey::of`].
///
/// It was `DefaultHasher`, whose algorithm Rust does not promise to keep
/// from one release to the next, so any toolchain update could have turned
/// every persistent row into a silent miss. This is blake3 over a written-down
/// encoding: for each path component a kind tag (1 prefix, 2 root, 3 `.`,
/// 4 `..`, 5 name), then for a prefix or name its length as a little-endian
/// `u64` and its bytes (the raw bytes on Unix, the UTF-16 code units as
/// little-endian bytes on Windows); then a 0 byte, the namespace label's
/// length and bytes. The path comes from a [`ResolvedPath`] (its directory
/// resolved by the operating system, its name as stored), so every spelling
/// of one file shares a key; hashing its components keeps native path
/// equality beyond that (on Windows `\` and `/` share a key), and distinct
/// non-Unicode names stay distinct. The scheme is named by
/// [`crate::rules_version::KEY_SCHEME`], folded into every row's version, so
/// a change to this encoding retires old rows through the version prune
/// rather than leaving them as unreachable leftovers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CacheKey(String);

impl CacheKey {
    /// The key for the file at `path` in `namespace`.
    pub(crate) fn of(path: &ResolvedPath, namespace: KeyNamespace) -> Self {
        Self::encode(path.as_path(), namespace)
    }

    /// The specified encoding of a path and a namespace. Callers outside
    /// the key tests reach it only through [`Self::of`], so every key is a
    /// key of a resolved path.
    fn encode(path: &Path, namespace: KeyNamespace) -> Self {
        use std::path::Component;
        let mut hasher = blake3::Hasher::new();
        for component in path.components() {
            match component {
                Component::Prefix(prefix) => {
                    hasher.update(&[1]);
                    hash_bytes(&mut hasher, &os_bytes(prefix.as_os_str()));
                }
                Component::RootDir => {
                    hasher.update(&[2]);
                }
                Component::CurDir => {
                    hasher.update(&[3]);
                }
                Component::ParentDir => {
                    hasher.update(&[4]);
                }
                Component::Normal(name) => {
                    hasher.update(&[5]);
                    hash_bytes(&mut hasher, &os_bytes(name));
                }
            }
        }
        hasher.update(&[0]);
        hash_bytes(&mut hasher, namespace.label().as_bytes());
        Self(hasher.finalize().to_hex().to_string())
    }

    /// The value bound to the `path_hash` column.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// Length-prefixed bytes, so no two sequences of parts hash alike.
fn hash_bytes(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

/// A path component's bytes, in an encoding that is specified per platform
/// rather than left to the standard library's unspecified internal one.
#[cfg(unix)]
fn os_bytes(part: &std::ffi::OsStr) -> std::borrow::Cow<'_, [u8]> {
    use std::os::unix::ffi::OsStrExt;
    std::borrow::Cow::Borrowed(part.as_bytes())
}

#[cfg(windows)]
fn os_bytes(part: &std::ffi::OsStr) -> std::borrow::Cow<'_, [u8]> {
    use std::os::windows::ffi::OsStrExt;
    std::borrow::Cow::Owned(part.encode_wide().flat_map(u16::to_le_bytes).collect())
}

#[cfg(not(any(unix, windows)))]
fn os_bytes(part: &std::ffi::OsStr) -> std::borrow::Cow<'_, [u8]> {
    std::borrow::Cow::Owned(part.to_string_lossy().into_owned().into_bytes())
}

/// When a row was written, in whole seconds since the Unix epoch: the
/// integer the `cached_at` column holds. One clock (jiff's) for every stamp
/// and every cutoff, with no failure case and no unsigned-to-signed cast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CachedAt(i64);

impl CachedAt {
    /// Now.
    pub(crate) fn now() -> Self {
        Self(jiff::Timestamp::now().as_second())
    }

    /// The same instant `days` days earlier.
    pub(crate) fn days_before(self, days: i64) -> Self {
        Self(self.0.saturating_sub(days.saturating_mul(86_400)))
    }

    /// The value bound to the `cached_at` column.
    pub(crate) fn column(self) -> i64 {
        self.0
    }
}

/// The integer the `check_alignment` column holds for a coverage: the SQL
/// boundary, and the only place the typed coverage becomes a number.
pub(crate) fn alignment_column(alignment: talkbank_model::validation::AlignmentValidation) -> i32 {
    use talkbank_model::validation::AlignmentValidation;
    match alignment {
        AlignmentValidation::Structure => 0,
        AlignmentValidation::IncludeTierAlignment => 1,
    }
}

/// The integer a pass/fail verdict is stored as.
pub(crate) fn outcome_column(outcome: crate::CacheOutcome) -> i32 {
    match outcome {
        crate::CacheOutcome::Valid => 1,
        crate::CacheOutcome::Invalid => 0,
    }
}

/// The integer a roundtrip verdict is stored as (`roundtrip_passed`, and
/// mirrored in `is_valid` on a roundtrip row).
pub(crate) fn roundtrip_column(outcome: crate::RoundtripOutcome) -> i32 {
    match outcome {
        crate::RoundtripOutcome::Passed => 1,
        crate::RoundtripOutcome::Failed => 0,
    }
}

/// A stored roundtrip integer read back; anything but 0 or 1 is corrupt.
pub(crate) fn roundtrip_from_column(
    value: i64,
) -> Result<crate::RoundtripOutcome, crate::CacheError> {
    match value {
        0 => Ok(crate::RoundtripOutcome::Failed),
        1 => Ok(crate::RoundtripOutcome::Passed),
        value => Err(crate::CacheError::CorruptColumn {
            column: "roundtrip_passed",
            value: Some(value),
        }),
    }
}

/// A stored pass/fail integer read back as a verdict; anything but 0 or 1
/// is a corrupt row, never a guess.
pub(crate) fn outcome_from_column(value: i64) -> Result<crate::CacheOutcome, crate::CacheError> {
    match value {
        0 => Ok(crate::CacheOutcome::Invalid),
        1 => Ok(crate::CacheOutcome::Valid),
        value => Err(crate::CacheError::CorruptColumn {
            column: "is_valid",
            value: Some(value),
        }),
    }
}

/// A SQLite count as a `usize`, refused rather than truncated or clamped.
pub(crate) fn count<T: Copy + Into<i128> + TryInto<usize>>(
    value: T,
) -> Result<usize, crate::CacheError> {
    value
        .try_into()
        .map_err(|_| crate::CacheError::CountOutOfRange {
            value: value.into(),
        })
}

#[cfg(test)]
mod key_tests {
    use super::*;
    use talkbank_model::ParserKind;

    const VALIDATION: KeyNamespace = KeyNamespace::Validation(ParserKind::TreeSitter);

    /// The key of `path` as written: these tests pin the encoding, not the
    /// resolution, so they do not read the filesystem.
    fn key(path: &str, namespace: KeyNamespace) -> CacheKey {
        CacheKey::encode(Path::new(path), namespace)
    }

    /// The key is pinned: a change to the algorithm or encoding fails here,
    /// and must come with a new `KEY_SCHEME`. Unix here; Windows below.
    #[cfg(unix)]
    #[test]
    fn the_key_is_pinned() {
        assert_eq!(key("/corpus/a.cha", VALIDATION).as_str(), PINNED_KEY,);
    }

    /// Value recorded when the scheme was introduced.
    #[cfg(unix)]
    const PINNED_KEY: &str = "8a4e235e0f0592f5a4432d6deac9f7e0432572351c8850956e305f9d76a50624";

    /// The Windows encoding is pinned too (prefix `C:`, names as UTF-16
    /// little-endian bytes), so a change to it fails on the Windows CI job.
    /// The digest was computed from the specified byte sequence by hand,
    /// with the same arithmetic reproducing the Unix pin above.
    #[cfg(windows)]
    #[test]
    fn the_windows_key_is_pinned() {
        assert_eq!(
            key(r"C:\corpus\a.cha", VALIDATION).as_str(),
            "e69f45aa745d10e15b600d60aaf3b2691a6f8cc3d6f4f40c8963ded4162f16d4",
        );
    }

    /// One file is one key however it is spelled, because the key is of
    /// its resolved path; a trailing separator does not change the encoding.
    /// Another name, another namespace or another parser is another key.
    #[test]
    fn equal_paths_share_a_key_and_different_ones_do_not() {
        let dir = tempfile::tempdir().expect("temporary directory");
        std::fs::create_dir_all(dir.path().join("corpus/sub")).expect("directories");
        let of = |path: std::path::PathBuf, namespace| {
            CacheKey::of(
                &ResolvedPath::of_file(&path).expect("resolvable"),
                namespace,
            )
        };
        let base = of(dir.path().join("corpus/a.cha"), VALIDATION);
        assert_eq!(base, of(dir.path().join("corpus/sub/../a.cha"), VALIDATION));
        assert_eq!(
            key("/corpus/sub", VALIDATION),
            key("/corpus/sub/", VALIDATION)
        );
        assert_ne!(base, of(dir.path().join("corpus/b.cha"), VALIDATION));
        assert_ne!(
            base,
            of(
                dir.path().join("corpus/a.cha"),
                KeyNamespace::Roundtrip(ParserKind::TreeSitter)
            )
        );
        assert_ne!(
            base,
            of(
                dir.path().join("corpus/a.cha"),
                KeyNamespace::Validation(ParserKind::Re2c)
            )
        );
    }

    /// Distinct non-Unicode names stay distinct (not collapsed by a lossy
    /// conversion).
    #[cfg(unix)]
    #[test]
    fn non_unicode_names_stay_distinct() {
        use std::os::unix::ffi::OsStrExt;
        let name = |bytes: &[u8]| Path::new(std::ffi::OsStr::from_bytes(bytes)).to_owned();
        let one = CacheKey::encode(&name(b"/a\xff.cha"), VALIDATION);
        let two = CacheKey::encode(&name(b"/a\xfe.cha"), VALIDATION);
        assert_ne!(one, two);
    }
}
