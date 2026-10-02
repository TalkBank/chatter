//! CLI command routing: one exhaustive match from each parsed `Commands`
//! variant to the code that runs it, so `cli/run.rs` stays a small
//! composition root.

use crate::cli::{Commands, Flag, NormalizeCheckArgs, ToJsonCheckArgs};
use crate::ui::Theme;

use super::cache::run_cache_command;
use super::validate::run_validate_command;
use super::validate_parallel::{
    RunNotice, ValidateDirectoryOptions, ValidationExecution, ValidationRules,
};
use super::{
    ValidationPresentation, chat_to_json, clean_file, create_new_file, json_to_chat,
    normalize_chat, run_fix, run_schema, run_update, show_alignment, watch_files,
};

/// Runtime context shared by the top-level CLI commands.
#[derive(Clone)]
pub struct CommandContext {
    /// The `--tui-mode` flag, decided against the rest of a command's output
    /// flags by that command (`ValidationPresentation::resolve`).
    pub tui_mode: crate::cli::TuiMode,
    /// Loaded TUI color theme.
    pub theme: Theme,
}

/// Run one parsed top-level CLI command.
///
/// One exhaustive match over `Commands`: a new command is a compile error
/// here until it is routed, and no arm can receive a command it does not
/// handle.
pub fn dispatch_command(command: Commands, context: &CommandContext) {
    match command {
        // Validation: validate, show-alignment, watch.
        Commands::Validate {
            path,
            list_checks,
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
            check_xphon,
        } => {
            /// A usage error, exit 2.
            fn usage(kind: clap::error::ErrorKind, message: String) -> ! {
                <crate::cli::Cli as clap::CommandFactory>::command()
                    .error(kind, message)
                    .exit()
            }
            /// Flags naming two outputs, or `--force` on an audit.
            fn conflict(conflict: super::validate_parallel::PresentationConflict) -> ! {
                usage(
                    clap::error::ErrorKind::ArgumentConflict,
                    conflict.to_string(),
                )
            }
            // --list-checks prints the check list and reads no file; clap
            // refuses it beside a path and requires a path without it, and
            // the two arms that would contradict clap say so as usage errors.
            let paths = match (list_checks, super::inputs::NonEmpty::new(path)) {
                (true, None) => {
                    super::list_checks::print_check_list();
                    return;
                }
                (false, Some(paths)) => paths,
                (true, Some(_)) => usage(
                    clap::error::ErrorKind::ArgumentConflict,
                    "--list-checks reads no file; do not also pass a path".to_owned(),
                ),
                (false, None) => usage(
                    clap::error::ErrorKind::MissingRequiredArgument,
                    "validate needs a path (or --list-checks)".to_owned(),
                ),
            };
            let presentation = ValidationPresentation::resolve(
                format,
                quiet,
                audit,
                context.tui_mode,
                context.theme.clone(),
            )
            .unwrap_or_else(|error| conflict(error));
            let cache =
                super::validate_parallel::CachePolicy::for_run(&presentation, cache_refresh)
                    .unwrap_or_else(|error| conflict(error));
            let suppress = super::error_codes::SuppressedCodes::of(&suppress);
            run_validate_command(
                paths,
                ValidateDirectoryOptions {
                    notices: RunNotice::for_flags(check_xphon, &suppress),
                    rules: ValidationRules {
                        alignment,
                        roundtrip,
                        parser_kind: match parser {
                            crate::cli::ParserBackend::TreeSitter => {
                                talkbank_transform::ParserKind::TreeSitter
                            }
                            crate::cli::ParserBackend::Re2c => talkbank_transform::ParserKind::Re2c,
                        },
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
            );
        }
        Commands::ShowAlignment {
            input,
            tier,
            view: Flag(view),
        } => show_alignment(&input, tier, view),
        Commands::Watch {
            path,
            alignment: Flag(alignment),
            clear,
        } => {
            if let Err(err) = watch_files(&path, alignment, clear) {
                eprintln!("Error: {}", err);
                std::process::exit(1);
            }
        }
        // Utilities.
        Commands::SpeakerId {
            input,
            mapping,
            reference,
            anchor,
            inserted_role,
            confidence_threshold,
            write_match_report,
            write_override,
            write_pending,
            override_file,
            session_id,
            judgment,
            output,
        } => crate::commands::speaker_id::run_speaker_id(
            crate::commands::speaker_id::SpeakerIdArgs {
                input: &input,
                mapping_spec: mapping.as_deref(),
                reference: reference.as_deref(),
                anchor: anchor.as_deref(),
                inserted_role: inserted_role.as_deref(),
                confidence_threshold,
                write_match_report_path: write_match_report.as_deref(),
                write_override_path: write_override.as_deref(),
                write_pending_path: write_pending.as_deref(),
                override_file_path: override_file.as_deref(),
                session_id: session_id.as_deref(),
                output: output.as_ref(),
                judgment: judgment.judgment,
                llm_endpoint: judgment.llm_endpoint.as_deref(),
                llm_model: judgment.llm_model.as_deref(),
                llm_api_key: judgment.llm_api_key.as_deref(),
                llm_timeout_secs: judgment.llm_timeout_secs,
                llm_max_retries: judgment.llm_max_retries,
                llm_cache_path: judgment.llm_cache.as_deref(),
                session_context_path: judgment.session_context.as_deref(),
            },
        ),
        Commands::Rediarize {
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
        ),
        Commands::Adjudicate {
            pending,
            override_file,
            scripted,
            interactive,
            operator,
        } => crate::commands::adjudicate::run_adjudicate(
            &pending,
            &override_file,
            scripted.as_deref(),
            interactive,
            operator.as_deref(),
        ),
        Commands::SanityScan {
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
        ),
        Commands::Normalize {
            input,
            output,
            checks: NormalizeCheckArgs(level),
        } => normalize_chat(&input, output.as_ref(), level),
        Commands::ToJson {
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
            .unwrap_or_else(|usage| {
                eprintln!("Error: {usage}");
                std::process::exit(2);
            });
            match target {
                super::json::ToJsonTarget::File { output } => {
                    chat_to_json(&input, output.as_ref(), layout, checks)
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
                ),
            }
        }
        Commands::FromJson { input, output } => json_to_chat(&input, output.as_ref()),
        Commands::Clean {
            path,
            diff_only,
            format,
        } => clean_file(&path, diff_only, format),
        Commands::Fix {
            paths,
            mode: Flag(mode),
            codes,
            alignment: Flag(alignment),
        } => {
            if run_fix(&paths, mode, &codes, alignment).failed() {
                std::process::exit(1);
            }
        }
        Commands::NewFile {
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
        ),
        Commands::Schema { url } => run_schema(url),
        Commands::Update => run_update(),
        // Cache maintenance and debug tools.
        Commands::Cache { command } => run_cache_command(command),
        Commands::Debug { command } => run_debug(command),
    }
}

fn run_debug(command: crate::cli::DebugCommands) {
    use crate::cli::DebugCommands;
    match command {
        DebugCommands::RetagLanguage { from, to, path } => {
            super::debug::run_retag_language(&path, &from, &to);
        }
        DebugCommands::FixS { path } => {
            super::debug::run_fix_s(&path);
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
            super::debug::run_join_retrace(&path, commit, transform_scope);
        }
        DebugCommands::OverlapAudit { path, database } => {
            super::debug::run_overlap_audit(&path, database.as_deref());
        }
        DebugCommands::LinkerAudit { path, anomalies } => {
            super::debug::run_linker_audit(&path, anomalies.as_deref());
        }
        DebugCommands::Sanitize { input, output } => {
            super::debug::run_sanitize(&input, output.as_deref());
        }
    }
}
