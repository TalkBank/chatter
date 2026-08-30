//! Re-attribute utterance speakers from an external diarization.
//!
//! Given a parsed [`ChatFile`] whose utterances carry media time bullets
//! (`\u{15}start_end\u{15}`) and a set of timestamped [`DiarizationTurn`]s
//! produced by an external diarizer (e.g. pyannote), reassign each
//! utterance's main-tier speaker to the diarization track that covers its
//! time span the most. This repairs transcripts whose word content is
//! correct but whose speaker attribution came from a weaker diarizer
//! (e.g. a bundled ASR that under-counts or mixes speakers).
//!
//! The diarizer is a pure DATA boundary: turns arrive as
//! [`DiarizationTurn`] values (a track code plus a [`TimeSpanMs`]), admitted
//! once to a start-ordered [`DiarizationTimeline`]; this module never touches
//! audio. It operates entirely on the typed CHAT model and re-serializes
//! through the model, never string-assembling CHAT.
//!
//! Design contract:
//! - An utterance with no bullet, or whose bullet overlaps NO turn, is
//!   left byte-stable and returned in [`RediarizeOutcome::flagged`] with
//!   the reason. Ambiguity is surfaced, never silently guessed.
//! - `@Participants` / `@ID` headers are reconciled to exactly the set of
//!   tracks that end up used, so the output is self-consistent CHAT ready
//!   for the downstream `speaker-id` / `merge` pipeline, which assigns the
//!   real roles. This module only fixes WHICH anonymous track owns each
//!   utterance; it does not assign roles.

use std::collections::HashSet;

use talkbank_model::ParseValidateOptions;
use talkbank_model::model::header::{Header, ParticipantEntries, ParticipantEntry};
use talkbank_model::model::{ChatFile, Line, SpeakerCode, TierSeparator};

use crate::PipelineError;
use crate::pipeline::parse_and_validate;
use crate::serialize::to_chat_string;

/// A half-open media time span in milliseconds (`[start_ms, end_ms)`).
///
/// Shared by diarization turns and utterance bullets so overlap is a
/// single typed operation rather than ad hoc integer juggling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeSpanMs {
    start_ms: u64,
    end_ms: u64,
}

/// Constructing a [`TimeSpanMs`] with `end_ms < start_ms` is a caller bug
/// in the diarization input, not a recoverable runtime state.
#[derive(Debug, thiserror::Error)]
#[error("time span end_ms ({end_ms}) precedes start_ms ({start_ms})")]
pub struct InvertedSpan {
    start_ms: u64,
    end_ms: u64,
}

impl TimeSpanMs {
    /// Build a span, rejecting an inverted `[start, end)`.
    pub fn new(start_ms: u64, end_ms: u64) -> Result<Self, InvertedSpan> {
        if end_ms < start_ms {
            return Err(InvertedSpan { start_ms, end_ms });
        }
        Ok(Self { start_ms, end_ms })
    }

    /// Where the span starts, in media milliseconds.
    ///
    /// # Why these exist
    ///
    /// The fields are private so that [`Self::new`] is the only route in and
    /// an inverted span cannot be built. That is right, but it left the type
    /// WRITE-ONLY through the public API: `DiarizationTurn::span` is a public
    /// field of a type a caller could hold and not read, so a downstream
    /// consumer of `parse_turns_json` had to re-declare the same concept to
    /// get the numbers back out. Reading cannot invert anything, so the
    /// invariant is untouched.
    #[must_use]
    pub fn start_ms(&self) -> u64 {
        self.start_ms
    }

    /// Where the span ends, in media milliseconds. See [`Self::start_ms`].
    #[must_use]
    pub fn end_ms(&self) -> u64 {
        self.end_ms
    }

    /// Milliseconds of overlap between the two spans (0 if disjoint).
    pub fn overlap_ms(&self, other: &Self) -> u64 {
        match self.intersection(other) {
            Some(span) => span.duration_ms(),
            None => 0,
        }
    }

    /// Their positive intersection, with emptiness removed from the type.
    fn intersection(&self, other: &Self) -> Option<NonEmptyTimeSpanMs> {
        NonEmptyTimeSpanMs::new(
            self.start_ms.max(other.start_ms),
            self.end_ms.min(other.end_ms),
        )
    }
}

/// A time span proven to contain positive media time.
///
/// This is private algorithm evidence: an ordinary [`TimeSpanMs`] permits an
/// empty half-open interval, while an interval admitted here can safely seed a
/// track's union-held duration.
#[derive(Debug, Clone, Copy)]
struct NonEmptyTimeSpanMs {
    start_ms: u64,
    end_ms: u64,
}

impl NonEmptyTimeSpanMs {
    fn new(start_ms: u64, end_ms: u64) -> Option<Self> {
        (start_ms < end_ms).then_some(Self { start_ms, end_ms })
    }

    fn duration_ms(self) -> u64 {
        // Construction proves the subtraction cannot underflow.
        self.end_ms - self.start_ms
    }
}

/// One timestamped diarization segment: an anonymous track code and the
/// span it speaks. The track code is the diarizer's own label (e.g.
/// `PAR0`, `PAR1`, `PAR2`); role assignment is a downstream concern.
#[derive(Debug, Clone)]
pub struct DiarizationTurn {
    /// The diarizer's anonymous track label for this segment.
    pub track: SpeakerCode,
    /// The media time span this track speaks.
    pub span: TimeSpanMs,
}

/// Diarization turns admitted to the time-ordered transform boundary.
///
/// The overlap-window algorithm relies on start-time ordering. Keeping an
/// arbitrary `&[DiarizationTurn]` in the public transform API made a wrong
/// answer representable for every library caller that did not happen to pass
/// through [`parse_turns_json`]. This type owns the one sorting transition and
/// keeps the ordered storage private.
#[derive(Debug, Clone)]
pub struct DiarizationTimeline {
    turns: Vec<DiarizationTurn>,
    longest_turn_ms: u64,
}

impl DiarizationTimeline {
    /// Admit turns to the transform timeline, ordering them once by start.
    #[must_use]
    pub fn new(mut turns: Vec<DiarizationTurn>) -> Self {
        turns.sort_by_key(|turn| turn.span.start_ms());
        let longest_turn_ms = turns
            .iter()
            .map(|turn| turn.span.end_ms().saturating_sub(turn.span.start_ms()))
            .max()
            .unwrap_or(0);
        Self {
            turns,
            longest_turn_ms,
        }
    }

    /// The admitted turns in nondecreasing start-time order.
    #[must_use]
    pub fn turns(&self) -> &[DiarizationTurn] {
        &self.turns
    }
}

/// Why an utterance was left unchanged instead of reassigned.
///
/// Serializes as `"no_bullet"` / `"no_overlapping_turn"` in the
/// machine-readable summary (see [`RediarizeSummary`]); those strings
/// are a stable output contract for batch consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FlagReason {
    /// The utterance carries no media time bullet, so it cannot be placed
    /// on the diarization timeline.
    NoBullet,
    /// The utterance has a bullet but no diarization turn overlaps it.
    NoOverlappingTurn,
}

impl std::fmt::Display for FlagReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBullet => write!(f, "no time bullet"),
            Self::NoOverlappingTurn => write!(f, "no overlapping diarization turn"),
        }
    }
}

/// Free-form provenance a turns file carries about its producer,
/// typically the diarizer model name (e.g.
/// `pyannote/speaker-diarization-community-1`). Reported in audit
/// trails; never interpreted by the transform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiarizationSource(String);

impl DiarizationSource {
    /// The provenance string as given by the producer.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DiarizationSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The parsed, validated content of a turns JSON file: the documented
/// data seam between an external diarizer and this transform. Format
/// contract: `book/src/chatter/user-guide/rediarize.md`.
#[derive(Debug)]
pub struct TurnsFile {
    source: Option<DiarizationSource>,
    timeline: DiarizationTimeline,
}

impl TurnsFile {
    /// Optional producer provenance (`"source"` in the JSON).
    #[must_use]
    pub fn source(&self) -> Option<&DiarizationSource> {
        self.source.as_ref()
    }

    /// The validated, ordered diarization timeline.
    #[must_use]
    pub fn timeline(&self) -> &DiarizationTimeline {
        &self.timeline
    }
}

/// Why a turns JSON file was rejected. `Json` is malformed input
/// (not JSON, wrong shape, unknown fields); `InvertedTurn` is
/// well-formed JSON whose data is semantically defective.
#[derive(Debug, thiserror::Error)]
pub enum TurnsJsonError {
    /// The text is not valid JSON or does not match the documented
    /// shape (including unknown fields, which are rejected so typos
    /// fail loudly).
    #[error("turns JSON is malformed: {0}")]
    Json(#[from] serde_json::Error),
    /// A turn's span is inverted (`end_ms < start_ms`): defective
    /// diarizer output the caller must fix at the source.
    #[error("turn at index {index}: {source}")]
    InvertedTurn {
        /// 0-based index of the offending turn in the `turns` array.
        index: usize,
        /// The underlying span inversion.
        source: InvertedSpan,
    },
}

/// Raw serde mirror of the turns JSON. Unknown fields are rejected:
/// a misspelled `start_ms` must fail the parse, not silently drop
/// timing data.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTurnsFile {
    #[serde(default)]
    source: Option<String>,
    turns: Vec<RawTurn>,
}

/// One raw turn entry as it appears in the JSON.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTurn {
    track: String,
    start_ms: u64,
    end_ms: u64,
}

/// Parse the documented turns JSON into validated [`DiarizationTurn`]s.
/// Rejects malformed JSON, unknown fields, and inverted spans; track
/// codes are taken as given (the producer owns the diarizer-label to
/// CHAT-code mapping).
pub fn parse_turns_json(text: &str) -> Result<TurnsFile, TurnsJsonError> {
    let raw: RawTurnsFile = serde_json::from_str(text)?;
    let mut turns = Vec::with_capacity(raw.turns.len());
    for (index, turn) in raw.turns.into_iter().enumerate() {
        let span = TimeSpanMs::new(turn.start_ms, turn.end_ms)
            .map_err(|source| TurnsJsonError::InvertedTurn { index, source })?;
        turns.push(DiarizationTurn {
            track: SpeakerCode::new(&turn.track),
            span,
        });
    }
    Ok(TurnsFile {
        source: raw.source.map(DiarizationSource),
        timeline: DiarizationTimeline::new(turns),
    })
}

/// An utterance the transform could not confidently reattribute. Carries
/// the 0-based main-tier position and the speaker it kept, so the caller
/// can review or route it to human adjudication.
///
/// The serde field names (`utterance_index`, `kept_speaker`, `reason`)
/// are the machine-readable summary contract (see [`RediarizeSummary`]);
/// renaming a field is a breaking change to that contract.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FlaggedUtterance {
    /// 0-based position of the utterance among main-tier lines.
    pub utterance_index: usize,
    /// The speaker the utterance kept (unchanged) because it could not be
    /// confidently reattributed.
    pub kept_speaker: SpeakerCode,
    /// Why the reattribution was declined.
    pub reason: FlagReason,
}

/// Summary of a [`rediarize`] pass.
#[derive(Debug, Clone, Default)]
pub struct RediarizeOutcome {
    /// Utterances whose speaker changed to a different track.
    pub reassigned: usize,
    /// Utterances whose owning track equalled their existing speaker (already
    /// correct) or that were left as-is for a flagged reason.
    pub unchanged: usize,
    /// Utterances that could not be confidently reattributed.
    pub flagged: Vec<FlaggedUtterance>,
    /// Utterances whose time was meaningfully split between tracks, when a
    /// [`ContestedThreshold`] was supplied. These WERE reattributed, to their
    /// winner; `flagged` keeps its narrower meaning of "declined".
    pub contested: Vec<ContestedUtterance>,
}

/// Machine-readable mirror of one rediarize pass: what `chatter
/// rediarize --summary-json` writes, shared here so every frontend
/// (CLI, desktop) emits the identical contract. Borrows from the
/// outcome and turns file; serialize it, don't store it.
///
/// JSON shape (a stable output contract, documented in
/// `book/src/chatter/user-guide/rediarize.md`):
///
/// ```json
/// {"source": "pyannote/...", "reassigned": 747, "unchanged": 145,
///  "flagged": [{"utterance_index": 12, "kept_speaker": "PAR1",
///               "reason": "no_overlapping_turn"}]}
/// ```
///
/// `unchanged` includes flagged utterances (they kept their speaker),
/// so total utterances = `reassigned + unchanged`.
#[derive(Debug, serde::Serialize)]
pub struct RediarizeSummary<'a> {
    /// Producer provenance from the turns file, if it carried one.
    pub source: Option<&'a str>,
    /// Utterances whose speaker changed to a different track.
    pub reassigned: usize,
    /// Utterances that kept their speaker (already correct, or flagged).
    pub unchanged: usize,
    /// Every utterance the transform declined to reattribute (unlike the
    /// human-readable stderr summary, this list is never truncated).
    pub flagged: &'a [FlaggedUtterance],
    /// Utterances whose time was meaningfully split between tracks. Empty
    /// unless `--contested-at` was supplied; these WERE reattributed.
    pub contested: &'a [ContestedUtterance],
}

impl<'a> RediarizeSummary<'a> {
    /// Assemble the summary from a pass's outcome plus the turns-file
    /// provenance it was driven by.
    pub fn new(source: Option<&'a DiarizationSource>, outcome: &'a RediarizeOutcome) -> Self {
        Self {
            source: source.map(DiarizationSource::as_str),
            reassigned: outcome.reassigned,
            unchanged: outcome.unchanged,
            flagged: &outcome.flagged,
            contested: &outcome.contested,
        }
    }
}

/// Content-level entry point mirroring `speaker_id::apply_mapping`:
/// parse `content`, run [`rediarize`], and re-serialize through the
/// typed model. This is the seam the CLI (and any future desktop
/// surface) calls, so frontends share one implementation.
pub fn rediarize_content(
    content: &str,
    timeline: &DiarizationTimeline,
    options: ParseValidateOptions,
    contested_at: Option<ContestedThreshold>,
) -> Result<(String, RediarizeOutcome), PipelineError> {
    let chat = parse_and_validate(content, options)?;
    let (rewritten, outcome) = rediarize(&chat, timeline, contested_at);
    Ok((to_chat_string(&rewritten), outcome))
}

/// Re-attribute every bulleted utterance in `chat` to the diarization track
/// holding the greatest union of time across that track's turns, returning the
/// rewritten [`ChatFile`] and an outcome report. Headers are reconciled to the
/// set of tracks actually used.
///
/// Supply `contested_at` to have utterances whose time is meaningfully split
/// reported in [`RediarizeOutcome::contested`]. There is no default; see
/// [`ContestedThreshold`].
///
/// The input `chat` is not mutated; a new `ChatFile` is built.
pub fn rediarize(
    chat: &ChatFile,
    timeline: &DiarizationTimeline,
    contested_at: Option<ContestedThreshold>,
) -> (ChatFile, RediarizeOutcome) {
    let mut outcome = RediarizeOutcome::default();
    let mut used_tracks: HashSet<SpeakerCode> = HashSet::new();
    let mut rewritten: Vec<Line> = Vec::with_capacity(chat.lines.as_slice().len());
    let mut utterance_index = 0usize;

    for line in chat.lines.as_slice().iter() {
        match line {
            Line::Utterance(u) => {
                let index = utterance_index;
                utterance_index += 1;
                let bullet = u.main.content.bullet.as_ref();
                match bullet.and_then(|b| TrackOwnership::of(b, timeline)) {
                    Some(ownership) => {
                        let track = ownership.winner().clone();
                        if contested_at.is_some_and(|at| at.contests(ownership.runner_up_share())) {
                            outcome.contested.push(ContestedUtterance {
                                utterance_index: index,
                                ownership,
                            });
                        }
                        used_tracks.insert(track.clone());
                        if track == u.main.speaker {
                            outcome.unchanged += 1;
                            rewritten.push(line.clone());
                        } else {
                            outcome.reassigned += 1;
                            let mut cloned = u.as_ref().clone();
                            cloned.main.speaker = track;
                            rewritten.push(Line::Utterance(Box::new(cloned)));
                        }
                    }
                    None => {
                        let reason = if bullet.is_none() {
                            FlagReason::NoBullet
                        } else {
                            FlagReason::NoOverlappingTurn
                        };
                        outcome.flagged.push(FlaggedUtterance {
                            utterance_index: index,
                            kept_speaker: u.main.speaker.clone(),
                            reason,
                        });
                        outcome.unchanged += 1;
                        used_tracks.insert(u.main.speaker.clone());
                        rewritten.push(line.clone());
                    }
                }
            }
            Line::Header { .. } => rewritten.push(line.clone()),
        }
    }

    let reconciled = reconcile_headers(rewritten, &used_tracks);
    (ChatFile::new(reconciled), outcome)
}

/// How a bullet's overlapped time divides between diarization tracks.
///
/// # Why this is a distribution and not a winner
///
/// It was `fn best_track(..) -> Option<SpeakerCode>`, which computed the whole
/// comparison and returned one name, so nothing downstream could tell a track
/// that held 95% of an utterance from one that held 34% of a three-way split.
/// Two facts were lost at once, and the second one is the reason the first was
/// wrong: it took the single greatest `overlap_ms` with NO per-track
/// accumulator, so three short turns of PAR2 lost to one longer turn of PAR1
/// even when PAR2 held twice as much of the utterance. pyannote emits exactly
/// that shape, short turns with gaps inside one speaker's run.
///
/// The serde field names (`shares`, `total_ms`) are part of the
/// machine-readable summary contract; see [`RediarizeSummary`].
///
/// The WHOLE distribution is emitted, not a winner and a runner-up. That
/// narrower shape is what the requesting consumer had tried twice and
/// abandoned: it cannot tell a 55/45 split from 55/23/22, and their
/// cross-tabulations need the difference. Contested utterances are a minority
/// of a file and the reader is a program, so verbosity is the cheap side.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TrackOwnership {
    /// Every track that overlaps the bullet at all, with the union of time its
    /// turns hold, descending. Never empty: [`Self::of`] returns `None` instead.
    shares: Vec<(SpeakerCode, u64)>,
    /// Sum of each track's union-held milliseconds.
    total_ms: u64,
}

/// Union accumulator for one track over one bullet.
///
/// Named fields prevent a held duration and an absolute media endpoint from
/// being exchanged in a three-element tuple. [`DiarizationTimeline`] proves
/// calls to [`Self::include`] arrive in nondecreasing start order.
struct TrackHold {
    track: SpeakerCode,
    held_ms: u64,
    counted_until_ms: u64,
}

impl TrackHold {
    fn first(track: SpeakerCode, span: NonEmptyTimeSpanMs) -> Self {
        Self {
            track,
            held_ms: span.duration_ms(),
            counted_until_ms: span.end_ms,
        }
    }

    fn include(&mut self, span: NonEmptyTimeSpanMs) {
        let new_start = span.start_ms.max(self.counted_until_ms);
        self.held_ms = self
            .held_ms
            .saturating_add(span.end_ms.saturating_sub(new_start));
        self.counted_until_ms = self.counted_until_ms.max(span.end_ms);
    }

    fn into_share(self) -> (SpeakerCode, u64) {
        (self.track, self.held_ms)
    }
}

impl TrackOwnership {
    /// The distribution for one bullet, or `None` when no turn overlaps it.
    fn of(bullet: &talkbank_model::model::Bullet, timeline: &DiarizationTimeline) -> Option<Self> {
        // Through the checked constructor, not a struct literal. The accessor
        // docs above say "the fields are private so that `Self::new` is the
        // only route in and an inverted span cannot be built"; a literal here
        // would have made that sentence false in the same file that asserts
        // it. A bullet whose end precedes its start owns nothing, which is
        // what `None` already means here.
        let utt = TimeSpanMs::new(bullet.timing.start_ms, bullet.timing.end_ms).ok()?;
        Self::of_span(utt, timeline)
    }

    /// The distribution for one time span over turns SORTED BY START.
    ///
    /// # Why a window rather than a scan of every turn
    ///
    /// The predecessor looked at every turn for every bullet, which is
    /// O(utterances x turns) and invisible from the call site: a 900-utterance
    /// transcript against a 40,000-turn diarization did 36 million overlap
    /// tests. Turns are time ordered, so the ones that can overlap a bullet
    /// are a CONTIGUOUS WINDOW, and finding its edges is two comparisons
    /// instead of a full pass. Measured on the proxy: 15.8 ms to 0.62 ms at
    /// 40,000 turns, and 1.07 ms to 0.10 ms at a realistic 2,581.
    ///
    /// The window's left edge is the subtle part. A turn can start well before
    /// the bullet and still overlap it, so it is not enough to start at the
    /// first turn with `start >= utt.start`: the search begins at the first
    /// turn that could reach the bullet at all, which is
    /// `utt.start - longest_turn`. That is why [`DiarizationTimeline`] records
    /// the longest duration rather than recomputing it per bullet.
    ///
    /// `the_windowed_scan_agrees_with_a_full_scan_on_every_bullet` pins this
    /// against the old implementation over the shapes that break a naive
    /// window: a turn far longer than its neighbours, zero-length turns, turns
    /// touching the bullet's edges exactly, and turns wholly outside it.
    fn of_span(utt: TimeSpanMs, timeline: &DiarizationTimeline) -> Option<Self> {
        let turns = timeline.turns();
        let first = turns.partition_point(|turn| {
            turn.span
                .start_ms()
                .saturating_add(timeline.longest_turn_ms)
                < utt.start_ms()
        });

        // Global start ordering also orders each track's subsequence. Remember
        // the end already counted for that track so a diarizer's overlapping
        // same-track turns describe one held interval rather than extra time.
        // Different tracks remain independent: simultaneous speakers each hold
        // the shared media interval, which is exactly the crosstalk evidence the
        // distribution is meant to retain.
        let mut held: Vec<TrackHold> = Vec::new();
        for turn in &turns[first..] {
            // Sorted by start, so once a turn begins at or after the bullet
            // ends, so does every turn after it.
            if turn.span.start_ms() >= utt.end_ms() && utt.end_ms() > utt.start_ms() {
                break;
            }
            let Some(span) = utt.intersection(&turn.span) else {
                continue;
            };
            match held.iter_mut().find(|hold| hold.track == turn.track) {
                Some(hold) => hold.include(span),
                None => held.push(TrackHold::first(turn.track.clone(), span)),
            }
        }
        if held.is_empty() {
            return None;
        }
        let mut shares: Vec<(SpeakerCode, u64)> =
            held.into_iter().map(TrackHold::into_share).collect();
        let total_ms = shares
            .iter()
            .fold(0u64, |total, (_, held_ms)| total.saturating_add(*held_ms));
        // Descending by held time. Ties break on the track code so the winner
        // is a function of the input rather than of turn order, which would
        // otherwise make the output depend on how the diarizer sorted its file.
        shares.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.as_str().cmp(b.0.as_str())));
        Some(Self { shares, total_ms })
    }

    /// The track holding the most of the bullet.
    #[must_use]
    pub fn winner(&self) -> &SpeakerCode {
        // `of` refuses to build an empty one, so this cannot be absent.
        &self.shares[0].0
    }

    /// Every overlapping track with its union-held milliseconds, descending.
    #[must_use]
    pub fn shares(&self) -> &[(SpeakerCode, u64)] {
        &self.shares
    }

    /// Sum of each track's union-held milliseconds.
    #[must_use]
    pub fn total_ms(&self) -> u64 {
        self.total_ms
    }

    /// The runner-up's share of total per-track union-held time, `0.0` when one
    /// track holds all of it.
    ///
    /// The runner-up rather than "everyone but the winner": two rivals at 20%
    /// each are a different situation from one at 40%, and this is the
    /// question a threshold is asked about.
    #[must_use]
    pub fn runner_up_share(&self) -> RunnerUpShare {
        match (self.shares.get(1), self.total_ms) {
            (Some((_, runner_up)), total) if total > 0 => {
                // `u64 as f64` is lossless below 2^53 ms, which is 285,000
                // years of audio.
                RunnerUpShare(*runner_up as f64 / total as f64)
            }
            _ => RunnerUpShare(0.0),
        }
    }
}

/// The share of total per-track union-held time a runner-up must hold for the
/// utterance to be reported as contested.
///
/// There is deliberately NO DEFAULT: no threshold means no contested
/// reporting. What share makes an utterance genuinely mixed has not been
/// measured against human listening; the rediarize book page carries the
/// reasoning.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContestedThreshold(f64);

/// A share that is not a share.
#[derive(Debug, thiserror::Error)]
#[error("a contested threshold must be a share between 0.0 and 1.0, got {0}")]
pub struct InvalidThreshold(f64);

impl ContestedThreshold {
    /// Build a threshold, refusing anything that is not a share.
    ///
    /// # Errors
    ///
    /// [`InvalidThreshold`] for a value outside `0.0..=1.0`, or for `NaN`,
    /// which no comparison would ever satisfy and which would silently mean
    /// "nothing is ever contested".
    pub fn new(share: f64) -> Result<Self, InvalidThreshold> {
        if share.is_nan() || !(0.0..=1.0).contains(&share) {
            return Err(InvalidThreshold(share));
        }
        Ok(Self(share))
    }

    /// Whether a runner-up holding this much of the total track-held time
    /// counts as contested.
    ///
    /// The comparison lives HERE rather than at the call site, and there is no
    /// accessor handing the raw `f64` back. Two `f64`s meaning different
    /// things (a threshold somebody supplied, a share something measured) is
    /// the tell for a missing type: nothing stops the comparison being written
    /// backwards, or against `total_ms`, or against a 0-to-100 number. An
    /// earlier `share()` existed for "a caller reporting what it was told",
    /// and the only caller it ever had was the comparison itself.
    #[must_use]
    pub fn contests(self, runner_up: RunnerUpShare) -> bool {
        runner_up.0 >= self.0
    }
}

/// The runner-up track's share of total per-track union-held time.
///
/// A newtype so it cannot be compared against anything but a
/// [`ContestedThreshold`], and so a reader of a signature can tell it from the
/// threshold it is measured against.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct RunnerUpShare(f64);

impl RunnerUpShare {
    /// The share as a number, for reporting.
    #[must_use]
    pub fn as_f64(self) -> f64 {
        self.0
    }
}

/// An utterance whose time was meaningfully split between tracks.
///
/// Reported, not declined: the winner is still the best available answer and
/// the utterance is still reattributed to it. This makes the mixed-utterance
/// population visible in every run rather than only in a separate census.
/// The serde field names (`utterance_index`, `assigned`, `ownership`) are the
/// machine-readable summary contract; renaming one is a breaking change to it.
#[derive(Debug, Clone)]
pub struct ContestedUtterance {
    /// 0-based position of the utterance among main-tier lines.
    pub utterance_index: usize,
    /// The whole distribution, so a reader can see the split rather than
    /// being told a verdict about it.
    pub ownership: TrackOwnership,
}

/// `assigned` is SERIALIZED but not STORED.
///
/// It was a field beside `ownership`, with a doc saying "which is
/// `ownership.winner()`": two representations of one fact held together by a
/// comment, and nothing enforcing the equality. Any future rule that assigns a
/// speaker for a reason other than the winner (a tie-break policy, an operator
/// override) would have set one and left the other describing a different
/// verdict, and the JSON would have said two things.
///
/// Derived at serialization instead, so the documented wire contract is
/// byte-identical and the two cannot drift.
impl serde::Serialize for ContestedUtterance {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut row = serializer.serialize_struct("ContestedUtterance", 3)?;
        row.serialize_field("utterance_index", &self.utterance_index)?;
        row.serialize_field("assigned", self.ownership.winner())?;
        row.serialize_field("ownership", &self.ownership)?;
        row.end()
    }
}

/// Rebuild `@Participants` and `@ID` headers so exactly `used_tracks` are
/// declared. Existing entries/rows for a used track are kept verbatim; a
/// used track with no existing declaration gets one cloned from an
/// existing sibling (same role) with the code swapped; declarations for
/// tracks no longer used are dropped.
fn reconcile_headers(lines: Vec<Line>, used_tracks: &HashSet<SpeakerCode>) -> Vec<Line> {
    let template_entry = lines.iter().find_map(|line| match line {
        Line::Header { header, .. } => match header.as_ref() {
            Header::Participants { entries } => entries.iter().next().cloned(),
            _ => None,
        },
        _ => None,
    });
    let template_id = lines.iter().find_map(|line| match line {
        Line::Header { header, .. } => match header.as_ref() {
            Header::ID(id) => Some(id.clone()),
            _ => None,
        },
        _ => None,
    });

    let mut declared_ids: HashSet<SpeakerCode> = HashSet::new();
    let mut result: Vec<Line> = Vec::with_capacity(lines.len());

    for line in lines {
        match line {
            Line::Header {
                header,
                span,
                separator,
            } => match *header {
                Header::Participants { entries } => {
                    let mut kept: Vec<ParticipantEntry> = entries
                        .iter()
                        .filter(|e| used_tracks.contains(&e.speaker_code))
                        .cloned()
                        .collect();
                    let present: HashSet<SpeakerCode> =
                        kept.iter().map(|e| e.speaker_code.clone()).collect();
                    for track in used_tracks {
                        if !present.contains(track)
                            && let Some(tpl) = &template_entry
                        {
                            kept.push(ParticipantEntry {
                                speaker_code: track.clone(),
                                name: tpl.name.clone(),
                                role: tpl.role.clone(),
                            });
                        }
                    }
                    kept.sort_by(|a, b| a.speaker_code.as_str().cmp(b.speaker_code.as_str()));
                    result.push(Line::Header {
                        header: Box::new(Header::Participants {
                            entries: ParticipantEntries::new(kept),
                        }),
                        span,
                        separator,
                    });
                }
                Header::ID(id) => {
                    if used_tracks.contains(&id.speaker) {
                        declared_ids.insert(id.speaker.clone());
                        result.push(Line::Header {
                            header: Box::new(Header::ID(id)),
                            span,
                            separator,
                        });
                    }
                    // else: drop the @ID row for an unused track.
                }
                other => result.push(Line::Header {
                    header: Box::new(other),
                    span,
                    separator,
                }),
            },
            other => result.push(other),
        }
    }

    // Insert @ID rows for used tracks that had none, cloned from a
    // template. They must land WITH the header block (after the last
    // surviving @ID row, or after @Participants when every original
    // @ID was dropped), never appended at end-of-file: an appended row
    // lands after @End and makes the output invalid CHAT (E501; caught
    // on the first real corpus file, 2026-07-08).
    if let Some(tpl) = template_id {
        let mut ordered: Vec<&SpeakerCode> = used_tracks.iter().collect();
        ordered.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        let new_rows: Vec<Line> = ordered
            .into_iter()
            .filter(|track| !declared_ids.contains(*track))
            .map(|track| {
                let mut new_id = tpl.clone();
                new_id.speaker = track.clone();
                Line::Header {
                    header: Box::new(Header::ID(new_id)),
                    span: talkbank_model::Span::DUMMY,
                    separator: TierSeparator::CLEAN,
                }
            })
            .collect();
        if !new_rows.is_empty() {
            let anchor = result.iter().rposition(|line| {
                matches!(line, Line::Header { header, .. }
                    if matches!(header.as_ref(), Header::ID(_) | Header::Participants { .. }))
            });
            // `new_rows` is non-empty only when some utterance used the
            // track, so a first utterance exists as the final fallback
            // anchor; `result.len()` is the total-function backstop for
            // a state that cannot occur (no headers AND no utterances).
            let insert_at = match anchor {
                Some(header_index) => header_index + 1,
                None => result
                    .iter()
                    .position(|line| matches!(line, Line::Utterance(_)))
                    .unwrap_or(result.len()),
            };
            result.splice(insert_at..insert_at, new_rows);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_lenient;
    use talkbank_parser::TreeSitterParser;

    // Two Rev tracks; the second is really two different adults across
    // time (0-1s and 2-3s), which a good diarizer splits into PAR1/PAR2.
    const FIXTURE: &str = "@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR0 Participant, PAR1 Participant
@ID:\teng|corpus|PAR0|||||Participant|||
@ID:\teng|corpus|PAR1|||||Participant|||
@Media:\ts, audio
*PAR0:\thello there . \u{15}0_1000\u{15}
*PAR1:\thi yourself . \u{15}1000_2000\u{15}
*PAR1:\tand goodbye . \u{15}2000_3000\u{15}
@End
";

    fn turn(track: &str, start_ms: u64, end_ms: u64) -> DiarizationTurn {
        DiarizationTurn {
            track: SpeakerCode::new(track),
            span: TimeSpanMs::new(start_ms, end_ms).expect("valid span"),
        }
    }

    #[test]
    fn splits_a_merged_track_by_overlap() {
        let parser = TreeSitterParser::new().expect("parser");
        let (chat, _errors) = parse_lenient(&parser, FIXTURE);

        // Diarization: PAR0 owns 0-1s; a DISTINCT adult owns 1-2s (PAR1);
        // a THIRD voice owns 2-3s (PAR2). Rev had lumped the last two.
        let turns = vec![
            turn("PAR0", 0, 1000),
            turn("PAR1", 1000, 2000),
            turn("PAR2", 2000, 3000),
        ];

        let timeline = DiarizationTimeline::new(turns);
        let (out, outcome) = rediarize(&chat, &timeline, None);
        let text = crate::serialize::to_chat_string(&out);

        // The third utterance moved off PAR1 onto PAR2.
        assert!(
            text.contains("*PAR2:\tand goodbye ."),
            "third utterance should be reattributed to PAR2.\n{text}"
        );
        // The first two keep their (correct) tracks.
        assert!(text.contains("*PAR0:\thello there ."), "{text}");
        assert!(text.contains("*PAR1:\thi yourself ."), "{text}");
        // One reassignment (PAR1 -> PAR2), two unchanged, none flagged.
        assert_eq!(outcome.reassigned, 1, "exactly one utterance reattributed");
        assert!(outcome.flagged.is_empty(), "no utterance should be flagged");
        // Headers reconciled: PAR2 now declared.
        assert!(
            text.contains("PAR2 Participant"),
            "PAR2 must be added to @Participants.\n{text}"
        );
        assert!(
            text.contains("eng|corpus|PAR2|"),
            "PAR2 must get an @ID row.\n{text}"
        );
    }

    /// The winner is the track holding the most time IN TOTAL, not the track
    /// of the single longest turn.
    ///
    /// pyannote emits short turns with gaps inside one speaker's run, so a
    /// track can hold most of a bullet across several turns while another
    /// holds one longer turn. `best_track` took the single greatest
    /// `overlap_ms` with no per-track accumulator, so it answered with the
    /// track of the longest TURN while its own docstring and the CLI help both
    /// said "the track with the greatest overlap".
    ///
    /// Here PAR2 holds 300+300 = 600 ms of the second utterance and PAR1 holds
    /// one turn of 400 ms, so PAR2 owns more of it despite the longer rival
    /// turn.
    #[test]
    fn the_winner_is_the_track_with_the_most_time_not_the_longest_turn() {
        let parser = TreeSitterParser::new().expect("parser");
        let (chat, _errors) = parse_lenient(&parser, FIXTURE);

        let turns = vec![
            turn("PAR0", 0, 1000),
            // The second utterance spans 1000-2000.
            turn("PAR2", 1000, 1300),
            turn("PAR1", 1300, 1700),
            turn("PAR2", 1700, 2000),
            turn("PAR1", 2000, 3000),
        ];

        let timeline = DiarizationTimeline::new(turns);
        let (out, _outcome) = rediarize(&chat, &timeline, None);
        let text = crate::serialize::to_chat_string(&out);
        assert!(
            text.contains("*PAR2:\thi yourself ."),
            "the second utterance belongs to PAR2, which holds 600 ms of it against \
             PAR1's 400 ms, even though PAR1's single turn is the longest one \
             touching it.\n{text}"
        );
    }

    /// A contested utterance is REPORTED, and still reattributed to its winner.
    ///
    /// Reporting is the ask: the mixed-utterance population becomes visible per
    /// run rather than only in a separate census. Declining to reattribute
    /// would be a different and larger decision, and the winner is still the
    /// best available answer.
    #[test]
    fn a_contested_utterance_is_reported_with_its_shares() {
        let parser = TreeSitterParser::new().expect("parser");
        let (chat, _errors) = parse_lenient(&parser, FIXTURE);

        // The second utterance (1000-2000) splits 600/400 between PAR2/PAR1.
        let turns = vec![
            turn("PAR0", 0, 1000),
            turn("PAR2", 1000, 1600),
            turn("PAR1", 1600, 2000),
            turn("PAR1", 2000, 3000),
        ];

        let threshold = ContestedThreshold::new(0.25).expect("valid share");
        let timeline = DiarizationTimeline::new(turns);
        let (_out, outcome) = rediarize(&chat, &timeline, Some(threshold));

        assert_eq!(outcome.contested.len(), 1, "one utterance is contested");
        let contested = &outcome.contested[0];
        assert_eq!(contested.utterance_index, 1);
        assert_eq!(contested.ownership.winner().as_str(), "PAR2");
        assert_eq!(contested.ownership.total_ms(), 1000);
        assert!(
            (contested.ownership.runner_up_share().as_f64() - 0.4).abs() < 1e-9,
            "PAR1 holds 400 of 1000 ms; got {}",
            contested.ownership.runner_up_share().as_f64()
        );
    }

    /// NO THRESHOLD MEANS NO REPORTING, which is what "an argument with no
    /// default" has to mean at the library seam as well as at the CLI.
    ///
    /// The share that makes an utterance contested is uncalibrated: the value
    /// the request arrived with (0.20) was carried from another tool's default
    /// and never measured against ears. A default here would hand every
    /// consumer a magic constant wearing the authority of the library.
    #[test]
    fn without_a_threshold_nothing_is_contested() {
        let parser = TreeSitterParser::new().expect("parser");
        let (chat, _errors) = parse_lenient(&parser, FIXTURE);
        let turns = vec![
            turn("PAR0", 0, 1000),
            turn("PAR2", 1000, 1600),
            turn("PAR1", 1600, 2000),
        ];

        let timeline = DiarizationTimeline::new(turns);
        let (_out, outcome) = rediarize(&chat, &timeline, None);
        assert!(
            outcome.contested.is_empty(),
            "with no threshold supplied, nothing is reported as contested"
        );
    }

    /// The windowed scan answers exactly what a full scan of every turn does.
    ///
    /// The production path searches one ordered window and unions same-track
    /// coverage incrementally. This independent oracle scans every turn, then
    /// sorts and unions each track's clipped intervals; agreement pins that
    /// the optimized window is the right one rather than merely a faster one.
    ///
    /// The adversarial part is the shapes that break a naive window: a turn
    /// much LONGER than the others (so a turn starting well before the
    /// utterance still overlaps it), zero-length turns, turns that touch the
    /// bullet's edges exactly, and turns entirely outside it.
    #[test]
    fn the_windowed_scan_agrees_with_a_full_scan_on_every_bullet() {
        /// A deliberately direct full-scan, per-track interval-union oracle.
        fn full_scan(utt: TimeSpanMs, turns: &[DiarizationTurn]) -> Vec<(String, u64)> {
            let mut intervals: Vec<(String, Vec<(u64, u64)>)> = Vec::new();
            for turn in turns {
                let start_ms = utt.start_ms().max(turn.span.start_ms());
                let end_ms = utt.end_ms().min(turn.span.end_ms());
                if end_ms <= start_ms {
                    continue;
                }
                match intervals
                    .iter_mut()
                    .find(|(track, _)| track == turn.track.as_str())
                {
                    Some((_, spans)) => spans.push((start_ms, end_ms)),
                    None => {
                        intervals.push((turn.track.as_str().to_string(), vec![(start_ms, end_ms)]))
                    }
                }
            }
            let mut shares: Vec<(String, u64)> = intervals
                .into_iter()
                .map(|(track, mut spans)| {
                    spans.sort_by_key(|(start_ms, _)| *start_ms);
                    let mut held_ms = 0u64;
                    let mut counted_until_ms = 0u64;
                    for (start_ms, end_ms) in spans {
                        let new_start = start_ms.max(counted_until_ms);
                        held_ms += end_ms.saturating_sub(new_start);
                        counted_until_ms = counted_until_ms.max(end_ms);
                    }
                    (track, held_ms)
                })
                .collect();
            shares.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            shares
        }

        let turns = vec![
            turn("PAR0", 0, 100),
            turn("PAR1", 90, 90),   // zero length, touching
            turn("PAR2", 95, 105),  // straddles the boundary
            turn("PAR0", 100, 200), // starts exactly where a bullet ends
            turn("PAR0", 50, 150),  // overlaps two turns of its own track
            turn("LONG", 0, 5_000), // longer than every other turn
            turn("PAR1", 300, 400),
            turn("PAR2", 4_900, 5_000),
        ];
        let timeline = DiarizationTimeline::new(turns.clone());

        // Every bullet shape, including empty ones and ones past the end.
        for start in [0u64, 50, 95, 100, 299, 300, 1_000, 4_950, 6_000] {
            for len in [0u64, 1, 50, 100, 5_000] {
                let utt = TimeSpanMs::new(start, start + len).expect("valid");
                let expected = full_scan(utt, &turns);
                let got = TrackOwnership::of_span(utt, &timeline)
                    .map(|o| {
                        o.shares()
                            .iter()
                            .map(|(t, ms)| (t.as_str().to_string(), *ms))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                assert_eq!(
                    got,
                    expected,
                    "window disagreed with the full scan for bullet {start}..{}",
                    start + len
                );
            }
        }
    }

    /// A turns file in ARBITRARY order still gets the right answer.
    ///
    /// The windowed scan needs turns sorted by start. Nothing in the JSON
    /// format requires a diarizer to emit them in order, and a precondition
    /// held by nothing is one a caller breaks. `parse_turns_json` sorts, so
    /// this drives the real entry point rather than handing `rediarize` a
    /// list the test sorted itself.
    #[test]
    fn a_turns_file_in_arbitrary_order_is_still_read_correctly() -> Result<(), TurnsJsonError> {
        let parser = TreeSitterParser::new().expect("parser");
        let (chat, _errors) = parse_lenient(&parser, FIXTURE);

        // Deliberately shuffled, and the LAST entry is the one that decides
        // the second utterance.
        let json = r#"{"turns":[
            {"track":"PAR1","start_ms":2000,"end_ms":3000},
            {"track":"PAR1","start_ms":1300,"end_ms":1700},
            {"track":"PAR0","start_ms":0,"end_ms":1000},
            {"track":"PAR2","start_ms":1700,"end_ms":2000},
            {"track":"PAR2","start_ms":1000,"end_ms":1300}
        ]}"#;
        let file = parse_turns_json(json)?;

        let (out, _outcome) = rediarize(&chat, file.timeline(), None);
        let text = crate::serialize::to_chat_string(&out);
        assert!(
            text.contains("*PAR2:\thi yourself ."),
            "PAR2 holds 600 ms of the second utterance against PAR1's 400, \
             whatever order the file listed them in.\n{text}"
        );
        Ok(())
    }

    #[test]
    fn a_threshold_outside_zero_to_one_is_refused() {
        assert!(ContestedThreshold::new(-0.1).is_err());
        assert!(ContestedThreshold::new(1.5).is_err());
        assert!(ContestedThreshold::new(f64::NAN).is_err());
        assert!(ContestedThreshold::new(0.0).is_ok());
        assert!(ContestedThreshold::new(1.0).is_ok());
    }

    #[test]
    fn flags_utterance_with_no_overlapping_turn() {
        let parser = TreeSitterParser::new().expect("parser");
        let (chat, _errors) = parse_lenient(&parser, FIXTURE);
        // Turns cover only 0-1s; the two later utterances overlap nothing.
        let turns = vec![turn("PAR0", 0, 1000)];

        let timeline = DiarizationTimeline::new(turns);
        let (_out, outcome) = rediarize(&chat, &timeline, None);
        assert_eq!(
            outcome.flagged.len(),
            2,
            "two utterances have no overlapping turn"
        );
        assert!(
            outcome
                .flagged
                .iter()
                .all(|f| f.reason == FlagReason::NoOverlappingTurn)
        );
    }
}
