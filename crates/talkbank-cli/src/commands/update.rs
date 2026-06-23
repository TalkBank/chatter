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
//! name. We therefore pass `CARGO_PKG_NAME`, which always matches the receipt and
//! survives a future package rename. With no receipt (build-from-source or a
//! package manager) self-update is unavailable and we say so rather than failing
//! opaquely. The facility is experimental (the cargo-dist updater is experimental
//! upstream).

use axoupdater::AxoUpdater;

use crate::exit_codes::EXIT_INPUT_ERROR;

/// Where a user reinstalls when in-place self-update is unavailable.
const RELEASES_URL: &str = "https://github.com/TalkBank/chatter/releases/latest";

/// Optional environment variable carrying a GitHub token to lift the
/// unauthenticated GitHub API rate limit (mainly relevant in CI). axoupdater
/// recommends an app-specific name so a stale ambient `GITHUB_TOKEN` cannot
/// interfere.
const GITHUB_TOKEN_ENV: &str = "CHATTER_GITHUB_TOKEN";

/// Run `chatter update`: self-update the running binary in place to the latest
/// release. Always terminates the process; never returns.
pub fn run_update() {
    let mut updater = AxoUpdater::new_for(env!("CARGO_PKG_NAME"));

    // The cargo-dist install receipt records what is installed; axoupdater needs
    // it to know the current version and where to install. Without it we cannot
    // self-update, so explain rather than fail opaquely.
    if let Err(err) = updater.load_receipt() {
        eprintln!(
            "chatter update: no install receipt found, so in-place self-update is unavailable."
        );
        eprintln!(
            "It works for installs from the official chatter installer. If you built from source"
        );
        eprintln!(
            "or used a package manager, update the same way, or reinstall the latest release:"
        );
        eprintln!("  {RELEASES_URL}");
        eprintln!("(details: {err})");
        std::process::exit(EXIT_INPUT_ERROR);
    }

    if let Ok(token) = std::env::var(GITHUB_TOKEN_ENV) {
        updater.set_github_token(&token);
    }

    // `run_sync` returns `Some(result)` when an update was installed, `None` when
    // already on the latest release.
    match updater.run_sync() {
        Ok(Some(_result)) => println!("chatter has been updated to the latest release."),
        Ok(None) => println!("chatter is already up to date."),
        Err(err) => {
            eprintln!("chatter update: self-update failed: {err}");
            eprintln!("You can reinstall the latest release from:");
            eprintln!("  {RELEASES_URL}");
            std::process::exit(EXIT_INPUT_ERROR);
        }
    }
}
