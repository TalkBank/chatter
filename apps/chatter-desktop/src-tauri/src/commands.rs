//! Tauri command handlers.

// The tauri::command proc macro's generated wrappers for
// Result-returning commands with borrowed State contain unreachable!
// arms; the panic-policy lint fires on those EXPANSIONS (function-level
// allows do not reach the generated sibling items, verified both
// attribute orders 2026-07-08). Module-scoped allow with this comment
// is the narrowest working scope; our own code in this module must
// still not write unreachable! (reviewed at the PR level).
#![allow(clippy::unreachable)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use talkbank_transform::report_destination::ReportInputs;

use dashmap::DashMap;
use tauri::{AppHandle, Emitter, State};

use crate::errors::{ClanError, ExportError, OpenExternalError, RevealError, ValidationStartError};
use crate::protocol::commands::{
    CancelValidationRequest, ExportFormat, ExportResultsRequest, OpenInClanRequest, ValidateRequest,
};
use crate::validation::{initialize_cache, initialize_cache_at};
use talkbank_transform::validation_runner::{ValidationConfig, ValidationRun};
use talkbank_transform::{CacheIdentity, UnifiedCache};

/// Shared state: ownership of the pending or running validation, and the
/// on-disk validation cache pools opened so far this app session.
///
/// The session owner uses `ArcSwapOption` for lock-free replacement and scoped
/// cancellation, no mutex needed. `caches` is a [`DashMap`] (the same concurrent-map idiom
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
/// under the OTHER setting. [`Self::cache_for_config`] is the seam that
/// fixes this, using the SAME [`ValidationConfig::cache_identity`]
/// composition the CLI uses (`crates/chatter/src/commands/validate/cache.rs`),
/// not a second mechanism.
///
/// Pools are opened lazily and memoized per distinct [`CacheIdentity`]:
/// opening one costs a SQLite pool + a dedicated tokio runtime, so a request
/// under a config already seen this session reuses the pool (the same "open
/// once" behavior the old single-cache design had when only one config
/// could ever exist), while a request under a genuinely new config (the
/// user just flipped a setting) pays that cost once and remembers the
/// result for next time.
pub struct ValidationState {
    runs: Arc<crate::validation_session::ValidationSessions>,
    /// Explicit cache root for test isolation; `None` uses the platform
    /// default (or `TALKBANK_CHAT_CACHE_DIR`). See [`Self::new_at`].
    cache_dir: Option<PathBuf>,
    caches: DashMap<CacheIdentity, Arc<UnifiedCache>>,
}

impl ValidationState {
    pub fn new() -> Self {
        Self {
            runs: Arc::default(),
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
            runs: Arc::default(),
            cache_dir: Some(cache_dir),
            caches: DashMap::new(),
        }
    }

    /// Return the cache pool keyed to the request's active rules, compiled
    /// grammar, and parser, opening and memoizing a pool on first use. See the struct docs for why
    /// this exists and what it fixes; the parser dimension closes the same
    /// stale-verdict shape one dimension over (a grammar change alters what
    /// parses, hence what validates, exactly like a rule-set or
    /// strict-linkers change does).
    pub fn cache_for_config(
        &self,
        config: &ValidationConfig,
    ) -> Result<Arc<UnifiedCache>, talkbank_transform::CacheError> {
        let identity = config.cache_identity();
        if let Some(existing) = self.caches.get(&identity) {
            return Ok(Arc::clone(existing.value()));
        }

        let opened = match &self.cache_dir {
            Some(dir) => initialize_cache_at(dir.clone(), identity.clone()),
            None => initialize_cache(identity.clone()),
        }?;
        // A concurrent first-use race can open two pools for the same
        // version; both are equally valid (same on-disk DB, same version
        // column), so whichever `insert` lands last simply wins the memo
        // slot. No mutex is worth adding to prevent that harmless
        // duplication.
        self.caches.insert(identity, Arc::clone(&opened));
        Ok(opened)
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
pub async fn validate(
    app: AppHandle,
    state: State<'_, ValidationState>,
    request: ValidateRequest,
) -> Result<(), ValidationStartError> {
    // `AssertUnwindSafe` is sound here for the reason the wrapper exists: on a
    // panic this run is abandoned outright, so no caller observes the partially
    // updated state a panic might leave behind. The pending lease releases only
    // its own cancel slot, and the event channel is dropped with the run.
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
    let pending = state.runs.begin(request.run_id.clone());

    let config = ValidationConfig::from(&request);
    // The cache pool MUST be selected from THIS request's active rule
    // selection, via the same seam `cache_for_config` composes the cache key
    // from (`RulesVersion::current_with_rule_selection`), not a single pool
    // opened once at app startup: see `ValidationState`'s docs for why.
    //
    // The pool is bound to the configuration it was opened for, so the run
    // cannot read another configuration's verdicts. A cache that will not open
    // (or bind) is said to the user, before the run's own events, and the run
    // goes on without it.
    let bound = state
        .cache_for_config(&config)
        .map_err(|error| error.to_string())
        .and_then(|cache| {
            ValidationRun::new(
                config.clone(),
                talkbank_transform::RunCache::ReadWrite(cache),
            )
            .map_err(|mismatch| mismatch.to_string())
        });
    let run = match bound {
        Ok(run) => run,
        Err(reason) => {
            let _ = app.emit(
                crate::protocol::events::VALIDATION,
                &pending.cache_unavailable(reason),
            );
            ValidationRun::uncached(config)
        }
    };
    let active = pending.start(request.path.into(), &run)?;

    // Spawn a thread to forward events to the frontend
    let app_clone = app.clone();
    std::thread::spawn(move || {
        while let Ok(event) = active.next_event() {
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
pub async fn cancel_validation(
    state: State<'_, ValidationState>,
    request: CancelValidationRequest,
) -> Result<(), ()> {
    // A stale request cannot take the newer run's cancellation slot. Cancelling
    // startup revokes admission; a later attempt to attach its runner stops it.
    state.runs.cancel(&request.run_id);
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
///
/// Takes one `request`, the `OpenInClanRequest` the backend works from, as
/// `validate` takes its `ValidateRequest`.
#[tauri::command]
pub async fn open_in_clan(request: OpenInClanRequest) -> Result<(), ClanError> {
    open_in_clan_request(request)
}

/// The exact CLAN coordinates + highlight message an Open-in-CLAN request
/// resolves to.
///
/// Separated from the Apple-Event send so the resolution (read file +
/// `resolve_clan_location` + message selection) is testable without launching
/// CLAN, and so the GUI's resolution can be compared against the CLI's.
#[derive(Debug, PartialEq, Eq)]
pub struct ResolvedClanTarget {
    file: String,
    /// CLAN-adjusted 1-indexed line (CLAN's hidden headers subtracted).
    line: i32,
    /// 1-indexed column.
    column: i32,
    /// Highlight message CLAN should locate, the bare error message.
    message: String,
}

/// Resolve an Open-in-CLAN request to its CLAN target without sending (no FFI).
///
/// Reads the file and admits only the diagnostic's original source revision,
/// then converts the error location into
/// CLAN-display coordinates (subtracting CLAN's hidden headers via the shared
/// `talkbank_model::resolve_clan_location`), and carries the request's message
/// through verbatim. Coordinate projection uses the same mapper as the CLI/TUI.
/// Admission does not prevent an external edit after this read or while CLAN opens.
pub fn resolve_open_in_clan(request: OpenInClanRequest) -> Result<ResolvedClanTarget, ClanError> {
    let source =
        std::fs::read_to_string(&request.file).map_err(|source| ClanError::ReadSource {
            path: PathBuf::from(&request.file),
            source,
        })?;

    if !request.source_revision.matches(&source) {
        return Err(ClanError::SourceChanged {
            path: PathBuf::from(&request.file),
        });
    }

    // The frontend sends its line and column as signed numbers; only a
    // one-based pair is a position, otherwise CLAN coordinates are computed
    // from the byte offset.
    let span = talkbank_model::Span::at(request.byte_offset);
    let position = usize::try_from(request.line)
        .ok()
        .zip(usize::try_from(request.col).ok())
        .and_then(|(line, column)| talkbank_model::LineColumn::one_based(line, column));
    let location = match position {
        Some(position) => talkbank_model::DiagnosticLocation::known_at(span, position),
        None => talkbank_model::DiagnosticLocation::new(span),
    };

    let clan_loc = talkbank_model::resolve_clan_location(&location, &source)?;

    Ok(ResolvedClanTarget {
        file: request.file,
        line: clan_loc.line as i32,
        column: clan_loc.column as i32,
        message: request.msg,
    })
}

impl ResolvedClanTarget {
    pub fn line(&self) -> i32 {
        self.line
    }
    pub fn column(&self) -> i32 {
        self.column
    }
    pub fn message(&self) -> &str {
        &self.message
    }

    fn send(self) -> Result<(), ClanError> {
        // Keep the checked path and coordinates together through dispatch.
        send2clan::open_location_in_clan(&self.file, self.line, self.column, &self.message)
            .map_err(ClanError::Send)
    }
}

pub fn open_in_clan_request(request: OpenInClanRequest) -> Result<(), ClanError> {
    resolve_open_in_clan(request)?.send()
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

/// Export validation results to a file. Takes one `request`, the
/// `ExportResultsRequest` the backend works from.
#[tauri::command]
pub async fn export_results(request: ExportResultsRequest) -> Result<(), ExportError> {
    export_results_request(request)
}

/// Typed admission for both export formats. Missing paths, labels or rendered
/// diagnostics are malformed input, not a reason to fabricate a placeholder.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportFile {
    path: PathBuf,
    errors: Vec<TextExportDiagnostic>,
    /// The file's outcome as the app shows it (`fileStatusLabel`), so the
    /// export says exactly what the screen said, from one owner.
    status_label: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextExportDiagnostic {
    rendered_text: String,
}

pub fn export_results_request(request: ExportResultsRequest) -> Result<(), ExportError> {
    let value: serde_json::Value = serde_json::from_str(&request.results)
        .map_err(|source| ExportError::MalformedResults { source })?;
    // Both formats require the same source inventory, including valid files
    // hidden by the UI. Keep the original JSON value for lossless JSON export.
    let files: Vec<ExportFile> = serde::Deserialize::deserialize(&value)
        .map_err(|source| ExportError::MalformedResults { source })?;
    let output = match request.format {
        ExportFormat::Json => serde_json::to_string_pretty(&value)
            .map_err(|source| ExportError::MalformedResults { source })?,
        ExportFormat::Text => {
            // The canonical miette-rendered text computed once in `events.rs`
            // (the text the on-screen error panel shows) and the app's own
            // status label, so exported text is what the app displayed.
            let mut lines = Vec::new();
            for file_entry in &files {
                lines.push(format!(
                    "{}\n{}",
                    file_entry.path.display(),
                    file_entry.status_label
                ));
                for error in &file_entry.errors {
                    lines.push(error.rendered_text.clone());
                }
            }
            lines.join("\n")
        }
    };

    let inputs = ReportInputs::discover(files.iter().map(|file| file.path.as_path()))
        .map_err(ExportError::Destination)?;
    let destination = inputs
        .staged_destination(Path::new(&request.path))
        .map_err(ExportError::Destination)?;
    let write = || -> std::io::Result<()> {
        let mut writer = destination.into_writer()?;
        writer.write_all(output.as_bytes())?;
        writer.finish()?.publish()
    };
    write().map_err(|source| ExportError::Write {
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
