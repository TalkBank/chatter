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
//! [`DiarizationTurn`] values (a track code plus a [`TimeSpanMs`]); this
//! module never touches audio. It operates entirely on the typed CHAT
//! model and re-serializes through the model, never string-assembling
//! CHAT.
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
use talkbank_model::model::{ChatFile, Line, SpeakerCode};

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

    /// Milliseconds of overlap between the two spans (0 if disjoint).
    pub fn overlap_ms(&self, other: &Self) -> u64 {
        let start = self.start_ms.max(other.start_ms);
        let end = self.end_ms.min(other.end_ms);
        end.saturating_sub(start)
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

/// Why an utterance was left unchanged instead of reassigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// Optional producer provenance (`"source"` in the JSON).
    pub source: Option<DiarizationSource>,
    /// The timestamped diarization turns, validated (no inverted spans).
    pub turns: Vec<DiarizationTurn>,
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
        turns,
    })
}

/// An utterance the transform could not confidently reattribute. Carries
/// the 0-based main-tier position and the speaker it kept, so the caller
/// can review or route it to human adjudication.
#[derive(Debug, Clone)]
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
    /// Utterances whose max-overlap track equalled their existing speaker
    /// (already correct) or that were left as-is for a flagged reason.
    pub unchanged: usize,
    /// Utterances that could not be confidently reattributed.
    pub flagged: Vec<FlaggedUtterance>,
}

/// Content-level entry point mirroring `speaker_id::apply_mapping`:
/// parse `content`, run [`rediarize`], and re-serialize through the
/// typed model. This is the seam the CLI (and any future desktop
/// surface) calls, so frontends share one implementation.
pub fn rediarize_content(
    content: &str,
    turns: &[DiarizationTurn],
    options: ParseValidateOptions,
) -> Result<(String, RediarizeOutcome), PipelineError> {
    let chat = parse_and_validate(content, options)?;
    let (rewritten, outcome) = rediarize(&chat, turns);
    Ok((to_chat_string(&rewritten), outcome))
}

/// Re-attribute every bulleted utterance in `chat` to its maximum-overlap
/// diarization track, returning the rewritten [`ChatFile`] and an outcome
/// report. Headers are reconciled to the set of tracks actually used.
///
/// The input `chat` is not mutated; a new `ChatFile` is built.
pub fn rediarize(chat: &ChatFile, turns: &[DiarizationTurn]) -> (ChatFile, RediarizeOutcome) {
    let mut outcome = RediarizeOutcome::default();
    let mut used_tracks: HashSet<SpeakerCode> = HashSet::new();
    let mut rewritten: Vec<Line> = Vec::with_capacity(chat.lines.0.len());
    let mut utterance_index = 0usize;

    for line in chat.lines.0.iter() {
        match line {
            Line::Utterance(u) => {
                let index = utterance_index;
                utterance_index += 1;
                let bullet = u.main.content.bullet.as_ref();
                match bullet.and_then(|b| best_track(b, turns)) {
                    Some(track) => {
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

/// The diarization track with the greatest overlap against `bullet`, or
/// `None` if no turn overlaps it at all.
fn best_track(
    bullet: &talkbank_model::model::Bullet,
    turns: &[DiarizationTurn],
) -> Option<SpeakerCode> {
    let utt = TimeSpanMs {
        start_ms: bullet.timing.start_ms,
        end_ms: bullet.timing.end_ms,
    };
    let mut best: Option<(&SpeakerCode, u64)> = None;
    for turn in turns {
        let overlap = utt.overlap_ms(&turn.span);
        if overlap == 0 {
            continue;
        }
        match best {
            Some((_, best_overlap)) if overlap <= best_overlap => {}
            _ => best = Some((&turn.track, overlap)),
        }
    }
    best.map(|(track, _)| track.clone())
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
            Line::Header { header, span } => match *header {
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
                    });
                }
                Header::ID(id) => {
                    if used_tracks.contains(&id.speaker) {
                        declared_ids.insert(id.speaker.clone());
                        result.push(Line::Header {
                            header: Box::new(Header::ID(id)),
                            span,
                        });
                    }
                    // else: drop the @ID row for an unused track.
                }
                other => result.push(Line::Header {
                    header: Box::new(other),
                    span,
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

        let (out, outcome) = rediarize(&chat, &turns);
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

    #[test]
    fn flags_utterance_with_no_overlapping_turn() {
        let parser = TreeSitterParser::new().expect("parser");
        let (chat, _errors) = parse_lenient(&parser, FIXTURE);
        // Turns cover only 0-1s; the two later utterances overlap nothing.
        let turns = vec![turn("PAR0", 0, 1000)];

        let (_out, outcome) = rediarize(&chat, &turns);
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
