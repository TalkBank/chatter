//! File-send API for opening CHAT files in CLAN.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use std::ffi::CString;
use std::os::raw::c_long;

use crate::error::{Error, Result};
use crate::ffi;

/// Send file information to CLAN application.
///
/// This function performs a complete workflow to communicate with CLAN:
/// 1. Validates all parameters
/// 2. Launches CLAN if not already running (platform-specific)
/// 3. Sends file path, cursor position, and optional message to CLAN
///
/// The function is stateless and thread-safe. Multiple threads can call it
/// concurrently without coordination.
///
/// # Arguments
///
/// * `timeout` - Timeout in seconds for the entire operation. Use 0 for default (30 seconds).
///   Negative values are treated as 0. Recommended: 10-30 seconds for interactive use.
///
/// * `file_path` - Path to the .cha file to open in CLAN. Must be non-empty.
///   Can be absolute or relative (relative to CLAN's working directory).
///
/// * `line_number` - 1-based line number for cursor positioning. Must be >= 1.
///
/// * `column_number` - 1-based column number for cursor positioning. Must be >= 1.
///
/// * `message` - Optional error/status message to display in CLAN.
///   Typical use: error descriptions, warnings.
///
/// # Returns
///
/// * `Ok(())` - File was successfully sent to CLAN and opened at the cursor position
/// * `Err(Error)` - Operation failed with specific error details
///
/// # Platform Support
///
/// * **macOS**: Full support using Apple Events and Launch Services
/// * **Windows**: Full support using Win32 APIs and message files
/// * **Linux**: Returns `Err(Error::UnsupportedPlatform)`
///
/// # Performance
///
/// * CLAN already running: 100-500ms typical
/// * CLAN needs launching: 2-5 seconds (includes app startup)
/// * Timeout scenario: Up to `timeout` seconds
///
/// # Examples
///
/// ```rust,no_run
/// use send2clan::send_to_clan;
///
/// // Basic usage
/// send_to_clan(30, "/path/to/file.cha", 42, 15, Some("Syntax error"))?;
///
/// // Without error message
/// send_to_clan(30, "/path/to/file.cha", 1, 1, None)?;
///
/// // With custom timeout
/// send_to_clan(60, "/path/to/file.cha", 10, 5, Some("Warning"))?;
/// # Ok::<(), send2clan::Error>(())
/// ```
///
/// # Example with error handling and retry
///
/// ```rust,no_run
/// use send2clan::{send_to_clan, Error};
/// use std::thread;
/// use std::time::Duration;
///
/// let mut retries = 3;
/// loop {
///     match send_to_clan(30, "/path/to/file.cha", 42, 15, Some("Error")) {
///         Ok(()) => {
///             println!("Success!");
///             break;
///         }
///         Err(e) if e.is_recoverable() && retries > 0 => {
///             eprintln!("Retrying after error: {}", e);
///             retries -= 1;
///             thread::sleep(Duration::from_secs(1));
///         }
///         Err(e) => {
///             eprintln!("Failed: {}", e);
///             return Err(e);
///         }
///     }
/// }
/// # Ok::<(), send2clan::Error>(())
/// ```
pub fn send_to_clan(
    timeout: c_long,
    file_path: &str,
    line_number: i32,
    column_number: i32,
    message: Option<&str>,
) -> Result<()> {
    let file_path_c = CString::new(file_path)?;
    let message_c = message.map(CString::new).transpose()?;

    let result = unsafe {
        ffi::send2clan(
            timeout,
            file_path_c.as_ptr(),
            line_number,
            column_number,
            message_c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
        )
    };

    if result == 0 {
        Ok(())
    } else {
        Err(Error::from_code(result))
    }
}

/// Canonical CLAN Apple-Event timeout (seconds) for "open this file at this
/// location" requests.
///
/// The CLI/TUI has always passed `30`. Exposing it as a shared constant lets
/// every surface (CLI and desktop GUI) issue the byte-identical request the
/// working CLI does, instead of each picking its own timeout (the desktop GUI
/// previously passed a different value and diverged).
pub const OPEN_LOCATION_TIMEOUT_SECS: c_long = 30;

/// Open `file_path` in CLAN at an already-resolved CLAN line/column, asking CLAN
/// to highlight `message`.
///
/// Thin convenience over [`send_to_clan`] that fixes the canonical
/// [`OPEN_LOCATION_TIMEOUT_SECS`] timeout, so every caller issues identical
/// parameters and the surfaces cannot drift apart. Convert a `ParseError`
/// location into CLAN coordinates with `talkbank_model::resolve_clan_location`
/// first, then pass the resulting line/column and the error's message here.
pub fn open_location_in_clan(
    file_path: &str,
    clan_line: i32,
    clan_column: i32,
    message: &str,
) -> Result<()> {
    send_to_clan(
        OPEN_LOCATION_TIMEOUT_SECS,
        file_path,
        clan_line,
        clan_column,
        Some(message),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression guard: the shared open-location helper must use the same
    /// timeout the CLI/TUI has always used, so the desktop GUI issues the
    /// identical request rather than its own ad-hoc parameters.
    #[test]
    fn open_location_timeout_matches_cli_reference() {
        assert_eq!(OPEN_LOCATION_TIMEOUT_SECS, 30);
    }
}
