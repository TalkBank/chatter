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
use talkbank_transform::UnifiedCache;
use talkbank_transform::validation_runner::{
    ParserKind, ValidationConfig, ValidationEvent, is_chat_transcript_path,
    validate_directory_streaming, validate_files_streaming,
};

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
        Self {
            roundtrip: request.roundtrip,
            parser_kind: request.parser_kind.into(),
            strict_linkers: request.strict_linkers,
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
/// Called once, at app startup (`ValidationState::new()`), not per validation
/// run: it opens a SQLite pool and a dedicated tokio runtime, so building a
/// fresh one on every "Validate"/"Re-validate" click would pay that setup cost
/// repeatedly on a hot, user-facing path for no benefit.
pub fn initialize_cache() -> Option<Arc<UnifiedCache>> {
    UnifiedCache::open_or_else(|error| {
        eprintln!("Warning: Failed to initialize validation cache: {error}");
    })
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
