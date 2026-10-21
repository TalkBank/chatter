//! Platform and CLAN-installation availability checks.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use crate::ffi;

/// Check if current platform is supported by send2clan.
///
/// Checks at compile time whether the library was built for
/// a supported platform (macOS or Windows). Always returns the same value
/// for a given binary.
///
/// Call this function early in your application to provide clear error messages
/// on unsupported platforms, rather than receiving errors at runtime.
///
/// This is a build-target flag, not a test of installed CLAN or message delivery.
///
/// # Examples
///
/// ```rust,no_run
/// use send2clan::is_platform_supported;
///
/// if !is_platform_supported() {
///     eprintln!("Error: send2clan only supports macOS and Windows");
///     std::process::exit(1);
/// }
///
/// // Sending can still fail; inspect the actual operation's result.
/// ```
pub fn is_platform_supported() -> bool {
    // Safety:
    // - This function takes no parameters and always succeeds
    // - It just returns a compile-time constant
    unsafe { ffi::ffi_is_platform_supported() }
}

/// Check if CLAN application is installed and available.
///
/// Queries platform installation-location heuristics. A positive answer does
/// not prove that launching or sending succeeds; a cached answer can become stale.
///
/// # Platform-Specific Detection
///
/// * **macOS**: Queries Launch Services with the C implementation's `CLANc` identifier.
/// * **Windows**: Reads `InstallPath` under `HKEY_LOCAL_MACHINE\Software\TalkBank\CLAN`
///   or uses the compiled default path only when that key does not exist.
///
/// On Windows the C bridge admits the registry value before using it: an
/// unopenable key or an unreadable, non-`REG_SZ`, empty, embedded-NUL or
/// over-long value yields `false` without probing any file. That admission is
/// verified against Win32 test doubles (`just send2clan-win32-doubles`), not on
/// native Windows; this boolean is not a launch guarantee.
///
/// # Examples
///
/// ```rust,no_run
/// use send2clan::is_clan_available;
///
/// if !is_clan_available() {
///     eprintln!("CLAN is not installed");
///     eprintln!("Download from: https://dali.talkbank.org/clan/");
///     std::process::exit(1);
/// }
///
/// // Availability is only a hint; sending still has independent failure modes.
/// ```
///
/// # Example with caching
///
/// ```rust,no_run
/// use send2clan::is_clan_available;
/// use std::sync::OnceLock;
///
/// static CLAN_AVAILABLE: OnceLock<bool> = OnceLock::new();
///
/// fn check_clan() -> bool {
///     *CLAN_AVAILABLE.get_or_init(|| is_clan_available())
/// }
/// ```
pub fn is_clan_available() -> bool {
    // Safety:
    // - This function takes no parameters and always succeeds
    // - It performs platform-specific checks to locate CLAN
    unsafe { ffi::ffi_is_clan_available() }
}
