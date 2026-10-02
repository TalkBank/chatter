//! `chatter cache clear`'s selection and mode, parsed straight out of their
//! flags.
//!
//! clap's argument group already made exactly one of `--all` and `--prefix`
//! present; these impls turn the flags into the values the command uses, a
//! [`ClearScope`] (whose prefix is already resolved, as the cache stores
//! paths) and a [`ClearMode`] (a [`super::flag_modes::Flag`]), so the
//! command matches values rather than re-deciding what the flags meant.

use clap::{Arg, ArgAction, ArgGroup, ArgMatches, Args, Command, FromArgMatches};
use std::path::PathBuf;

use talkbank_transform::{CacheScope, ResolvedPrefix};

/// The clap ids, each naming its argument where it is declared and where it
/// is read.
const ALL: &str = "all";
const PREFIX: &str = "prefix";

/// Which cache entries `chatter cache clear` covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClearScope(pub CacheScope);

/// Whether `chatter cache clear` removes entries or only counts them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearMode {
    /// Remove them.
    Apply,
    /// Count them and remove nothing (`--dry-run`).
    DryRun,
}

impl FromArgMatches for ClearScope {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, clap::Error> {
        match (matches.get_flag(ALL), matches.get_one::<PathBuf>(PREFIX)) {
            (true, None) => Ok(Self(CacheScope::All)),
            // Resolved as the cache stores paths, so `--prefix corpus` and
            // an absolute or linked spelling of it select the same rows. Any
            // path the operating system accepts, UTF-8 or not.
            (false, Some(prefix)) => match ResolvedPrefix::of(prefix) {
                Ok(resolved) => Ok(Self(CacheScope::Under(resolved))),
                Err(error) => Err(clap::Error::raw(
                    clap::error::ErrorKind::ValueValidation,
                    format!("cannot resolve --prefix {}: {error}\n", prefix.display()),
                )),
            },
            // The required, exclusive group refuses both before we are
            // asked; reaching here means that declaration changed.
            (true, Some(_)) | (false, None) => Err(clap::Error::raw(
                clap::error::ErrorKind::ArgumentConflict,
                "exactly one of --all and --prefix is required\n",
            )),
        }
    }

    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), clap::Error> {
        *self = Self::from_arg_matches(matches)?;
        Ok(())
    }
}

impl Args for ClearScope {
    fn augment_args(command: Command) -> Command {
        command
            .arg(
                Arg::new(ALL)
                    .long("all")
                    .action(ArgAction::SetTrue)
                    .help("Clear all cache entries"),
            )
            .arg(
                Arg::new(PREFIX)
                    .long("prefix")
                    .value_name("PREFIX")
                    .value_parser(clap::value_parser!(PathBuf))
                    .help("Clear entries for this path and everything under it"),
            )
            .group(
                ArgGroup::new("scope")
                    .required(true)
                    .multiple(false)
                    .args([ALL, PREFIX]),
            )
    }

    fn augment_args_for_update(command: Command) -> Command {
        Self::augment_args(command)
    }
}

impl super::flag_modes::FlagMode for ClearMode {
    const LONG: &'static str = "dry-run";
    const HELP: &'static str = "Show what would be cleared without actually clearing";
    fn absent() -> Self {
        Self::Apply
    }
    fn present() -> Self {
        Self::DryRun
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::super::flag_modes::Flag;

    fn parse(flags: &[&str]) -> Result<(ClearScope, ClearMode), clap::Error> {
        let command =
            Flag::<ClearMode>::augment_args(ClearScope::augment_args(Command::new("clear")));
        let matches =
            command.try_get_matches_from(std::iter::once("clear").chain(flags.iter().copied()))?;
        Ok((
            ClearScope::from_arg_matches(&matches)?,
            Flag::<ClearMode>::from_arg_matches(&matches)?.0,
        ))
    }

    /// Each flag set names one scope and mode; a relative prefix is
    /// resolved where it is parsed; none or both scopes is a usage error.
    #[test]
    fn the_flags_select_one_scope_and_mode() {
        assert_eq!(
            parse(&["--all"]).expect("--all"),
            (ClearScope(CacheScope::All), ClearMode::Apply)
        );
        let (scope, mode) = parse(&["--prefix", "corpus", "--dry-run"]).expect("--prefix");
        assert_eq!(mode, ClearMode::DryRun);
        assert_eq!(
            scope,
            ClearScope(CacheScope::Under(
                ResolvedPrefix::of(std::path::Path::new("corpus")).expect("resolvable")
            ))
        );
        assert!(parse(&[]).is_err());
        assert!(parse(&["--all", "--prefix", "corpus"]).is_err());
    }
}
