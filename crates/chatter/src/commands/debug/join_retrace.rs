//! `chatter debug join-retrace`, OBVIOUS dangling-retrace (E370) auto-join.

use std::path::PathBuf;

use talkbank_transform::join_retrace::{
    JoinRetraceStats, RetraceJoinScope, join_dangling_retraces,
};

use super::*;

/// Join dangling-retrace utterances with their same-speaker successor.
///
/// Implements `chatter debug join-retrace`. For each qualifying file, the
/// repair joins dangling retrace utterances according to the provided `scope`
/// (see [`join_dangling_retraces`] for the exact rules). With `dry_run`, files
/// are parsed and analyzed but never written; the would-be changes are
/// reported.
///
/// When either joined side carried dependent tiers, those tiers are dropped on
/// the joined utterance and counted as needing re-morphotag, so the operator
/// knows which files must be re-run through morphotagging afterwards.
pub fn run_join_retrace(paths: &[PathBuf], dry_run: bool, scope: RetraceJoinScope) {
    let files = collect_cha_files(paths);
    if files.is_empty() {
        die("no .cha files found in the provided paths");
    }

    let parser = talkbank_parser::TreeSitterParser::new()
        .unwrap_or_else(|e| die(&format!("parser initialization failed: {e:?}")));

    let mut changed_files = 0usize;
    let mut totals = JoinRetraceStats::default();

    for path in files {
        let Some(mut open) = InPlace::open(&parser, path) else {
            continue;
        };

        let stats = join_dangling_retraces(open.model_mut(), scope);
        if stats.is_empty() {
            continue;
        }

        let display = open.path().display().to_string();
        let mode = if dry_run {
            Commit::DryRun
        } else {
            Commit::Write
        };
        // The dry run and the real write share ONE change detection, so
        // `--dry-run` cannot report a file the real run would leave alone.
        let announced = match open.commit(mode) {
            Committed::Wrote => format!(
                "{display}: joined {} utterance(s){}",
                stats.joined_utterances,
                remorphotag_suffix(&stats)
            ),
            Committed::WouldWrite => format!(
                "[dry-run] {display}: would join {} utterance(s){}",
                stats.joined_utterances,
                remorphotag_suffix(&stats)
            ),
            Committed::Unchanged => continue,
        };
        println!("{announced}");

        changed_files += 1;
        totals.joined_utterances += stats.joined_utterances;
        totals.needs_remorphotag += stats.needs_remorphotag;
        totals.dependent_tiers_dropped += stats.dependent_tiers_dropped;
    }

    if changed_files == 0 {
        println!("No OBVIOUS dangling-retrace (E370) joins needed.");
        return;
    }

    let verb = if dry_run { "Would join" } else { "Joined" };
    println!(
        "{verb} {} utterance(s) across {changed_files} file(s); {} joined utterance(s) had dependent tiers dropped and need re-morphotag ({} tier(s) dropped total).",
        totals.joined_utterances, totals.needs_remorphotag, totals.dependent_tiers_dropped
    );
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
