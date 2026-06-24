//! `chatter debug join-retrace`, OBVIOUS dangling-retrace (E370) auto-join.

use std::path::PathBuf;

use talkbank_model::WriteChat;
use talkbank_transform::join_retrace::{JoinRetraceStats, RetraceJoinScope, join_dangling_retraces};

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
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| die(&format!("cannot read {}: {e}", path.display())));
        let mut parsed = parser
            .parse_chat_file(&source)
            .unwrap_or_else(|e| die(&format!("parse failed for {}: {e:?}", path.display())));

        let stats = join_dangling_retraces(&mut parsed, scope);
        if stats.is_empty() {
            continue;
        }

        let rewritten = parsed.to_chat_string();
        if rewritten == source {
            continue;
        }

        if dry_run {
            println!(
                "[dry-run] {}: would join {} utterance(s){}",
                path.display(),
                stats.joined_utterances,
                remorphotag_suffix(&stats)
            );
        } else {
            std::fs::write(&path, &rewritten)
                .unwrap_or_else(|e| die(&format!("cannot write {}: {e}", path.display())));
            println!(
                "{}: joined {} utterance(s){}",
                path.display(),
                stats.joined_utterances,
                remorphotag_suffix(&stats)
            );
        }

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
