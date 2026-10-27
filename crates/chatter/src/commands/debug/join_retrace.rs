//! `chatter experimental transform join-retrace`, OBVIOUS dangling-retrace (E370) auto-join.

use std::path::PathBuf;

use talkbank_transform::join_retrace::{
    JoinRetraceStats, RetraceJoinScope, join_dangling_retraces,
};

use super::*;

/// Join dangling-retrace utterances with their same-speaker successor.
///
/// Implements `chatter experimental transform join-retrace`. For each qualifying file, the
/// repair joins dangling retrace utterances according to the provided `scope`
/// (see [`join_dangling_retraces`] for the exact rules). With
/// [`Commit::DryRun`], files are parsed and analyzed but never written; the
/// would-be changes are reported.
///
/// When either joined side carried dependent tiers, those tiers are dropped on
/// the joined utterance and counted as needing re-morphotag, so the operator
/// knows which files must be re-run through morphotagging afterwards.
pub fn run_join_retrace(
    paths: &[PathBuf],
    commit: Commit,
    scope: RetraceJoinScope,
) -> crate::exit_codes::CommandResult {
    let (files, parser) = super::transcripts_and_parser(paths)?;

    let mut changed_files = 0usize;
    let mut refused_files = 0usize;
    let mut totals = JoinRetraceStats::default();

    for path in files {
        let Some(mut open) = InPlace::open(&parser, path) else {
            refused_files += 1;
            continue;
        };

        let stats = join_dangling_retraces(open.model_mut(), scope);
        if stats.is_empty() {
            continue;
        }

        let display = open.path().display().to_string();
        // The dry run and the real write share ONE change detection, so
        // preview mode cannot report a file the real run would leave alone.
        let announced = match open.commit(commit) {
            Ok(Committed::Wrote) => format!(
                "{display}: joined {} utterance(s){}",
                stats.joined_utterances,
                remorphotag_suffix(&stats)
            ),
            Ok(Committed::WouldWrite) => format!(
                "[dry-run] {display}: would join {} utterance(s){}",
                stats.joined_utterances,
                remorphotag_suffix(&stats)
            ),
            Ok(Committed::Unchanged) => continue,
            Err(error) => {
                refused_files += 1;
                eprintln!("ERROR: cannot publish {display}: {error}");
                continue;
            }
        };
        outln!("{announced}");

        changed_files += 1;
        totals.joined_utterances += stats.joined_utterances;
        totals.needs_remorphotag += stats.needs_remorphotag;
        totals.dependent_tiers_dropped += stats.dependent_tiers_dropped;
    }

    if changed_files == 0 && refused_files == 0 {
        outln!("No OBVIOUS dangling-retrace (E370) joins needed.");
        return Ok(());
    }

    let verb = match commit {
        Commit::DryRun => "Would join",
        Commit::Write => "Joined",
    };
    outln!(
        "{verb} {} utterance(s) across {changed_files} file(s); {} joined utterance(s) had dependent tiers dropped and need re-morphotag ({} tier(s) dropped total).",
        totals.joined_utterances,
        totals.needs_remorphotag,
        totals.dependent_tiers_dropped
    );
    if refused_files > 0 {
        eprintln!("Refused {refused_files} file(s); those files are unchanged.");
        return Err(crate::exit_codes::ReportedFailure::Input.into());
    }
    Ok(())
}

/// Render the per-file re-morphotag note for the join report.
fn remorphotag_suffix(stats: &JoinRetraceStats) -> String {
    if stats.needs_remorphotag == 0 {
        String::new()
    } else {
        format!(
            ", {} dropped dependent tiers (needs re-morphotag)",
            stats.needs_remorphotag
        )
    }
}
