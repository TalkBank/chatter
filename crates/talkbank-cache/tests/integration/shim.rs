//! Test shim over the typed cache API, for tests written against files on
//! disk: it reads the file and hashes what it read, as the validation runner
//! does, and keys it by the file's `ResolvedPath`. Typed end to end: the
//! coverage and the verdict are the cache's own types, never booleans the
//! shim would have to translate.
//!
//! ONE copy for every crate's tests: talkbank-transform and
//! talkbank-parser-tests include this file with `#[path]`, so each test
//! target uses part of it.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use talkbank_cache::{
    CacheError, CacheLookup, CacheOutcome, CachePool, ContentHash, ResolvedPath, RoundtripOutcome,
    ValidationCache, VerdictReader,
};
use talkbank_model::validation::AlignmentValidation;

/// The hash of the file's current bytes.
pub fn content(path: &Path) -> ContentHash {
    ContentHash::of(&std::fs::read(path).expect("read the cached test file"))
}

/// The cache's form of the file's path.
pub fn cached(path: &Path) -> ResolvedPath {
    ResolvedPath::of_file(path).expect("resolvable path")
}

/// A lookup's verdict, `None` for a miss; a failed lookup fails the test.
pub fn verdict<V>(lookup: Result<CacheLookup<V>, CacheError>) -> Option<V> {
    match lookup.expect("cache lookup") {
        CacheLookup::Hit(outcome) => Some(outcome),
        CacheLookup::Miss => None,
    }
}

pub fn set_validation(
    cache: &CachePool,
    path: &Path,
    alignment: AlignmentValidation,
    outcome: CacheOutcome,
) -> Result<(), CacheError> {
    cache.set(&cached(path), &content(path), alignment, outcome)
}

pub fn get_validation(
    cache: &CachePool,
    path: &Path,
    alignment: AlignmentValidation,
) -> Option<CacheOutcome> {
    verdict(cache.get(&cached(path), &content(path), alignment))
}

pub fn set_roundtrip(
    cache: &CachePool,
    path: &Path,
    alignment: AlignmentValidation,
    outcome: RoundtripOutcome,
) -> Result<(), CacheError> {
    cache.set_roundtrip(&cached(path), &content(path), alignment, outcome)
}

pub fn get_roundtrip(
    cache: &CachePool,
    path: &Path,
    alignment: AlignmentValidation,
) -> Option<RoundtripOutcome> {
    verdict(cache.get_roundtrip(&cached(path), &content(path), alignment))
}

/// Test data: valid for an even index, invalid for an odd one, so a row
/// read back for the wrong file shows.
pub fn alternating(index: usize) -> CacheOutcome {
    match index % 2 {
        0 => CacheOutcome::Valid,
        _ => CacheOutcome::Invalid,
    }
}

/// A test cache's refusal to store.
pub fn refused(reason: &str) -> CacheError {
    CacheError::Message(reason.to_owned())
}

/// One write a [`RefusingCache`] was asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attempt {
    /// A validation verdict for this file, coverage and outcome.
    Validation(PathBuf, AlignmentValidation, CacheOutcome),
    /// A roundtrip verdict for this file, coverage and outcome.
    Roundtrip(PathBuf, AlignmentValidation, RoundtripOutcome),
}

/// A cache that has no verdict for anything and refuses every write,
/// recording what it was asked to store. Opened for `identity`, as a run's
/// cache is opened for its configuration's.
pub struct RefusingCache {
    identity: talkbank_cache::CacheIdentity,
    attempts: Mutex<Vec<Attempt>>,
}

impl RefusingCache {
    /// A refusing cache for `identity`.
    pub fn new(identity: talkbank_cache::CacheIdentity) -> Self {
        Self {
            identity,
            attempts: Mutex::new(Vec::new()),
        }
    }

    /// Every write attempted so far, in order.
    pub fn attempts(&self) -> Vec<Attempt> {
        self.attempts.lock().expect("attempt log").clone()
    }
}

impl VerdictReader for RefusingCache {
    fn identity(&self) -> &talkbank_cache::CacheIdentity {
        &self.identity
    }

    fn get(
        &self,
        _: &ResolvedPath,
        _: &ContentHash,
        _: AlignmentValidation,
    ) -> Result<CacheLookup<CacheOutcome>, CacheError> {
        Ok(CacheLookup::Miss)
    }

    fn get_roundtrip(
        &self,
        _: &ResolvedPath,
        _: &ContentHash,
        _: AlignmentValidation,
    ) -> Result<CacheLookup<RoundtripOutcome>, CacheError> {
        Ok(CacheLookup::Miss)
    }
}

impl ValidationCache for RefusingCache {
    fn set(
        &self,
        path: &ResolvedPath,
        _: &ContentHash,
        alignment: AlignmentValidation,
        outcome: CacheOutcome,
    ) -> Result<(), CacheError> {
        self.attempts
            .lock()
            .expect("attempt log")
            .push(Attempt::Validation(
                path.as_path().to_owned(),
                alignment,
                outcome,
            ));
        Err(refused("test storage refuses persistence"))
    }

    fn set_roundtrip(
        &self,
        path: &ResolvedPath,
        _: &ContentHash,
        alignment: AlignmentValidation,
        outcome: RoundtripOutcome,
    ) -> Result<(), CacheError> {
        self.attempts
            .lock()
            .expect("attempt log")
            .push(Attempt::Roundtrip(
                path.as_path().to_owned(),
                alignment,
                outcome,
            ));
        Err(refused("test storage refuses persistence"))
    }
}
