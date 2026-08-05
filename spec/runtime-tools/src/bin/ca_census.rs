//! `ca_census`: a per-mark attestation census for Conversation Analysis notation.
//!
//! CA is the one region of CHAT that chatter has never specified. The rules it
//! does have were each added because something broke, so what exists is a
//! record of what the grammar happened to accept rather than a statement of
//! what the notation MEANS. Writing that specification needs an inventory
//! first: for every CA mark, where does it actually occur, and how regular is
//! that usage?
//!
//! This tool answers exactly that, from the typed CHAT AST. For each mark it
//! records, per occurrence:
//!
//! - the ROLE it was parsed into (top-level separator, top-level overlap
//!   point, word-interior element, word-interior paired delimiter),
//! - its SLOT (first, medial, last, or sole item of its container),
//! - its GLUE (whether non-whitespace source bytes abut it on each side),
//! - the KIND of its stream neighbours,
//! - whether the file declares `@Options: CA`,
//! - which corpus the file belongs to.
//!
//! The aggregate then reports, per mark, the share of occurrences in its single
//! most common (slot, glue) shape. That number is the answer to "do CA
//! transcribers do anything?": a mark at 99% is regular notation with a typo
//! tail, and a mark at 40% is genuinely unconstrained. The claim is testable,
//! and this is the test.
//!
//! Adjacency is read from the source bytes immediately outside each mark's own
//! span. That is whitespace detection at a known byte offset, the same question
//! the validator's span-adjacency rules ask; all CHAT MEANING here comes from
//! the typed AST, never from scanning text.
//!
//! Usage:
//!   cargo run --release --manifest-path spec/runtime-tools/Cargo.toml \
//!     --bin ca_census -- \
//!     --file-list /path/to/candidates.txt \
//!     --data-root ~/0tb/data \
//!     --out /path/to/ca-census.json
//!
//! The file list is a preselection of candidate paths (locating files with a
//! symbol search is fine; a file with no CA mark contributes nothing to the
//! census, so preselection cannot bias the counts).

use anyhow::{Context, Result};
use clap::Parser as ClapParser;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use talkbank_model::alignment::helpers::{ContentItem, walk_content};
use talkbank_model::model::{
    CADelimiter, CAElement, ChatOptionFlag, Header, OverlapPoint, OverlapPointKind, Separator,
    Word, WordContent,
};
use talkbank_parser::TreeSitterParser;

#[derive(ClapParser)]
#[command(name = "ca_census")]
#[command(about = "Per-mark attestation census for CA notation, from the typed CHAT AST")]
struct Args {
    /// Newline-separated candidate `.cha` paths.
    #[arg(long)]
    file_list: PathBuf,

    /// Corpus root; the first path component below it labels the corpus.
    #[arg(long)]
    data_root: PathBuf,

    /// Where the JSON census is written.
    #[arg(long)]
    out: PathBuf,

    /// Stop after this many files (for smoke runs).
    #[arg(long)]
    limit: Option<usize>,
}

/// Which grammatical position the parser assigned the mark to.
#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Role {
    /// A `Separator` item in the main-tier content stream.
    TopLevelSeparator,
    /// An `OverlapPoint` item in the main-tier content stream.
    TopLevelOverlapPoint,
    /// A single-position CA element inside a word (`↑`, `∙`).
    WordElement,
    /// One half of a paired CA span delimiter inside a word (`°`, `☺`).
    WordDelimiter,
}

impl Role {
    /// Stable label for the JSON key and the stdout summary.
    fn label(self) -> &'static str {
        match self {
            Role::TopLevelSeparator => "top_level_separator",
            Role::TopLevelOverlapPoint => "top_level_overlap_point",
            Role::WordElement => "word_element",
            Role::WordDelimiter => "word_delimiter",
        }
    }
}

/// Where the mark sat among its container's items.
#[derive(Clone, Copy)]
enum Slot {
    /// The container held nothing else.
    Only,
    First,
    Medial,
    Last,
}

impl Slot {
    /// Derive the slot from an index and a container length.
    fn of(index: usize, len: usize) -> Self {
        match (index, len) {
            (_, 0 | 1) => Slot::Only,
            (0, _) => Slot::First,
            (i, l) if i + 1 == l => Slot::Last,
            _ => Slot::Medial,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Slot::Only => "only",
            Slot::First => "first",
            Slot::Medial => "medial",
            Slot::Last => "last",
        }
    }
}

/// Whether non-whitespace source bytes abut the mark on each side.
#[derive(Clone, Copy)]
enum Glue {
    /// Whitespace or a line boundary on both sides: the canonical shape.
    Free,
    LeftGlued,
    RightGlued,
    BothGlued,
    /// The mark carried no real span, so adjacency is unknowable (the re2c
    /// front end fills dummy spans; so does direct model construction).
    Unknown,
}

impl Glue {
    fn label(self) -> &'static str {
        match self {
            Glue::Free => "free",
            Glue::LeftGlued => "left_glued",
            Glue::RightGlued => "right_glued",
            Glue::BothGlued => "both_glued",
            Glue::Unknown => "unknown",
        }
    }
}

/// One mark's accumulated evidence.
#[derive(Default, Serialize)]
struct MarkStats {
    /// Role label; a mark that somehow appears in two roles keeps the first.
    role: String,
    /// The mark's own symbol, for readers.
    symbol: String,
    total: u64,
    /// Occurrences in files that declare `@Options: CA`, and those that do not.
    ca_declared: u64,
    ca_undeclared: u64,
    files: u64,
    corpora: BTreeMap<String, u64>,
    slot: BTreeMap<String, u64>,
    glue: BTreeMap<String, u64>,
    /// Kind of the stream neighbour when glued on that side.
    glued_left_neighbour: BTreeMap<String, u64>,
    glued_right_neighbour: BTreeMap<String, u64>,
    /// Share of occurrences in the single most common `slot|glue` shape, and
    /// the shape itself. This is the regularity measure.
    dominant_shape: String,
    dominant_share: f64,
    /// Every distinct `slot|glue` shape with its count, most common first is
    /// not guaranteed by the map; read `dominant_shape` for the headline.
    shapes: BTreeMap<String, u64>,
    #[serde(skip)]
    file_ids: BTreeSet<usize>,
}

impl MarkStats {
    /// Fold one occurrence in.
    #[allow(clippy::too_many_arguments)]
    fn observe(
        &mut self,
        role: Role,
        symbol: &str,
        slot: Slot,
        glue: Glue,
        left: Option<&'static str>,
        right: Option<&'static str>,
        file: &FileFacts,
    ) {
        if self.role.is_empty() {
            self.role = role.label().to_owned();
            self.symbol = symbol.to_owned();
        }
        self.total += 1;
        if file.ca_declared {
            self.ca_declared += 1;
        } else {
            self.ca_undeclared += 1;
        }
        self.file_ids.insert(file.id);
        *self.corpora.entry(file.corpus.clone()).or_default() += 1;
        *self.slot.entry(slot.label().to_owned()).or_default() += 1;
        *self.glue.entry(glue.label().to_owned()).or_default() += 1;
        *self
            .shapes
            .entry(format!("{}|{}", slot.label(), glue.label()))
            .or_default() += 1;
        if matches!(glue, Glue::LeftGlued | Glue::BothGlued)
            && let Some(kind) = left
        {
            *self
                .glued_left_neighbour
                .entry(kind.to_owned())
                .or_default() += 1;
        }
        if matches!(glue, Glue::RightGlued | Glue::BothGlued)
            && let Some(kind) = right
        {
            *self
                .glued_right_neighbour
                .entry(kind.to_owned())
                .or_default() += 1;
        }
    }

    /// Compute the derived fields once accumulation is done.
    fn finish(&mut self) {
        self.files = self.file_ids.len() as u64;
        if let Some((shape, count)) = self.shapes.iter().max_by_key(|(_, count)| **count) {
            self.dominant_shape = shape.clone();
            self.dominant_share = if self.total == 0 {
                0.0
            } else {
                *count as f64 / self.total as f64
            };
        }
    }
}

/// Per-file facts every occurrence is tagged with.
struct FileFacts {
    id: usize,
    corpus: String,
    ca_declared: bool,
}

/// The whole census.
#[derive(Serialize)]
struct CaCensus {
    files_listed: usize,
    files_parsed: usize,
    files_unparsable: usize,
    files_with_marks: usize,
    /// Files carrying a CA mark but NOT declaring `@Options: CA`.
    files_with_marks_without_ca_option: usize,
    marks: BTreeMap<String, MarkStats>,
}

/// Stable name and symbol for a top-level separator variant.
///
/// Written out rather than derived, so the compiler forces a decision when a
/// variant is added: a census that silently skipped a new mark would be worse
/// than one that fails to build.
fn separator_identity(separator: &Separator) -> (&'static str, &'static str) {
    match separator {
        Separator::Comma { .. } => ("comma", ","),
        Separator::Semicolon { .. } => ("semicolon", ";"),
        Separator::Colon { .. } => ("colon", ":"),
        Separator::Tag { .. } => ("tag", "\u{201E}"),
        Separator::Vocative { .. } => ("vocative", "\u{2021}"),
        Separator::CaContinuation { .. } => ("ca_continuation", "[^c]"),
        Separator::UnmarkedEnding { .. } => ("unmarked_ending", "\u{221E}"),
        Separator::Uptake { .. } => ("uptake", "\u{2261}"),
        Separator::CaNoBreak { .. } => ("ca_no_break", "\u{2248}"),
        Separator::CaTechnicalBreak { .. } => ("ca_technical_break", "\u{224B}"),
        Separator::RisingToHigh { .. } => ("rising_to_high", "\u{21D7}"),
        Separator::RisingToMid { .. } => ("rising_to_mid", "\u{2197}"),
        Separator::Level { .. } => ("level", "\u{2192}"),
        Separator::FallingToMid { .. } => ("falling_to_mid", "\u{2198}"),
        Separator::FallingToLow { .. } => ("falling_to_low", "\u{21D8}"),
    }
}

/// Stable name and symbol for an overlap marker.
fn overlap_identity(kind: OverlapPointKind) -> (&'static str, &'static str) {
    match kind {
        OverlapPointKind::TopOverlapBegin => ("top_overlap_begin", "\u{2308}"),
        OverlapPointKind::TopOverlapEnd => ("top_overlap_end", "\u{2309}"),
        OverlapPointKind::BottomOverlapBegin => ("bottom_overlap_begin", "\u{230A}"),
        OverlapPointKind::BottomOverlapEnd => ("bottom_overlap_end", "\u{230B}"),
    }
}

/// A short kind label for a content item, used to describe neighbours.
fn item_kind(item: &ContentItem<'_>) -> &'static str {
    match item {
        ContentItem::Word(_) => "word",
        ContentItem::ReplacedWord(_) => "replaced_word",
        ContentItem::Separator(_) => "separator",
        ContentItem::Event(_) => "event",
        ContentItem::Pause(_) => "pause",
        ContentItem::Action(_) => "action",
        ContentItem::OverlapPoint(_) => "overlap_point",
        ContentItem::OtherSpokenEvent(_) => "other_spoken_event",
        ContentItem::Freecode(_) => "freecode",
        ContentItem::InternalBullet(_) => "internal_bullet",
        ContentItem::LongFeatureBegin(_) => "long_feature_begin",
        ContentItem::LongFeatureEnd(_) => "long_feature_end",
        ContentItem::UnderlineBegin(_) => "underline_begin",
        ContentItem::UnderlineEnd(_) => "underline_end",
        ContentItem::NonvocalBegin(_) => "nonvocal_begin",
        ContentItem::NonvocalEnd(_) => "nonvocal_end",
        ContentItem::NonvocalSimple(_) => "nonvocal_simple",
    }
}

/// Whether the byte at `offset` in `source` is CHAT whitespace or absent.
///
/// Absent counts as free: a mark at a line edge has no glued neighbour.
fn is_boundary(source: &str, offset: Option<usize>) -> bool {
    match offset {
        None => true,
        Some(offset) => match source.as_bytes().get(offset) {
            None => true,
            Some(byte) => byte.is_ascii_whitespace(),
        },
    }
}

/// Classify a mark's glue from the source bytes immediately outside its span.
fn glue_of(source: &str, span: Option<talkbank_model::Span>) -> Glue {
    let Some(span) = span.filter(|span| *span != talkbank_model::Span::DUMMY) else {
        return Glue::Unknown;
    };
    let start = span.start as usize;
    let end = span.end as usize;
    let left_free = is_boundary(source, start.checked_sub(1));
    let right_free = is_boundary(source, Some(end));
    match (left_free, right_free) {
        (true, true) => Glue::Free,
        (false, true) => Glue::LeftGlued,
        (true, false) => Glue::RightGlued,
        (false, false) => Glue::BothGlued,
    }
}

/// Record every word-interior CA mark in one word.
fn census_word(
    word: &Word,
    source: &str,
    file: &FileFacts,
    marks: &mut BTreeMap<String, MarkStats>,
) {
    let contents: Vec<&WordContent> = word.content.iter().collect();
    let len = contents.len();
    for (index, content) in contents.iter().enumerate() {
        let slot = Slot::of(index, len);
        match content {
            WordContent::CAElement(element) => {
                let CAElement {
                    element_type, span, ..
                } = element;
                let name = format!("{element_type:?}");
                let key = format!("element:{name}");
                marks.entry(key).or_default().observe(
                    Role::WordElement,
                    element_type.to_symbol(),
                    slot,
                    glue_of(source, *span),
                    None,
                    None,
                    file,
                );
            }
            WordContent::CADelimiter(delimiter) => {
                let CADelimiter {
                    delimiter_type,
                    span,
                    ..
                } = delimiter;
                let name = format!("{delimiter_type:?}");
                let key = format!("delimiter:{name}");
                marks.entry(key).or_default().observe(
                    Role::WordDelimiter,
                    delimiter_type.to_symbol(),
                    slot,
                    glue_of(source, *span),
                    None,
                    None,
                    file,
                );
            }
            _ => {}
        }
    }
}

/// Record every CA mark in one file. Returns whether any was found.
fn census_file(
    chat_file: &talkbank_model::model::ChatFile,
    source: &str,
    file: &FileFacts,
    marks: &mut BTreeMap<String, MarkStats>,
) -> bool {
    let mut found = false;

    for utterance in chat_file.utterances() {
        // In-order recursive traversal is the project's definition of
        // adjacency (design rule 4), so the neighbour of a mark inside a
        // group is the item that really precedes it in the source.
        let mut stream: Vec<(&'static str, Option<talkbank_model::Span>, Option<MarkRef>)> =
            Vec::new();
        walk_content(
            utterance.main.content.content.as_slice(),
            None,
            &mut |item| {
                let mark = match &item {
                    ContentItem::Separator(separator) => {
                        Some(MarkRef::Separator(separator.span(), {
                            let (name, symbol) = separator_identity(separator);
                            (name, symbol)
                        }))
                    }
                    ContentItem::OverlapPoint(point) => {
                        let OverlapPoint { kind, span, .. } = point;
                        Some(MarkRef::Overlap(*span, overlap_identity(*kind)))
                    }
                    _ => None,
                };
                if let ContentItem::Word(word) = &item {
                    census_word(word, source, file, marks);
                }
                let span = match &item {
                    ContentItem::Word(word) => Some(word.span),
                    ContentItem::Separator(separator) => Some(separator.span()),
                    ContentItem::Pause(pause) => Some(pause.span),
                    ContentItem::OverlapPoint(point) => point.span,
                    _ => None,
                };
                stream.push((item_kind(&item), span, mark));
            },
        );

        let len = stream.len();
        for index in 0..len {
            let Some(mark) = stream[index].2 else {
                continue;
            };
            let left = index.checked_sub(1).map(|i| stream[i].0);
            let right = stream.get(index + 1).map(|entry| entry.0);
            let slot = Slot::of(index, len);
            let (role, key, symbol, span) = match mark {
                MarkRef::Separator(span, (name, symbol)) => (
                    Role::TopLevelSeparator,
                    format!("separator:{name}"),
                    symbol,
                    Some(span),
                ),
                MarkRef::Overlap(span, (name, symbol)) => (
                    Role::TopLevelOverlapPoint,
                    format!("overlap:{name}"),
                    symbol,
                    span,
                ),
            };
            found = true;
            marks.entry(key).or_default().observe(
                role,
                symbol,
                slot,
                glue_of(source, span),
                left,
                right,
                file,
            );
        }
    }
    found
}

/// A CA mark spotted during the walk, carried until its neighbours are known.
#[derive(Clone, Copy)]
enum MarkRef {
    Separator(talkbank_model::Span, (&'static str, &'static str)),
    Overlap(Option<talkbank_model::Span>, (&'static str, &'static str)),
}

/// The corpus label for a path: the first component below `data_root`.
fn corpus_of(path: &Path, data_root: &Path) -> String {
    path.strip_prefix(data_root)
        .ok()
        .and_then(|rest| rest.components().next())
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn main() -> Result<()> {
    let args = Args::parse();
    let list = std::fs::read_to_string(&args.file_list)
        .with_context(|| format!("reading file list {}", args.file_list.display()))?;
    let paths: Vec<PathBuf> = list
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(PathBuf::from)
        .take(args.limit.unwrap_or(usize::MAX))
        .collect();

    let parser = TreeSitterParser::new().map_err(|error| anyhow::anyhow!("{error}"))?;
    let mut marks: BTreeMap<String, MarkStats> = BTreeMap::new();
    let mut files_parsed = 0usize;
    let mut files_unparsable = 0usize;
    let mut files_with_marks = 0usize;
    let mut files_with_marks_without_ca_option = 0usize;

    for (id, path) in paths.iter().enumerate() {
        let Ok(source) = std::fs::read_to_string(path) else {
            files_unparsable += 1;
            continue;
        };
        // CA-mode detection needs the parsed headers, so parse once here and
        // hand the source to the census; a file that will not build a model
        // at all is counted and skipped rather than guessed at. A file that
        // builds a model but also carries diagnostics is still usable here
        // (the census only reads headers), matching the stated intent
        // ("will not parse at all") more literally than the old
        // any-diagnostic-fails behavior did.
        let talkbank_parser::ParseProduct::Built { file: parsed, .. } =
            parser.parse_chat_file(&source)
        else {
            files_unparsable += 1;
            continue;
        };
        files_parsed += 1;
        if files_parsed.is_multiple_of(500) {
            eprintln!("  {files_parsed} files parsed...");
        }
        let ca_declared = parsed.headers().any(|header| match header {
            Header::Options { options } => options.iter().any(ChatOptionFlag::enables_ca_mode),
            _ => false,
        });
        let file = FileFacts {
            id,
            corpus: corpus_of(path, &args.data_root),
            ca_declared,
        };
        if census_file(&parsed, &source, &file, &mut marks) {
            files_with_marks += 1;
            if !ca_declared {
                files_with_marks_without_ca_option += 1;
            }
        }
    }

    for stats in marks.values_mut() {
        stats.finish();
    }

    let census = CaCensus {
        files_listed: paths.len(),
        files_parsed,
        files_unparsable,
        files_with_marks,
        files_with_marks_without_ca_option,
        marks,
    };

    let json = serde_json::to_string_pretty(&census)?;
    std::fs::write(&args.out, json)
        .with_context(|| format!("writing census to {}", args.out.display()))?;

    println!(
        "files: {} listed, {} parsed, {} unparsable, {} carry a CA mark ({} of those do NOT declare @Options: CA)",
        census.files_listed,
        census.files_parsed,
        census.files_unparsable,
        census.files_with_marks,
        census.files_with_marks_without_ca_option
    );
    println!();
    println!(
        "{:<38} {:>10} {:>8} {:>26} {:>7}",
        "mark", "total", "files", "dominant shape", "share"
    );
    let mut rows: Vec<(&String, &MarkStats)> = census.marks.iter().collect();
    // Descending by total, so the key is reversed rather than the comparison.
    rows.sort_by_key(|row| std::cmp::Reverse(row.1.total));
    for (key, stats) in rows {
        println!(
            "{:<38} {:>10} {:>8} {:>26} {:>6.1}%",
            format!("{key} {}", stats.symbol),
            stats.total,
            stats.files,
            stats.dominant_shape,
            stats.dominant_share * 100.0
        );
    }
    println!();
    println!("full census written to {}", args.out.display());
    Ok(())
}
