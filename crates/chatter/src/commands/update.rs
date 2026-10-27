//! `chatter update`: in-process self-update via the axoupdater library.
//!
//! Most CLIs self-update in place rather than shelling out to a separate updater
//! binary; chatter does the same. It embeds axoupdater (the cargo-dist
//! self-updater, used as a library) and, on `chatter update`, checks the latest
//! GitHub release and replaces the running binary in place. This removes the
//! standalone `chatter-update` program and the name-coupling that previously made
//! `chatter update` report "not installed" on a correct install: the cargo-dist
//! installer named the updater after the package (`talkbank-cli-update`) while the
//! launcher only looked for `chatter-update`.
//!
//! axoupdater identifies the installed app from the cargo-dist install receipt
//! (`$XDG_CONFIG_HOME/<app>/` or `~/.config/<app>/`), keyed by the cargo PACKAGE
//! name. New installs use `CARGO_PKG_NAME`; legacy `chatter` receipts remain
//! readable when the current receipt is absent. Once loaded, the receipt must
//! identify this executable and the official release source, then the updater
//! targets the current package identity. With no receipt (build-from-source or a
//! package manager) self-update is unavailable and we say so rather than failing
//! opaquely. The facility is experimental (the cargo-dist updater is experimental
//! upstream).

use axoupdater::{AxoUpdater, AxoupdateError, ReleaseSourceType, UpdateResult};

/// Where a user reinstalls when in-place self-update is unavailable.
const RELEASES_URL: &str = "https://github.com/TalkBank/chatter/releases/latest";

/// Optional environment variable carrying a GitHub token to lift the
/// unauthenticated GitHub API rate limit (mainly relevant in CI). axoupdater
/// recommends an app-specific name so a stale ambient `GITHUB_TOKEN` cannot
/// interfere.
const GITHUB_TOKEN_ENV: &str = "CHATTER_GITHUB_TOKEN";

/// Installer identity before the registry package rename, not the binary name.
const LEGACY_PACKAGE: &str = "chatter";

/// An updater admitted from a receipt for this executable and release source.
/// No unconfigured updater can reach the update operation through this wrapper.
struct InstalledUpdater(AxoUpdater);

#[derive(Debug, thiserror::Error)]
enum ReceiptError {
    #[error(transparent)]
    Load(Box<AxoupdateError>),
    #[error("the install receipt belongs to another executable installation")]
    OtherInstallation,
    #[error("the install receipt does not identify the official TalkBank/chatter release source")]
    UnexpectedSource,
}

impl From<AxoupdateError> for ReceiptError {
    fn from(error: AxoupdateError) -> Self {
        Self::Load(Box::new(error))
    }
}

impl InstalledUpdater {
    fn load() -> Result<Self, ReceiptError> {
        let mut updater = AxoUpdater::new_for(env!("CARGO_PKG_NAME"));
        let receipt_name = match updater.load_receipt() {
            Ok(_) => env!("CARGO_PKG_NAME"),
            Err(AxoupdateError::NoReceipt { .. }) => {
                updater.load_receipt_as(LEGACY_PACKAGE)?;
                LEGACY_PACKAGE
            }
            Err(err) => return Err(err.into()),
        };

        // Do not let an unrelated package's similarly named receipt redirect
        // self-update. A malformed current receipt never falls back to legacy.
        let official_source = updater.source.as_ref().is_some_and(|source| {
            matches!(source.release_type, ReleaseSourceType::GitHub)
                && source.owner.eq_ignore_ascii_case("TalkBank")
                && source.name.eq_ignore_ascii_case("chatter")
                && source.app_name == receipt_name
        });
        if !official_source {
            return Err(ReceiptError::UnexpectedSource);
        }
        if !updater.check_receipt_is_for_this_executable()? {
            return Err(ReceiptError::OtherInstallation);
        }

        // load_receipt_as retains the legacy release identity. Change it only
        // after admission; subsequent downloads use the current installer.
        updater.set_name(env!("CARGO_PKG_NAME"));
        Ok(Self(updater))
    }

    fn run(mut self) -> Result<Option<UpdateResult>, Box<AxoupdateError>> {
        if let Ok(token) = std::env::var(GITHUB_TOKEN_ENV) {
            self.0.set_github_token(&token);
        }
        self.0.run_sync().map_err(Box::new)
    }
}

/// Run `chatter update`: self-update the running binary in place to the latest
/// release. Return success or a reported failure to the program boundary.
pub fn run_update() -> crate::exit_codes::CommandResult {
    let updater = match InstalledUpdater::load() {
        Ok(updater) => updater,
        Err(err) => {
            eprintln!(
                "chatter update: cannot use an official install receipt for this executable."
            );
            eprintln!(
                "It works for installs from the official chatter installer. If you built from source"
            );
            eprintln!(
                "or used a package manager, update the same way, or reinstall the latest release:"
            );
            eprintln!("  {RELEASES_URL}");
            eprintln!("(details: {err})");
            return Err(crate::exit_codes::ReportedFailure::Input.into());
        }
    };

    // `run_sync` returns `Some(result)` when an update was installed, `None` when
    // already on the latest release.
    match updater.run() {
        Ok(Some(_result)) => outln!("chatter has been updated to the latest release."),
        Ok(None) => outln!("chatter is already up to date."),
        Err(err) => {
            eprintln!("chatter update: self-update failed: {err}");
            eprintln!("You can reinstall the latest release from:");
            eprintln!("  {RELEASES_URL}");
            return Err(crate::exit_codes::ReportedFailure::Input.into());
        }
    }
    Ok(())
}
