//! Presence flags parsed straight into the modes they select.
//!
//! A flag like `--skip-alignment` or `--compact` chooses between two values
//! of a typed mode. The translation happens once, where the flag is parsed:
//! [`Flag<M>`] implements clap's `Args` for any [`FlagMode`], and a
//! command's field is `#[command(flatten)] Flag<M>`. No boolean reaches a
//! command, and the libraries keep no bool constructors, which would exist
//! only so a CLI could translate its own flags.
//!
//! Two-flag combinations that select one value ([`ToJsonCheckArgs`],
//! [`NormalizeCheckArgs`]) get their own `Args` impls here, with the
//! combinations clap cannot express refused rather than resolved silently.

use clap::{Arg, ArgAction, ArgMatches, Args, Command, FromArgMatches};
use talkbank_model::CheckLevel;
use talkbank_model::validation::AlignmentValidation;

/// A mode chosen by one presence flag.
pub trait FlagMode: Sized + Clone + Send + Sync + 'static {
    /// The flag's long name, without dashes, also its clap id.
    const LONG: &'static str;
    /// The flag's help line.
    const HELP: &'static str;
    /// The flag's one-letter form, if it has one.
    const SHORT: Option<char> = None;
    /// The mode when the flag is absent.
    fn absent() -> Self;
    /// The mode when the flag is present.
    fn present() -> Self;
}

/// The mode a presence flag selected. Flatten it into a command's arguments:
/// `#[command(flatten)] alignment: Flag<AlignmentValidation>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flag<M>(pub M);

impl<M: FlagMode> FromArgMatches for Flag<M> {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, clap::Error> {
        Ok(Self(match matches.get_flag(M::LONG) {
            true => M::present(),
            false => M::absent(),
        }))
    }

    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), clap::Error> {
        *self = Self::from_arg_matches(matches)?;
        Ok(())
    }
}

impl<M: FlagMode> Args for Flag<M> {
    fn augment_args(command: Command) -> Command {
        let arg = Arg::new(M::LONG)
            .long(M::LONG)
            .action(ArgAction::SetTrue)
            .help(M::HELP);
        command.arg(match M::SHORT {
            Some(short) => arg.short(short),
            None => arg,
        })
    }

    fn augment_args_for_update(command: Command) -> Command {
        Self::augment_args(command)
    }
}

impl FlagMode for AlignmentValidation {
    const LONG: &'static str = "skip-alignment";
    const HELP: &'static str =
        "Skip dependent-tier alignment checks (alignment is checked by default)";
    fn absent() -> Self {
        Self::IncludeTierAlignment
    }
    fn present() -> Self {
        Self::Structure
    }
}

impl FlagMode for talkbank_transform::RoundtripCheck {
    const LONG: &'static str = "roundtrip";
    const HELP: &'static str = "Test serialization idempotency (developer tool)";
    fn absent() -> Self {
        Self::Skip
    }
    fn present() -> Self {
        Self::Run
    }
}

impl FlagMode for talkbank_transform::JsonLayout {
    const LONG: &'static str = "compact";
    const HELP: &'static str = "Compact (minified) JSON output instead of pretty-printed";
    fn absent() -> Self {
        Self::Pretty
    }
    fn present() -> Self {
        Self::Compact
    }
}

impl FlagMode for talkbank_transform::JsonSchemaPolicy {
    const LONG: &'static str = "skip-schema-validation";
    const HELP: &'static str = "Skip validation against the CHAT JSON Schema \
        (https://talkbank.org/schemas/v0.1/chat-file.json). \
        Useful for faster output when you trust the data model.";
    fn absent() -> Self {
        Self::Validate
    }
    fn present() -> Self {
        Self::Skip
    }
}

impl FlagMode for crate::commands::CacheRefreshMode {
    const LONG: &'static str = "force";
    const HELP: &'static str =
        "Force fresh validation (clears and updates cache for specified path)";
    fn absent() -> Self {
        Self::ReuseExisting
    }
    fn present() -> Self {
        Self::ForceRefresh
    }
}

impl FlagMode for crate::commands::json::JsonRefresh {
    const LONG: &'static str = "force";
    const HELP: &'static str = "Force full rebuild (ignore mtime, reconvert all files)";
    fn absent() -> Self {
        Self::Incremental
    }
    fn present() -> Self {
        Self::Rebuild
    }
}

impl FlagMode for crate::commands::json::OrphanJson {
    const LONG: &'static str = "prune";
    const HELP: &'static str = "Remove .json files with no matching .cha source (directory mode; links are never followed)";
    fn absent() -> Self {
        Self::Keep
    }
    fn present() -> Self {
        Self::Prune
    }
}

impl FlagMode for talkbank_model::LinkerChecks {
    const LONG: &'static str = "strict-linkers";
    const HELP: &'static str =
        "Enable strict cross-utterance linker validation (quotation and completion linkers)";
    fn absent() -> Self {
        Self::Lenient
    }
    fn present() -> Self {
        Self::Strict
    }
}

impl FlagMode for crate::commands::debug::Commit {
    const LONG: &'static str = "dry-run";
    const HELP: &'static str = "Show what would be joined without modifying any files";
    fn absent() -> Self {
        Self::Write
    }
    fn present() -> Self {
        Self::DryRun
    }
}

/// The clap ids of the check flags, each naming its argument where it is
/// declared and where it is read.
const SKIP_VALIDATION: &str = "skip-validation";
const SKIP_ALIGNMENT: &str = "skip-alignment";
const VALIDATE: &str = "validate";

/// `to-json`'s CHAT checks: `--skip-validation` and `--skip-alignment`
/// select one [`CheckLevel`]. Both together is a usage error: skipping
/// validation already skips alignment, so one half would do nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToJsonCheckArgs(pub CheckLevel);

impl FromArgMatches for ToJsonCheckArgs {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, clap::Error> {
        match (
            matches.get_flag(SKIP_VALIDATION),
            matches.get_flag(SKIP_ALIGNMENT),
        ) {
            (false, false) => Ok(Self(CheckLevel::Validate(
                AlignmentValidation::IncludeTierAlignment,
            ))),
            (false, true) => Ok(Self(CheckLevel::Validate(AlignmentValidation::Structure))),
            (true, false) => Ok(Self(CheckLevel::ParseOnly)),
            // `conflicts_with` below refuses the pair first.
            (true, true) => Err(clap::Error::raw(
                clap::error::ErrorKind::ArgumentConflict,
                "--skip-validation already skips alignment; do not also pass --skip-alignment\n",
            )),
        }
    }

    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), clap::Error> {
        *self = Self::from_arg_matches(matches)?;
        Ok(())
    }
}

impl Args for ToJsonCheckArgs {
    fn augment_args(command: Command) -> Command {
        command
            .arg(
                Arg::new(SKIP_ALIGNMENT)
                    .long(SKIP_ALIGNMENT)
                    .action(ArgAction::SetTrue)
                    .help("Disable tier alignment validation during conversion"),
            )
            .arg(
                Arg::new(SKIP_VALIDATION)
                    .long(SKIP_VALIDATION)
                    .action(ArgAction::SetTrue)
                    .conflicts_with(SKIP_ALIGNMENT)
                    .help("Skip validation of the CHAT data model (parse only, no alignment)"),
            )
    }

    fn augment_args_for_update(command: Command) -> Command {
        Self::augment_args(command)
    }
}

/// `normalize`'s checks: nothing by default, `--validate` to validate, and
/// `--skip-alignment` (which requires `--validate`) to validate without
/// alignment. `--skip-alignment` alone is refused: it would do nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NormalizeCheckArgs(pub CheckLevel);

impl FromArgMatches for NormalizeCheckArgs {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self, clap::Error> {
        match (matches.get_flag(VALIDATE), matches.get_flag(SKIP_ALIGNMENT)) {
            (false, false) => Ok(Self(CheckLevel::ParseOnly)),
            (true, false) => Ok(Self(CheckLevel::Validate(
                AlignmentValidation::IncludeTierAlignment,
            ))),
            (true, true) => Ok(Self(CheckLevel::Validate(AlignmentValidation::Structure))),
            // `requires` below refuses this first.
            (false, true) => Err(clap::Error::raw(
                clap::error::ErrorKind::MissingRequiredArgument,
                "--skip-alignment requires --validate\n",
            )),
        }
    }

    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), clap::Error> {
        *self = Self::from_arg_matches(matches)?;
        Ok(())
    }
}

impl Args for NormalizeCheckArgs {
    fn augment_args(command: Command) -> Command {
        command
            .arg(
                Arg::new(VALIDATE)
                    .long(VALIDATE)
                    .action(ArgAction::SetTrue)
                    .help("Validate and check alignment before writing output"),
            )
            .arg(
                Arg::new(SKIP_ALIGNMENT)
                    .long(SKIP_ALIGNMENT)
                    .action(ArgAction::SetTrue)
                    .requires(VALIDATE)
                    .help("Skip alignment checks (requires --validate)"),
            )
    }

    fn augment_args_for_update(command: Command) -> Command {
        Self::augment_args(command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse<A: Args + FromArgMatches>(flags: &[&str]) -> Result<A, clap::Error> {
        let matches = A::augment_args(Command::new("cmd"))
            .try_get_matches_from(std::iter::once("cmd").chain(flags.iter().copied()))?;
        A::from_arg_matches(&matches)
    }

    /// Each presence flag selects its mode; the mapping is what the hand
    /// impls own, and no type checks which value means "present".
    #[test]
    fn presence_flags_select_their_modes() {
        use super::super::fix_mode::FixMode;
        assert_eq!(
            parse::<Flag<FixMode>>(&[]).expect("parses").0,
            FixMode::Report
        );
        assert_eq!(
            parse::<Flag<FixMode>>(&["--apply"]).expect("parses").0,
            FixMode::Apply
        );
        assert_eq!(
            parse::<Flag<AlignmentValidation>>(&["--skip-alignment"])
                .expect("parses")
                .0,
            AlignmentValidation::Structure
        );
        assert_eq!(
            parse::<Flag<AlignmentValidation>>(&[]).expect("parses").0,
            AlignmentValidation::IncludeTierAlignment
        );
        assert_eq!(
            parse::<Flag<talkbank_transform::JsonLayout>>(&["--compact"])
                .expect("parses")
                .0,
            talkbank_transform::JsonLayout::Compact
        );
        assert_eq!(
            parse::<Flag<talkbank_transform::RoundtripCheck>>(&[])
                .expect("parses")
                .0,
            talkbank_transform::RoundtripCheck::Skip
        );
    }

    /// The two-flag checks select one level each and refuse a combination
    /// with a flag that would do nothing.
    #[test]
    fn check_flag_pairs_select_one_level_or_are_refused() {
        assert_eq!(
            parse::<ToJsonCheckArgs>(&["--skip-validation"])
                .expect("parses")
                .0,
            CheckLevel::ParseOnly
        );
        assert_eq!(
            parse::<ToJsonCheckArgs>(&["--skip-alignment"])
                .expect("parses")
                .0,
            CheckLevel::Validate(AlignmentValidation::Structure)
        );
        assert!(parse::<ToJsonCheckArgs>(&["--skip-validation", "--skip-alignment"]).is_err());
        assert_eq!(
            parse::<NormalizeCheckArgs>(&[]).expect("parses").0,
            CheckLevel::ParseOnly
        );
        assert_eq!(
            parse::<NormalizeCheckArgs>(&["--validate", "--skip-alignment"])
                .expect("parses")
                .0,
            CheckLevel::Validate(AlignmentValidation::Structure)
        );
        assert!(parse::<NormalizeCheckArgs>(&["--skip-alignment"]).is_err());
    }
}
