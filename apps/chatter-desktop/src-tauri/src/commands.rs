//! Tauri command handlers.

// The tauri::command proc macro's generated wrappers for
// Result-returning commands with borrowed State contain unreachable!
// arms; the panic-policy lint fires on those EXPANSIONS (function-level
// allows do not reach the generated sibling items, verified both
// attribute orders 2026-07-08). Module-scoped allow with this comment
// is the narrowest working scope; our own code in this module must
// still not write unreachable! (reviewed at the PR level).
#![allow(clippy::unreachable)]

use std::path::PathBuf;
use std::sync::Arc;

use arc_swap::ArcSwapOption;
use crossbeam_channel::Sender;
use dashmap::DashMap;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::errors::{
    ClanError, ExportError, InstallCliError, OpenExternalError, RevealError, ValidationStartError,
};
use crate::protocol::commands::{
    ExportFormat, ExportResultsRequest, OpenInClanRequest, ParserKindRequest, ValidateRequest,
};
use crate::validation::{
    initialize_cache_at_with_rules_version, initialize_cache_with_rules_version,
    validate_target_streaming_with_config,
};
use talkbank_transform::validation_runner::ValidationConfig;
use talkbank_transform::{GRAMMAR_FINGERPRINT, RulesVersion, UnifiedCache};

/// Shared state: cancel sender for the current validation run, and the
/// on-disk validation cache pools opened so far this app session.
///
/// Uses `ArcSwapOption` for lock-free atomic swap of the cancel sender, no
/// mutex needed. `caches` is a [`DashMap`] (the same concurrent-map idiom
/// `talkbank-lsp`'s backend state uses), not a `Mutex<HashMap<...>>>`: this
/// crate's coding standard bans an explicit mutex, and `DashMap`'s sharded
/// internal locking is the sanctioned alternative.
///
/// # Why more than one cache pool
///
/// A validation request's active [`talkbank_model::RuleSelection`]
/// (suppressed codes, strict-linkers) changes which files count as Valid, so
/// a cache row produced under one active config is not a valid answer for a
/// different one, the exact defect this whole struct exists to prevent: it
/// used to open exactly ONE `RulesVersion::current()` pool for the app's
/// whole lifetime, so toggling "Strict linkers" in the settings panel and
/// re-validating the same file could silently reuse a verdict computed
/// under the OTHER setting. [`Self::cache_for_rules`] is the seam that
/// fixes this, using the SAME [`RulesVersion::current_with_rule_selection`]
/// composition the CLI uses (`crates/chatter/src/commands/validate/cache.rs`),
/// not a second mechanism.
///
/// Pools are opened lazily and memoized per distinct [`RulesVersion`]:
/// opening one costs a SQLite pool + a dedicated tokio runtime, so a request
/// under a config already seen this session reuses the pool (the same "open
/// once" behavior the old single-cache design had when only one config
/// could ever exist), while a request under a genuinely new config (the
/// user just flipped a setting) pays that cost once and remembers the
/// result for next time.
pub struct ValidationState {
    cancel_tx: ArcSwapOption<Sender<()>>,
    /// Explicit cache root for test isolation; `None` uses the platform
    /// default (or `TALKBANK_CHAT_CACHE_DIR`). See [`Self::new_at`].
    cache_dir: Option<PathBuf>,
    caches: DashMap<RulesVersion, Arc<UnifiedCache>>,
}

impl ValidationState {
    pub fn new() -> Self {
        Self {
            cancel_tx: ArcSwapOption::empty(),
            cache_dir: None,
            caches: DashMap::new(),
        }
    }

    /// Test/isolation constructor: every cache pool this state opens is
    /// rooted at `cache_dir` instead of the platform default, so tests
    /// never touch the developer's real cache and never collide with each
    /// other. Mirrors `validation::initialize_cache_at`.
    pub fn new_at(cache_dir: PathBuf) -> Self {
        Self {
            cancel_tx: ArcSwapOption::empty(),
            cache_dir: Some(cache_dir),
            caches: DashMap::new(),
        }
    }

    /// Return the cache pool keyed to the request's active RULE SELECTION AND
    /// the grammar compiled into this binary, opening and memoizing a new
    /// pool on first use for that combination. See the struct docs for why
    /// this exists and what it fixes; the parser dimension closes the same
    /// stale-verdict shape one dimension over (a grammar change alters what
    /// parses, hence what validates, exactly like a rule-set or
    /// strict-linkers change does).
    pub fn cache_for_rules(
        &self,
        rules: &talkbank_model::RuleSelection,
    ) -> Option<Arc<UnifiedCache>> {
        let rules_version = RulesVersion::current_with_rule_selection(rules, GRAMMAR_FINGERPRINT);
        if let Some(existing) = self.caches.get(&rules_version) {
            return Some(Arc::clone(existing.value()));
        }

        let opened = match &self.cache_dir {
            Some(dir) => initialize_cache_at_with_rules_version(dir.clone(), rules_version.clone()),
            None => initialize_cache_with_rules_version(rules_version.clone()),
        }?;
        // A concurrent first-use race can open two pools for the same
        // version; both are equally valid (same on-disk DB, same version
        // column), so whichever `insert` lands last simply wins the memo
        // slot. No mutex is worth adding to prevent that harmless
        // duplication.
        self.caches.insert(rules_version, Arc::clone(&opened));
        Some(opened)
    }
}

impl Default for ValidationState {
    fn default() -> Self {
        Self::new()
    }
}

/// Start validation on a single file or folder target.
///
/// # Why the body is wrapped rather than written inline
///
/// A panic in here does not merely fail the run: it unwinds out of the command
/// and leaves the IPC promise UNSETTLED, so the frontend's `await` never
/// resolves and never rejects. The UI then sits in its `invoked` phase forever
/// with nothing to display, which is indistinguishable to the user from the app
/// being broken, and indistinguishable to us from a slow disk. That is exactly
/// how a nested-runtime panic in the cache (fixed in `talkbank_cache::blocking`)
/// stayed invisible for four weeks while being 100% reproducible.
///
/// So the outcome of this command is made TOTAL: it always settles, as `Ok` or
/// as `Err`, and a panic becomes an `Err` the frontend already knows how to
/// render (an alert plus an `aborted` run, from which Re-validate is offered).
/// Catching a panic is not a substitute for not panicking; it is a guarantee
/// that a run always has an outcome, which is the property the UI's phase
/// machine depends on and could not previously rely on.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn validate(
    app: AppHandle,
    state: State<'_, ValidationState>,
    path: String,
    roundtrip: bool,
    parser_kind: ParserKindRequest,
    strict_linkers: bool,
    jobs: Option<u32>,
) -> Result<(), ValidationStartError> {
    let request = ValidateRequest {
        path,
        roundtrip,
        parser_kind,
        strict_linkers,
        jobs,
    };

    // `AssertUnwindSafe` is sound here for the reason the wrapper exists: on a
    // panic this run is abandoned outright, so no caller observes the partially
    // updated state a panic might leave behind. The cancel slot is overwritten
    // by the next run, and the event channel is dropped with the run.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        start_validation(&app, &state, request)
    })) {
        Ok(outcome) => outcome,
        Err(panic) => Err(ValidationStartError::Panicked {
            message: describe_panic(panic.as_ref()),
        }),
    }
}

/// Extract a human-usable message from a caught panic payload.
///
/// The payload is `Box<dyn Any>`; the two shapes `panic!` actually produces are
/// `&str` (a literal message) and `String` (a formatted one). Anything else is
/// reported as unknown rather than guessed at, because a wrong message in a bug
/// report costs more than no message.
fn describe_panic(panic: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = panic.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = panic.downcast_ref::<String>() {
        message.clone()
    } else {
        "the validator panicked without a message".to_string()
    }
}

/// The actual startup sequence, separated so [`validate`] can guarantee it
/// always produces an outcome. Returns as soon as the run is streaming; the
/// run itself continues on its own threads.
fn start_validation(
    app: &AppHandle,
    state: &ValidationState,
    request: ValidateRequest,
) -> Result<(), ValidationStartError> {
    if request.path.is_empty() {
        return Err(ValidationStartError::EmptyPath);
    }

    let config = ValidationConfig::from(&request);
    // The cache pool MUST be selected from THIS request's active rule
    // selection, via the same seam `cache_for_rules` composes the cache key
    // from (`RulesVersion::current_with_rule_selection`), not a single pool
    // opened once at app startup: see `ValidationState`'s docs for why.
    let cache = state.cache_for_rules(&config.rules);
    let (rx, cancel_tx) =
        validate_target_streaming_with_config(request.path.into(), config, cache)?;

    // Atomically store the cancel sender (lock-free)
    state.cancel_tx.store(Some(Arc::new(cancel_tx)));

    // Spawn a thread to forward events to the frontend
    let app_clone = app.clone();
    std::thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            let _ = app_clone.emit(crate::protocol::events::VALIDATION, &event);
        }
    });

    Ok(())
}

/// Cancel the current validation run.
///
/// The `Result<(), ()>` clippy objects to is Tauri's requirement, not a
/// discarded error: the body's own comment says cancelling has no failure mode.
/// A fabricated error type would be worse than the empty one, since it would
/// imply failures that cannot happen.
#[allow(clippy::result_unit_err)]
#[tauri::command]
pub async fn cancel_validation(state: State<'_, ValidationState>) -> Result<(), ()> {
    // Atomically take the cancel sender (lock-free). Cancelling has no failure
    // mode: with no run in flight there is simply nothing to signal, and a
    // receiver that has already hung up means the run ended on its own. The
    // `Result` is retained only because Tauri requires it for a command taking
    // borrowed `State`; `Err` is uninhabited in practice.
    if let Some(tx) = state.cancel_tx.swap(None) {
        let _ = tx.send(());
    }
    Ok(())
}

/// Check if CLAN app is available on this platform.
#[tauri::command]
pub async fn check_clan_available() -> bool {
    send2clan::is_clan_available()
}

/// Open a file location in the CLAN app.
///
/// Uses `resolve_clan_location` from `talkbank-model`, the same function the
/// TUI uses. Resolves line/column from byte offset when not provided, adjusts
/// for CLAN hidden headers.
#[tauri::command]
pub async fn open_in_clan(
    file: String,
    line: i32,
    col: i32,
    byte_offset: u32,
    msg: String,
) -> Result<(), ClanError> {
    open_in_clan_request(OpenInClanRequest {
        file,
        line,
        col,
        byte_offset,
        msg,
    })
}

/// The exact CLAN coordinates + highlight message an Open-in-CLAN request
/// resolves to.
///
/// Separated from the Apple-Event send so the resolution (read file +
/// `resolve_clan_location` + message selection) is testable without launching
/// CLAN, and so the GUI's resolution can be compared against the CLI's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedClanTarget {
    /// CLAN-adjusted 1-indexed line (CLAN's hidden headers subtracted).
    pub line: i32,
    /// 1-indexed column.
    pub column: i32,
    /// Highlight message CLAN should locate, the bare error message.
    pub message: String,
}

/// Resolve an Open-in-CLAN request to its CLAN target without sending (no FFI).
///
/// Reads the file for source context, converts the error location into
/// CLAN-display coordinates (subtracting CLAN's hidden headers via the shared
/// `talkbank_model::resolve_clan_location`), and carries the request's message
/// through verbatim. This mirrors exactly what the CLI/TUI computes before it
/// hands off to `send2clan`.
pub fn resolve_open_in_clan(request: &OpenInClanRequest) -> Result<ResolvedClanTarget, ClanError> {
    let source =
        std::fs::read_to_string(&request.file).map_err(|source| ClanError::ReadSource {
            path: PathBuf::from(&request.file),
            source,
        })?;

    let location = talkbank_model::SourceLocation {
        span: talkbank_model::Span::new(request.byte_offset, request.byte_offset),
        line: (request.line >= 1).then_some(request.line as usize),
        column: (request.col >= 1).then_some(request.col as usize),
    };

    let clan_loc = talkbank_model::resolve_clan_location(&location, &source)?;

    Ok(ResolvedClanTarget {
        line: clan_loc.line as i32,
        column: clan_loc.column as i32,
        message: request.msg.clone(),
    })
}

pub fn open_in_clan_request(request: OpenInClanRequest) -> Result<(), ClanError> {
    let target = resolve_open_in_clan(&request)?;

    // Route through the SAME shared primitive (and canonical timeout) the
    // CLI/TUI uses, so the desktop issues the identical CLAN request the working
    // CLI does instead of its own ad-hoc parameters.
    send2clan::open_location_in_clan(&request.file, target.line, target.column, &target.message)
        .map_err(ClanError::Send)
}

/// Install the bundled CLI binary to a system path (VS Code-style).
///
/// On macOS/Linux: symlinks to `/usr/local/bin/chatter`.
/// On Windows: copies to a user-writable PATH location.
#[tauri::command]
pub async fn install_cli(app: AppHandle) -> Result<String, InstallCliError> {
    let resource_path = app
        .path()
        .resource_dir()
        .map_err(|source| InstallCliError::ResourceDir { source })?
        .join("resources")
        .join("chatter");

    if !resource_path.exists() {
        return Err(InstallCliError::NotBundled {
            path: resource_path,
        });
    }

    #[cfg(unix)]
    {
        let target = std::path::PathBuf::from("/usr/local/bin/chatter");
        // Remove existing symlink or file
        if target.exists() || target.is_symlink() {
            std::fs::remove_file(&target).map_err(|source| InstallCliError::RemoveExisting {
                path: target.clone(),
                source,
            })?;
        }
        std::os::unix::fs::symlink(&resource_path, &target).map_err(|source| {
            InstallCliError::Symlink {
                path: target.clone(),
                source,
            }
        })?;
        Ok(format!(
            "CLI installed: {} -> {}",
            target.display(),
            resource_path.display()
        ))
    }

    #[cfg(windows)]
    {
        let target = dirs::data_local_dir()
            .ok_or(InstallCliError::NoLocalDataDir)?
            .join("Chatter")
            .join("chatter.exe");
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|source| InstallCliError::Install {
                path: target.clone(),
                source,
            })?;
        }
        std::fs::copy(&resource_path, &target).map_err(|source| InstallCliError::Install {
            path: target.clone(),
            source,
        })?;
        Ok(format!(
            "CLI installed to {}. Add this directory to your PATH.",
            target.display()
        ))
    }
}

/// Reveal a file in the platform file manager (Finder, Explorer, etc.).
#[tauri::command]
pub async fn reveal_in_file_manager(path: String) -> Result<(), RevealError> {
    let path = std::path::Path::new(&path);
    if !path.exists() {
        return Err(RevealError::Missing {
            path: path.to_path_buf(),
        });
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()
            .map_err(|source| RevealError::Launch { source })?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn()
            .map_err(|source| RevealError::Launch { source })?;
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(parent) = path.parent() {
            std::process::Command::new("xdg-open")
                .arg(parent)
                .spawn()
                .map_err(|source| RevealError::Launch { source })?;
        }
    }

    Ok(())
}

/// Export validation results to a file.
#[tauri::command]
pub async fn export_results(
    results: String,
    format: ExportFormat,
    path: String,
) -> Result<(), ExportError> {
    export_results_request(ExportResultsRequest {
        results,
        format,
        path,
    })
}

pub fn export_results_request(request: ExportResultsRequest) -> Result<(), ExportError> {
    let output = match request.format {
        ExportFormat::Json => {
            let parsed: serde_json::Value = serde_json::from_str(&request.results)
                .map_err(|source| ExportError::MalformedResults { source })?;
            serde_json::to_string_pretty(&parsed)
                .map_err(|source| ExportError::MalformedResults { source })?
        }
        ExportFormat::Text => {
            // Reuse the canonical miette-rendered text already computed once in
            // `events.rs::to_frontend_event` (the same text the on-screen error
            // panel shows), instead of hand-rebuilding a poorer one-line
            // "path:line: code msg" form from raw JSON fields. Keeps exported
            // text byte-identical to what the app displayed.
            let parsed: Vec<serde_json::Value> = serde_json::from_str(&request.results)
                .map_err(|source| ExportError::MalformedResults { source })?;
            let mut lines = Vec::new();
            for file_entry in &parsed {
                let path = file_entry["path"].as_str().unwrap_or("?");
                if let Some(errors) = file_entry["errors"].as_array() {
                    for error in errors {
                        let rendered_text = error["renderedText"].as_str().unwrap_or("?");
                        lines.push(format!("{path}\n{rendered_text}"));
                    }
                }
            }
            lines.join("\n")
        }
    };

    std::fs::write(&request.path, output).map_err(|source| ExportError::Write {
        path: PathBuf::from(&request.path),
        source,
    })
}

/// Open an external `http(s)` URL in the user's default browser.
///
/// Backs the About dialog's links (talkbank.org, the GitHub repo). Only
/// `http`/`https` URLs are accepted, so a compromised webview cannot use this
/// to launch arbitrary local programs. The app carries no `shell`/`opener`
/// plugin, so this shells out to the platform opener directly.
#[tauri::command]
pub fn open_external(url: String) -> Result<(), OpenExternalError> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(OpenExternalError::NotHttp { url });
    }
    // Defense in depth: a well-formed URL percent-encodes whitespace and control
    // characters, so their raw presence marks a hostile string. Rejecting them
    // keeps such a string from ever reaching a platform opener.
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(OpenExternalError::Unprintable { url });
    }

    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(target_os = "macos")]
    command.arg(&url);

    #[cfg(target_os = "linux")]
    let mut command = std::process::Command::new("xdg-open");
    #[cfg(target_os = "linux")]
    command.arg(&url);

    // Use `explorer.exe`, which receives the URL as a single argv entry and
    // opens it in the default browser WITHOUT a shell. Never `cmd /C start`:
    // that hands the URL to the cmd.exe parser, a command-injection vector.
    #[cfg(target_os = "windows")]
    let mut command = std::process::Command::new("explorer");
    #[cfg(target_os = "windows")]
    command.arg(&url);

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    return Err("opening external URLs is unsupported on this platform".to_string());

    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        command
            .spawn()
            .map_err(|source| OpenExternalError::Launch { source })?;
        Ok(())
    }
}
