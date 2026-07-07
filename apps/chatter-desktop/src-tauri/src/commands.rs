//! Tauri command handlers.

use std::sync::Arc;

use arc_swap::ArcSwapOption;
use crossbeam_channel::Sender;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::protocol::commands::{
    ExportFormat, ExportResultsRequest, OpenInClanRequest, ParserKindRequest, ValidateRequest,
};
use crate::validation::{initialize_cache, validate_target_streaming_with_config};
use talkbank_transform::UnifiedCache;
use talkbank_transform::validation_runner::ValidationConfig;

/// Shared state: cancel sender for the current validation run, and the
/// on-disk validation cache opened once for the app's lifetime.
///
/// Uses `ArcSwapOption` for lock-free atomic swap of the cancel sender, no
/// mutex needed. The cache is opened once here rather than per validation
/// run: `UnifiedCache::new()` opens a SQLite pool and a dedicated tokio
/// runtime, so building a fresh one on every "Validate"/"Re-validate" click
/// would pay that setup cost repeatedly for no benefit.
pub struct ValidationState {
    cancel_tx: ArcSwapOption<Sender<()>>,
    cache: Option<Arc<UnifiedCache>>,
}

impl ValidationState {
    pub fn new() -> Self {
        Self {
            cancel_tx: ArcSwapOption::empty(),
            cache: initialize_cache(),
        }
    }
}

impl Default for ValidationState {
    fn default() -> Self {
        Self::new()
    }
}

/// Start validation on a single file or folder target.
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
) -> Result<(), String> {
    let request = ValidateRequest {
        path,
        roundtrip,
        parser_kind,
        strict_linkers,
        jobs,
    };
    if request.path.is_empty() {
        return Err("No path provided".into());
    }

    let config = ValidationConfig::from(&request);
    let (rx, cancel_tx) =
        validate_target_streaming_with_config(request.path.into(), config, state.cache.clone())?;

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
#[tauri::command]
pub async fn cancel_validation(state: State<'_, ValidationState>) -> Result<(), String> {
    // Atomically take the cancel sender (lock-free)
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
) -> Result<(), String> {
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
pub fn resolve_open_in_clan(request: &OpenInClanRequest) -> Result<ResolvedClanTarget, String> {
    let source = std::fs::read_to_string(&request.file).map_err(|e| e.to_string())?;

    let location = talkbank_model::SourceLocation {
        span: talkbank_model::Span::new(request.byte_offset, request.byte_offset),
        line: (request.line >= 1).then_some(request.line as usize),
        column: (request.col >= 1).then_some(request.col as usize),
    };

    let clan_loc =
        talkbank_model::resolve_clan_location(&location, &source).map_err(|e| e.to_string())?;

    Ok(ResolvedClanTarget {
        line: clan_loc.line as i32,
        column: clan_loc.column as i32,
        message: request.msg.clone(),
    })
}

pub fn open_in_clan_request(request: OpenInClanRequest) -> Result<(), String> {
    let target = resolve_open_in_clan(&request)?;

    // Route through the SAME shared primitive (and canonical timeout) the
    // CLI/TUI uses, so the desktop issues the identical CLAN request the working
    // CLI does instead of its own ad-hoc parameters.
    send2clan::open_location_in_clan(&request.file, target.line, target.column, &target.message)
        .map_err(|e| e.to_string())
}

/// Install the bundled CLI binary to a system path (VS Code-style).
///
/// On macOS/Linux: symlinks to `/usr/local/bin/chatter`.
/// On Windows: copies to a user-writable PATH location.
#[tauri::command]
pub async fn install_cli(app: AppHandle) -> Result<String, String> {
    let resource_path = app
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?
        .join("resources")
        .join("chatter");

    if !resource_path.exists() {
        return Err(format!(
            "Bundled CLI not found at {}. Build with `cargo build --release -p chatter` first.",
            resource_path.display()
        ));
    }

    #[cfg(unix)]
    {
        let target = std::path::PathBuf::from("/usr/local/bin/chatter");
        // Remove existing symlink or file
        if target.exists() || target.is_symlink() {
            std::fs::remove_file(&target).map_err(|e| {
                format!(
                    "Cannot remove existing {}: {}. Try running with sudo.",
                    target.display(),
                    e
                )
            })?;
        }
        std::os::unix::fs::symlink(&resource_path, &target).map_err(|e| {
            format!(
                "Cannot create symlink at {}: {}. Try running with sudo.",
                target.display(),
                e
            )
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
            .ok_or("Cannot determine local app data directory")?
            .join("Chatter")
            .join("chatter.exe");
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::copy(&resource_path, &target).map_err(|e| e.to_string())?;
        Ok(format!(
            "CLI installed to {}. Add this directory to your PATH.",
            target.display()
        ))
    }
}

/// Reveal a file in the platform file manager (Finder, Explorer, etc.).
#[tauri::command]
pub async fn reveal_in_file_manager(path: String) -> Result<(), String> {
    let path = std::path::Path::new(&path);
    if !path.exists() {
        return Err(format!("Path does not exist: {}", path.display()));
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(parent) = path.parent() {
            std::process::Command::new("xdg-open")
                .arg(parent)
                .spawn()
                .map_err(|e| e.to_string())?;
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
) -> Result<(), String> {
    export_results_request(ExportResultsRequest {
        results,
        format,
        path,
    })
}

pub fn export_results_request(request: ExportResultsRequest) -> Result<(), String> {
    let output = match request.format {
        ExportFormat::Json => {
            let parsed: serde_json::Value =
                serde_json::from_str(&request.results).map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())?
        }
        ExportFormat::Text => {
            // Reuse the canonical miette-rendered text already computed once in
            // `events.rs::to_frontend_event` (the same text the on-screen error
            // panel shows), instead of hand-rebuilding a poorer one-line
            // "path:line: code msg" form from raw JSON fields. Keeps exported
            // text byte-identical to what the app displayed.
            let parsed: Vec<serde_json::Value> =
                serde_json::from_str(&request.results).map_err(|e| e.to_string())?;
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

    std::fs::write(&request.path, output).map_err(|e| e.to_string())
}
