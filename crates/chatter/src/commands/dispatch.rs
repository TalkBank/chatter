//! CLI command routing: one exhaustive match from each parsed `Commands`
//! variant to the code that runs it, so `cli/run.rs` stays a small
//! composition root.

use crate::cli::{
    CheckCommands, Commands, ConvertCommands, ExperimentalCommands, Flag, InspectCommands,
    NormalizeCheckArgs, ReviewCommands, SpeakerCommands, ToJsonCheckArgs,
};

use super::cache::run_cache_command;
use super::validate::run_validate_command;
use super::validate_parallel::{
    RunNotice, ValidateDirectoryOptions, ValidationExecution, ValidationRules,
};
use super::{
    ValidationPresentation, chat_to_json, clean_file, create_new_file, json_to_chat,
    normalize_chat, run_fix, run_schema, run_update, show_alignment, watch_files,
};

pub fn dispatch_command(command: Commands) -> crate::exit_codes::CommandResult {
    match command {
        Commands::Validate {
            path,
            format,
            alignment: Flag(alignment),
            cache_refresh: Flag(cache_refresh),
            jobs,
            quiet,
            max_errors,
            roundtrip: Flag(roundtrip),
            parser,
            strict_linkers: Flag(linkers),
            audit,
            suppress,
            tui_mode,
            theme,
        } => {
            /// A usage error, exit 2.
            fn usage(
                kind: clap::error::ErrorKind,
                message: String,
            ) -> crate::exit_codes::CommandStop {
                crate::exit_codes::CommandStop::Usage(
                    <crate::cli::Cli as clap::CommandFactory>::command().error(kind, message),
                )
            }
            /// Flags naming two outputs, or `--refresh-cache` on an audit.
            fn conflict(
                conflict: super::validate_parallel::PresentationConflict,
            ) -> crate::exit_codes::CommandStop {
                usage(
                    clap::error::ErrorKind::ArgumentConflict,
                    conflict.to_string(),
                )
            }
            let paths = super::inputs::NonEmpty::new(path).ok_or_else(|| {
                usage(
                    clap::error::ErrorKind::MissingRequiredArgument,
                    "validate needs at least one path".to_owned(),
                )
            })?;
            let presentation = ValidationPresentation::resolve(
                format,
                quiet,
                audit,
                tui_mode,
                crate::ui::Theme::load(theme),
            )
            .map_err(conflict)?;
            let cache =
                super::validate_parallel::CachePolicy::for_run(&presentation, cache_refresh)
                    .map_err(conflict)?;
            let suppress = super::error_codes::SuppressedCodes::of(&suppress);
            run_validate_command(
                paths,
                ValidateDirectoryOptions {
                    notices: RunNotice::for_suppression(&suppress),
                    rules: ValidationRules {
                        alignment,
                        roundtrip,
                        parser_kind: parser.into(),
                        linkers,
                    },
                    execution: ValidationExecution {
                        cache,
                        jobs,
                        error_limit: match max_errors {
                            None => talkbank_transform::ErrorLimit::Unlimited,
                            Some(limit) => talkbank_transform::ErrorLimit::StopAfter(limit),
                        },
                    },
                    presentation,
                    suppress,
                },
            )?;
        }
        Commands::Watch {
            path,
            parser,
            alignment: Flag(alignment),
            clear,
        } => {
            watch_files(&path, parser.into(), alignment, clear)?;
        }
        // Utilities.
        Commands::Format {
            input,
            output,
            checks: NormalizeCheckArgs(level),
        } => normalize_chat(&input, output.as_ref(), level)?,
        Commands::Fix {
            paths,
            mode: Flag(mode),
            codes,
            alignment: Flag(alignment),
        } => {
            if run_fix(&paths, mode, &codes, alignment)?.failed() {
                return Err(crate::exit_codes::ReportedFailure::Input.into());
            }
        }
        Commands::New {
            output,
            speaker,
            language,
            role,
            corpus,
            utterance,
        } => create_new_file(
            output.as_deref(),
            &speaker,
            &language,
            &role,
            &corpus,
            utterance.as_deref(),
        )?,
        Commands::Schema { url } => run_schema(url)?,
        Commands::Cache { command } => run_cache_command(command)?,
        Commands::Convert { command } => run_convert(command)?,
        Commands::Inspect { command } => run_inspect(command)?,
        Commands::Experimental { command } => match command {
            ExperimentalCommands::Speakers { command } => run_speakers(command)?,
            ExperimentalCommands::Review { command } => run_review(command)?,
            ExperimentalCommands::Transform { command } => run_debug(command)?,
            ExperimentalCommands::Update => run_update()?,
        },
        Commands::Discovery { command } => {
            use crate::cli::args::DiscoveryCommands;
            let result = match command {
                DiscoveryCommands::List { format } => crate::cli::catalog::run(&[], format),
                DiscoveryCommands::Show { path, format } => crate::cli::catalog::run(&path, format),
            };
            result?;
        }
        Commands::Checks { command } => match command {
            CheckCommands::List => super::list_checks::print_check_list()?,
            CheckCommands::Explain { code } => super::list_checks::explain_check(code)?,
        },
    }
    Ok(())
}
fn run_convert(command: ConvertCommands) -> crate::exit_codes::CommandResult {
    match command {
        ConvertCommands::ToJson {
            input,
            output,
            output_dir,
            layout: Flag(layout),
            refresh: Flag(refresh),
            orphans: Flag(orphans),
            jobs,
            checks: ToJsonCheckArgs(chat),
            schema: Flag(schema),
        } => {
            // Every flag arrived as the value it selects.
            let checks = super::json::ToJsonChecks { chat, schema };
            let target = super::json::ToJsonTarget::resolve(
                super::json::InputKind::of(&input),
                output,
                output_dir,
                refresh,
                orphans,
                jobs,
            )
            .map_err(|usage| {
                eprintln!("Error: {usage}");
                crate::exit_codes::CommandStop::Reported(
                    crate::exit_codes::ReportedFailure::Precondition,
                )
            })?;
            match target {
                super::json::ToJsonTarget::File { output } => {
                    chat_to_json(&input, output.as_ref(), layout, checks)?
                }
                super::json::ToJsonTarget::Directory {
                    output_dir,
                    refresh,
                    orphans,
                    jobs,
                } => super::json::chat_to_json_directory(
                    &input,
                    &output_dir,
                    layout,
                    checks,
                    refresh,
                    orphans,
                    jobs,
                )?,
            }
        }
        ConvertCommands::FromJson { input, output } => json_to_chat(&input, output.as_ref())?,
    }
    Ok(())
}
fn run_inspect(command: InspectCommands) -> crate::exit_codes::CommandResult {
    match command {
        InspectCommands::Alignment {
            input,
            tier,
            view: Flag(view),
        } => show_alignment(&input, tier, view)?,
        InspectCommands::Words {
            path,
            diff_only,
            format,
        } => clean_file(&path, diff_only, format)?,
        InspectCommands::Overlaps { path, database } => {
            super::debug::run_overlap_audit(&path, database.as_deref())?;
        }
        InspectCommands::Linkers { path, anomalies } => {
            super::debug::run_linker_audit(&path, anomalies.as_deref())?;
        }
    }
    Ok(())
}
fn run_speakers(command: SpeakerCommands) -> crate::exit_codes::CommandResult {
    use super::speaker_id::{SpeakerRequest, run_speaker_id};
    match command {
        SpeakerCommands::Map {
            input,
            mapping,
            output,
        } => run_speaker_id(
            &input,
            output.as_ref(),
            SpeakerRequest::Map {
                specification: &mapping,
            },
        )?,
        SpeakerCommands::Match {
            input,
            reference,
            anchor,
            inserted_role,
            confidence_threshold,
            write_match_report,
            write_override,
            write_pending,
            output,
        } => run_speaker_id(
            &input,
            output.as_ref(),
            SpeakerRequest::Match {
                reference: &reference,
                anchor: &anchor,
                inserted_role: &inserted_role,
                confidence_threshold,
                write_match_report_path: write_match_report.as_deref(),
                write_override_path: write_override.as_deref(),
                write_pending_path: write_pending.as_deref(),
            },
        )?,
        SpeakerCommands::Replay {
            input,
            override_file,
            session_id,
            output,
        } => run_speaker_id(
            &input,
            output.as_ref(),
            SpeakerRequest::Replay {
                overrides: &override_file,
                session: &session_id,
            },
        )?,
        SpeakerCommands::Suggest {
            input,
            anchor,
            write_pending,
            provider,
        } => {
            super::speaker_id::run_suggestion(&input, anchor.as_deref(), &write_pending, provider)?
        }
        SpeakerCommands::Rediarize {
            input,
            turns,
            output,
            summary_json,
            contested_at,
        } => crate::commands::rediarize::run_rediarize(
            &input,
            &turns,
            output.as_ref(),
            summary_json.as_deref(),
            contested_at,
        )?,
        SpeakerCommands::Scan {
            merged_dir,
            override_file,
            anchor,
            threshold,
            write_pending,
        } => crate::commands::sanity_scan::run_sanity_scan(
            &merged_dir,
            &override_file,
            &anchor,
            threshold,
            &write_pending,
        )?,
    }
    Ok(())
}

fn run_review(command: ReviewCommands) -> crate::exit_codes::CommandResult {
    use super::adjudicate::ReviewSource;
    match command {
        ReviewCommands::Scripted {
            pending,
            override_file,
            script,
            operator,
        } => super::adjudicate::run_adjudicate(
            &pending,
            &override_file,
            ReviewSource::Scripted(&script),
            &operator,
        )?,
        ReviewCommands::Interactive {
            pending,
            override_file,
            operator,
        } => super::adjudicate::run_adjudicate(
            &pending,
            &override_file,
            ReviewSource::Interactive,
            &operator,
        )?,
    }
    Ok(())
}
fn run_debug(command: crate::cli::DebugCommands) -> crate::exit_codes::CommandResult {
    use crate::cli::DebugCommands;
    match command {
        DebugCommands::RetagLanguage {
            from,
            to,
            path,
            commit: Flag(commit),
        } => {
            super::debug::run_retag_language(&path, &from, &to, commit)?;
        }
        DebugCommands::LanguageRuns {
            path,
            commit: Flag(commit),
        } => {
            super::debug::run_fix_s(&path, commit)?;
        }
        DebugCommands::JoinRetrace {
            path,
            commit: Flag(commit),
            scope,
        } => {
            use crate::cli::JoinRetraceScope;
            use talkbank_transform::join_retrace::RetraceJoinScope;
            let transform_scope = match scope {
                JoinRetraceScope::Repetition => RetraceJoinScope::RepetitionOnly,
                JoinRetraceScope::Corrections => RetraceJoinScope::RepetitionAndCorrections,
                JoinRetraceScope::All => RetraceJoinScope::AllSameSpeakerSuccessor,
            };
            super::debug::run_join_retrace(&path, commit, transform_scope)?;
        }
        DebugCommands::Redact { input, output } => {
            super::debug::run_sanitize(&input, output.as_deref())?;
        }
    }
    Ok(())
}
