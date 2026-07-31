//! Desktop validation orchestration for a single selected target.
//!
//! Chatter's desktop contract is one target at a time:
//! - one `.cha` file
//! - or one directory
//!
//! Both cases route through the exact same shared streaming entrypoints the
//! CLI uses (`talkbank_transform::validation_runner::{validate_directory_streaming,
//! validate_files_streaming}`), with a real on-disk cache. Desktop must not
//! reimplement cache lookups, stats accounting, or per-file rule dispatch;
//! see `apps/chatter-desktop/CLAUDE.md` ("No desktop-local domain logic").

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender, unbounded};
use talkbank_transform::validation_runner::{
    ParserKind, ValidationConfig, ValidationEvent, is_chat_transcript_path,
    validate_directory_streaming, validate_files_streaming,
};
use talkbank_transform::{RulesVersion, UnifiedCache};

use crate::events::{FrontendEvent, to_frontend_event};
use crate::protocol::commands::{ParserKindRequest, ValidateRequest};

impl From<ParserKindRequest> for ParserKind {
    fn from(value: ParserKindRequest) -> Self {
        match value {
            ParserKindRequest::TreeSitter => ParserKind::TreeSitter,
            ParserKindRequest::Re2c => ParserKind::Re2c,
        }
    }
}

impl From<&ValidateRequest> for ValidationConfig {
    fn from(request: &ValidateRequest) -> Self {
        // `strict_linkers` now lives inside `model_config`
        // (`talkbank_model::ValidationConfig`) rather than as a bare bool on
        // the runner config, so it is set the same way the CLI folds
        // `--strict-linkers` in: via `with_strict_linkers()` when requested.
        // The desktop request carries no `--suppress` equivalent yet, so no
        // codes are disabled here.
        let model_config = if request.strict_linkers {
            talkbank_model::ValidationConfig::new().with_strict_linkers()
        } else {
            talkbank_model::ValidationConfig::new()
        };
        Self {
            roundtrip: request.roundtrip,
            parser_kind: request.parser_kind.into(),
            model_config,
            jobs: request.jobs.map(|jobs| jobs as usize),
            ..Self::default()
        }
    }
}

/// Start validation for a single desktop target with an explicit config and
/// cache, used by the `validate` Tauri command once a `ValidateRequest`
/// carries user-chosen settings (roundtrip, parser kind, strict linkers,
/// jobs). The cache is a parameter, not built here, so the app can open it
/// once at startup (`ValidationState::new()`) and reuse it across every
/// validate/re-validate call instead of paying SQLite-pool setup cost per run.
pub fn validate_target_streaming_with_config(
    target: PathBuf,
    config: ValidationConfig,
    cache: Option<Arc<UnifiedCache>>,
) -> Result<(Receiver<FrontendEvent>, Sender<()>), String> {
    if !target.exists() {
        return Err(format!("Path does not exist: {}", target.display()));
    }

    if target.is_dir() {
        let (validation_rx, cancel_tx) = validate_directory_streaming(&target, &config, cache);
        Ok((bridge_validation_events(validation_rx, target), cancel_tx))
    } else if target.is_file() {
        if !is_chat_transcript_path(&target) {
            return Err(format!(
                "Chatter validates one .cha file or one folder at a time: {}",
                target.display()
            ));
        }
        let root = target
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        let (validation_rx, cancel_tx) = validate_files_streaming(vec![target], &config, cache);
        Ok((bridge_validation_events(validation_rx, root), cancel_tx))
    } else {
        Err(format!(
            "Path is not a file or directory: {}",
            target.display()
        ))
    }
}

/// Construct the shared on-disk validation cache, the exact same construction
/// the CLI uses (`crates/chatter/src/commands/validate/cache.rs`), minus the
/// `--force`-clear step the desktop has no flag for. Zero CLI dependency:
/// `UnifiedCache::new()` resolves the OS cache dir on its own.
///
/// Keyed to [`RulesVersion::current`] (no suppression, no strict-linkers).
/// Callers that need a pool keyed to a specific active
/// `talkbank_model::ValidationConfig` (any desktop request whose settings
/// might not be the default) use [`initialize_cache_with_rules_version`]
/// instead; see `ValidationState::cache_for_config` in `commands.rs`, the
/// seam that opens (and memoizes) one pool per distinct config a session
/// actually uses, rather than one pool for the app's whole lifetime.
pub fn initialize_cache() -> Option<Arc<UnifiedCache>> {
    UnifiedCache::open_or_else(|error| {
        eprintln!("Warning: Failed to initialize validation cache: {error}");
    })
}

/// [`initialize_cache`], keyed to an explicit [`RulesVersion`] instead of
/// [`RulesVersion::current`].
///
/// Opening a SQLite pool and a dedicated tokio runtime costs real time, so
/// this is not called per validation run either: `ValidationState` opens (at
/// most) one pool per DISTINCT active config a session has actually used,
/// reusing it for every subsequent request under that same config, the same
/// "open once, reuse" behavior the single-cache design had back when only
/// one config could ever exist.
pub fn initialize_cache_with_rules_version(
    rules_version: RulesVersion,
) -> Option<Arc<UnifiedCache>> {
    UnifiedCache::open_or_else_with_rules_version(rules_version, |error| {
        eprintln!("Warning: Failed to initialize validation cache: {error}");
    })
}

/// Open the validation cache rooted at an explicit directory.
///
/// [`initialize_cache`] resolves the cache root from the platform default
/// (or the `TALKBANK_CHAT_CACHE_DIR` override), which is right for the
/// shipping app: there is exactly one cache and the user does not choose
/// where it lives.
///
/// In-process callers that need their OWN cache root, notably tests that
/// must not touch the developer's real cache and must not collide with
/// each other, take this instead. Before it existed the only lever was
/// mutating `TALKBANK_CHAT_CACHE_DIR` with `unsafe { set_var }`, which is
/// a process global: two tests in one binary overwrote each other's root,
/// and the `unsafe` was only sound under a one-process-per-test runner.
/// Passing the directory removes both problems, and the environment
/// variable goes back to its legitimate job of configuring CHILD
/// processes (the CLI subprocess tests set it via `Command::env`).
pub fn initialize_cache_at(cache_dir: PathBuf) -> Option<Arc<UnifiedCache>> {
    match UnifiedCache::with_directory(cache_dir) {
        Ok(cache) => Some(Arc::new(cache)),
        Err(error) => {
            eprintln!("Warning: Failed to initialize validation cache: {error}");
            None
        }
    }
}

/// [`initialize_cache_at`], keyed to an explicit [`RulesVersion`]. Test
/// counterpart of [`initialize_cache_with_rules_version`], used by
/// `ValidationState::new_at` so cache-key tests can isolate BOTH the
/// directory (never the developer's real cache) and the rule set (never a
/// row left behind by a differently-configured earlier test run).
pub fn initialize_cache_at_with_rules_version(
    cache_dir: PathBuf,
    rules_version: RulesVersion,
) -> Option<Arc<UnifiedCache>> {
    match UnifiedCache::with_directory_and_rules_version(cache_dir, rules_version) {
        Ok(cache) => Some(Arc::new(cache)),
        Err(error) => {
            eprintln!("Warning: Failed to initialize validation cache: {error}");
            None
        }
    }
}

fn bridge_validation_events(
    validation_rx: Receiver<ValidationEvent>,
    root: PathBuf,
) -> Receiver<FrontendEvent> {
    let (frontend_tx, frontend_rx) = unbounded();

    std::thread::spawn(move || {
        while let Ok(event) = validation_rx.recv() {
            match to_frontend_event(event, &root) {
                Some(frontend_event) => {
                    if frontend_tx.send(frontend_event).is_err() {
                        break;
                    }
                }
                None => continue,
            }
        }
    });

    frontend_rx
}
