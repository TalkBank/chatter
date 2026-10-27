//! Command-line interface handlers for `chatter`.
//!
//! This module contains CLI definitions and command dispatch.
//! Individual command implementations are in the `commands` module.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

pub(crate) mod args;
pub(crate) mod catalog;
mod logging;
mod run;

pub use args::{
    AlignmentTier, CacheCommands, CheckCommands, ClearMode, ClearScope, Cli, Commands,
    ConvertCommands, DebugCommands, ExperimentalCommands, FixMode, Flag, FlagMode, InspectCommands,
    JoinRetraceScope, LogFormat, NormalizeCheckArgs, OutputFormat, ReviewCommands, SpeakerCommands,
    ToJsonCheckArgs, TuiMode,
};
pub use logging::init_tracing;
pub use run::run;

/// Removed spellings remain usage errors, with a concrete migration route.
pub(crate) fn migration_hint(error: &clap::Error) -> Option<&'static str> {
    use clap::error::{ContextKind, ContextValue};
    let Some(ContextValue::String(command)) = error.get(ContextKind::InvalidSubcommand) else {
        return None;
    };
    match command.as_str() {
        "normalize" => Some("Use chatter format."),
        "to-json" => Some("Use chatter convert to-json."),
        "from-json" => Some("Use chatter convert from-json."),
        "show-alignment" => Some("Use chatter inspect alignment."),
        "clean" => {
            Some("Use chatter inspect words (this displays words; it does not repair CHAT).")
        }
        "new-file" => Some("Use chatter new."),
        "debug" => {
            Some("Use chatter inspect for audits or chatter experimental transform for rewrites.")
        }
        "speaker-id" => Some("Use chatter experimental speakers map, match, replay or suggest."),
        "rediarize" => Some("Use chatter experimental speakers rediarize."),
        "sanity-scan" => Some("Use chatter experimental speakers scan."),
        "adjudicate" => Some("Use chatter experimental review scripted or interactive."),
        "update" => Some("Use chatter experimental update."),
        _ => None,
    }
}
