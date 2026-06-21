//! `chatter debug overlap`, CA overlap audit.

use std::io::Write;
use std::path::{Path, PathBuf};

use talkbank_model::alignment::helpers::overlap_groups::analyze_file_overlaps;
use talkbank_model::errors::NullErrorSink;

use super::*;

/// Run the overlap-audit debug command.
pub fn run_overlap_audit(paths: &[PathBuf], database_path: Option<&Path>) {
    let mut db_writer: Option<std::io::BufWriter<std::fs::File>> = database_path.map(|p| {
        let file = std::fs::File::create(p).unwrap_or_else(|e| {
            eprintln!("ERROR: cannot create database file {}: {e}", p.display());
            std::process::exit(1);
        });
        std::io::BufWriter::new(file)
    });

    let mut files_processed = 0;
    let mut total_groups = 0;
    let mut total_bottoms = 0;
    let mut total_orphaned_tops = 0;
    let mut total_orphaned_bottoms = 0;
    let mut total_timed = 0;
    let mut total_consistent = 0;
    let mut files_with_overlaps = 0;

    println!(
        "file\tutterances\tgroups\tbottoms\torphan_tops\torphan_bottoms\ttimed\tconsistent\tquality"
    );

    // The tree-sitter CHAT grammar is compiled into the binary and
    // structurally validated by CI, so construction is infallible
    // in practice (matches the pattern used in
    // `crates/talkbank-cli/src/lib.rs` chat_parser()).
    #[allow(clippy::expect_used)]
    let parser = talkbank_parser::TreeSitterParser::new().expect("grammar loads");

    for path in collect_cha_files(paths) {
        let source = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("ERROR: cannot read {}: {e}", path.display());
                continue;
            }
        };

        let chat = parser.parse_chat_file_streaming(&source, &NullErrorSink);
        let analysis = analyze_file_overlaps(&chat.lines);

        let utt_count = chat
            .lines
            .iter()
            .filter(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .count();

        if !analysis.has_overlaps() {
            continue;
        }

        files_with_overlaps += 1;

        // Temporal consistency
        let mut timed = 0;
        let mut consistent = 0;
        for group in &analysis.groups {
            for bottom in &group.bottoms {
                if let (Some(top_b), Some(bot_b)) = (group.top.bullet, bottom.bullet) {
                    timed += 1;
                    let tolerance: u64 = 2000;
                    if bot_b.0 <= top_b.1 + tolerance && bot_b.0 + tolerance >= top_b.0 {
                        consistent += 1;
                    }
                }
            }
        }

        let quality = if analysis.orphaned_tops.is_empty() && analysis.orphaned_bottoms.is_empty() {
            "fully_paired"
        } else {
            let total = analysis.groups.len()
                + analysis.orphaned_tops.len()
                + analysis.orphaned_bottoms.len();
            let orphan_frac = (analysis.orphaned_tops.len() + analysis.orphaned_bottoms.len())
                as f64
                / total as f64;
            if orphan_frac > 0.8 {
                "open_only"
            } else {
                "mixed"
            }
        };

        let filename = path.file_stem().unwrap_or_default().to_string_lossy();
        println!(
            "{filename}\t{utt_count}\t{}\t{}\t{}\t{}\t{timed}\t{consistent}\t{quality}",
            analysis.groups.len(),
            analysis.total_bottoms(),
            analysis.orphaned_tops.len(),
            analysis.orphaned_bottoms.len(),
        );

        // Write JSON lines record if database output requested
        if let Some(ref mut db_file) = db_writer {
            let record = serde_json::json!({
                "file": path.to_string_lossy(),
                "filename": filename,
                "utterances": utt_count,
                "groups": analysis.groups.len(),
                "bottoms": analysis.total_bottoms(),
                "orphaned_tops": analysis.orphaned_tops.len(),
                "orphaned_bottoms": analysis.orphaned_bottoms.len(),
                "timed_pairs": timed,
                "temporally_consistent": consistent,
                "quality": quality,
            });
            writeln!(db_file, "{}", record).unwrap_or_else(|e| {
                eprintln!("ERROR: failed to write database record: {e}");
            });
        }

        files_processed += 1;
        total_groups += analysis.groups.len();
        total_bottoms += analysis.total_bottoms();
        total_orphaned_tops += analysis.orphaned_tops.len();
        total_orphaned_bottoms += analysis.orphaned_bottoms.len();
        total_timed += timed;
        total_consistent += consistent;
    }

    eprintln!();
    eprintln!("=== Summary ===");
    eprintln!("  Files processed:    {files_processed}");
    eprintln!("  Files with overlaps: {files_with_overlaps}");
    eprintln!("  Total groups:       {total_groups}");
    eprintln!("  Total bottoms:      {total_bottoms}");
    eprintln!("  Orphaned tops:      {total_orphaned_tops}");
    eprintln!("  Orphaned bottoms:   {total_orphaned_bottoms}");
    if total_timed > 0 {
        let pct = total_consistent as f64 / total_timed as f64 * 100.0;
        eprintln!("  Timed pairs:        {total_timed}");
        eprintln!("  Temporally ok:      {total_consistent} ({pct:.0}%)");
    }
    if let Some(path) = database_path {
        eprintln!("  Database written:   {}", path.display());
    }
}
