//! The one owner of WHERE the cache lives.
//!
//! Four names had grown around one product (`chatter` the CLI,
//! `org.talkbank.chatter` the bundle id, `chatter-desktop` the app, and
//! `talkbank-chat` the cache directory), and the directory name was written as
//! a bare literal at its single use site while the database file name was
//! written as a bare literal at two. Nobody could answer "where is my cache"
//! from any one of them, and a rename would have silently split one cache into
//! two.
//!
//! Everything that needs the location derives it from here. The names
//! themselves are deliberately NOT being changed to match the product: a user's
//! existing cache is a real 200+ MB artifact, and relocating it silently would
//! turn one warm cache into one cold cache plus one orphan nobody ever deletes.
//! The fix is single-sourcing plus documentation, not a move.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use std::path::{Path, PathBuf};

use super::error::CacheError;

/// Directory holding the cache, relative to the platform cache root.
///
/// Historical, and kept deliberately: see the module doc comment. It predates
/// the `chatter` binary name and is where every existing installation's cache
/// already lives.
const CACHE_DIR_NAME: &str = "talkbank-chat";

/// The SQLite database inside the cache directory.
const CACHE_DB_FILE_NAME: &str = "talkbank-cache.db";

/// Environment variable that relocates the cache root.
///
/// When set (to an absolute or relative directory path), the cache database
/// lives directly in that directory instead of under the platform cache root.
/// This is the supported way to redirect cache state, and the only reliable
/// isolation mechanism on Windows: the platform default there resolves through
/// the Known Folder API, which ignores `HOME`-style environment variables
/// entirely.
pub const CACHE_DIR_ENV: &str = "TALKBANK_CHAT_CACHE_DIR";

/// The default cache directory.
///
/// Resolution order: [`CACHE_DIR_ENV`] if set and non-empty (used verbatim, with
/// no name appended); otherwise the platform cache root (`~/Library/Caches` on
/// macOS, `XDG_CACHE_HOME` or `~/.cache` on Linux, `%LocalAppData%` on Windows)
/// plus `CACHE_DIR_NAME`.
pub fn default_cache_dir() -> Result<PathBuf, CacheError> {
    if let Some(dir) = std::env::var_os(CACHE_DIR_ENV)
        && !dir.is_empty()
    {
        return Ok(PathBuf::from(dir));
    }
    dirs::cache_dir()
        .map(|root| root.join(CACHE_DIR_NAME))
        .ok_or(CacheError::CacheDirMissing)
}

/// The database file inside a given cache directory.
///
/// Every caller that needs the file (opening it, or reporting its size to a
/// user) goes through this, so the file name exists once.
pub fn cache_db_path(cache_dir: &Path) -> PathBuf {
    cache_dir.join(CACHE_DB_FILE_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The database always sits inside the directory it was asked for, which is
    /// what lets a caller reason about the cache as one relocatable unit.
    #[test]
    fn the_database_lives_inside_the_cache_directory() {
        let dir = Path::new("/some/cache/root");
        let db = cache_db_path(dir);
        assert_eq!(db.parent(), Some(dir));
    }

    /// Two callers asking for the database of the same directory get the same
    /// path. Trivially true now, and the reason the name is not written at each
    /// call site: it was, and one of them could have drifted.
    #[test]
    fn the_database_path_has_one_spelling() {
        let dir = Path::new("/some/cache/root");
        assert_eq!(cache_db_path(dir), cache_db_path(dir));
    }
}
