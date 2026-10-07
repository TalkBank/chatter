//! Word-level speaker attribution: measured ownership of each source word,
//! admitted against the same source as the partition it produces.
use talkbank_model::alignment::{
    CompleteWorTimings, WorTimingBinding, WorTimingCorrespondence, WorTimingSequence,
    assess_wor_timing_sequence, bind_wor_timing, corroborate_wor_timing,
};
use talkbank_model::model::{Bullet, Utterance};

use super::{SplitChildren, SplitOutcome, SplitRefusal, UtteranceSplitPlan};
use crate::rediarize::{DiarizationTimeline, TrackOwnership};

/// Why measured word-level attribution cannot establish a structural split.
#[derive(Debug, thiserror::Error)]
pub enum WordSpeakerSplitRefusal {
    /// No source word tier exists; a parent bullet does not time its words.
    #[error("word-level speaker attribution requires a source %wor tier")]
    MissingWordTiming,
    /// Positional counts do not match the producing main tier.
    #[error("source %wor has {wor} words but the main tier has {main}")]
    WordCount {
        /// Main-tier lexical slots.
        main: usize,
        /// Recorded word-tier slots.
        wor: usize,
    },
    /// Equal counts alone do not prove lexical correspondence.
    #[error("source %wor disagrees lexically at word slots {slots:?}")]
    LexicalMismatch {
        /// Source-word positions that did not corroborate.
        slots: Vec<usize>,
    },
    /// An empty projection supplies no attribution evidence.
    #[error("source has no lexical word timing to attribute")]
    EmptyWordTiming,
    /// Missing or nonpositive word intervals cannot establish ownership.
    #[error("source word timing is incomplete: {issues:?}")]
    IncompleteWordTiming {
        /// Exhaustive issues retained from the source timing admission.
        issues: Vec<talkbank_model::alignment::WorTimingSequenceIssue>,
    },
    /// An uncovered word cannot inherit a nearby or previous track.
    #[error("no diarization turn overlaps source word {slot}")]
    UncoveredWord {
        /// Source word with no positive held time.
        slot: usize,
    },
    /// Equal held time is genuine ambiguity, not evidence for either track.
    #[error("source word {slot} has tied ownership among {tracks:?}")]
    TiedWord {
        /// Source word whose largest held time was shared.
        slot: usize,
        /// Equally supported tracks, not a guessed winner.
        tracks: Vec<talkbank_model::SpeakerCode>,
    },
    /// The proposed speaker boundary crosses indivisible CHAT structure.
    #[error(transparent)]
    Partition(#[from] SplitRefusal),
    /// Internal producer inconsistency, never a claim of invalid CHAT.
    #[error("partition producer returned {children} children for {runs} speaker runs")]
    ProducerChildCount {
        /// Actual structural children returned by the producer.
        children: usize,
        /// Admitted contiguous ownership runs.
        runs: usize,
    },
}

/// What measured word ownership did to its source utterance.
pub enum WordSpeakerPartition<'source> {
    /// Every word belongs to one track. The source keeps its content and every
    /// dependent tier; only its speaker becomes `speaker`. Nothing was rebuilt
    /// or copied: the caller relabels the utterance it already holds.
    Relabeled {
        /// The track that owns every word.
        speaker: talkbank_model::SpeakerCode,
    },
    /// Two or more speaker runs, one relabeled child each.
    Split(SplitChildren<'source>),
}

/// Attributed structural children with their measured ownership and loss receipts.
#[must_use]
pub struct WordSpeakerSplitOutcome<'source> {
    partition: WordSpeakerPartition<'source>,
    ownership: Vec<TrackOwnership>,
}

impl<'source> WordSpeakerSplitOutcome<'source> {
    /// Inspect what the partition did to the source.
    pub fn partition(&self) -> &WordSpeakerPartition<'source> {
        &self.partition
    }

    /// Source-word distributions, including minority held time.
    pub fn ownership(&self) -> &[TrackOwnership] {
        &self.ownership
    }

    /// Consume all output and evidence together, without a children-only shortcut.
    pub fn into_parts(self) -> WordSpeakerSplitParts<'source> {
        WordSpeakerSplitParts {
            partition: self.partition,
            ownership: self.ownership,
        }
    }
}

/// Mutable transform output and retained evidence, not complete-validity proof.
pub struct WordSpeakerSplitParts<'source> {
    /// The relabeling or the relabeled children awaiting complete construction
    /// admission, with their source-bound dependent-tier losses.
    pub partition: WordSpeakerPartition<'source>,
    /// Measured distributions for every original word.
    pub ownership: Vec<TrackOwnership>,
}

/// Measured word ownership and its partition, admitted together against one source.
///
/// No acoustic inference occurs here. The external timeline is projected through
/// count-matched, lexically corroborated, complete source word timing. Ownership
/// uses the same union-of-held-time policy as whole-turn rediarization, but a tie
/// or gap refuses word splitting instead of inventing a track. Execution cannot
/// accept a different source or independently supplied speaker vector.
///
/// The retained borrow also prevents modifying the source between admission and
/// execution:
///
/// ```compile_fail,E0506
/// use talkbank_model::model::Utterance;
/// use talkbank_transform::{rediarize::DiarizationTimeline, utterance_split::WordSpeakerSplitPlan};
/// fn change(source: &mut Utterance, timeline: &DiarizationTimeline) {
///     let Ok(plan) = WordSpeakerSplitPlan::admit(source, timeline) else { return };
///     source.main.speaker = talkbank_model::SpeakerCode::new("OTHER");
///     let _ = plan.execute();
/// }
/// ```
#[must_use]
pub struct WordSpeakerSplitPlan<'source> {
    partition: UtteranceSplitPlan<'source>,
    ownership: Vec<TrackOwnership>,
}

/// Complete source word timing admitted before acoustic inference is requested.
///
/// The timing projection and producing utterance share one borrow. A caller can
/// retain this capability across inference and then bind the resulting timeline
/// without rematching counts or lexical content, or accepting a second source.
///
/// ```compile_fail,E0506
/// use talkbank_model::model::Utterance;
/// use talkbank_transform::{rediarize::DiarizationTimeline, utterance_split::WordSpeakerSource};
/// fn change(source: &mut Utterance, timeline: &DiarizationTimeline) {
///     let Ok(timed) = WordSpeakerSource::admit(source) else { return };
///     source.main.speaker = talkbank_model::SpeakerCode::new("OTHER");
///     let _ = timed.bind_timeline(timeline);
/// }
/// ```
#[must_use]
pub struct WordSpeakerSource<'source> {
    source: &'source Utterance,
    timing: CompleteWorTimings<'source>,
}

impl<'source> WordSpeakerSource<'source> {
    /// Admit nonempty, corroborated, positive timing from the exact source.
    ///
    /// # Errors
    /// Refuses missing, empty, count-drifted, lexically stale or incomplete timing.
    pub fn admit(source: &'source Utterance) -> Result<Self, WordSpeakerSplitRefusal> {
        let matched = match bind_wor_timing(&source.main, source.wor_tier()) {
            WorTimingBinding::CountMatched(matched) => matched,
            WorTimingBinding::Missing(missing) => {
                return Err(if missing.main_count().get() == 0 {
                    WordSpeakerSplitRefusal::EmptyWordTiming
                } else {
                    WordSpeakerSplitRefusal::MissingWordTiming
                });
            }
            WorTimingBinding::Drifted(drift) => {
                return Err(WordSpeakerSplitRefusal::WordCount {
                    main: drift.main_count().get(),
                    wor: drift.wor_count().get(),
                });
            }
        };
        let corroborated = match corroborate_wor_timing(matched) {
            WorTimingCorrespondence::Corroborated(corroborated) => corroborated,
            WorTimingCorrespondence::Uncorroborated(mismatch) => {
                return Err(WordSpeakerSplitRefusal::LexicalMismatch {
                    slots: mismatch
                        .mismatches()
                        .iter()
                        .map(|item| item.slot().get())
                        .collect(),
                });
            }
        };
        let complete = match assess_wor_timing_sequence(corroborated) {
            WorTimingSequence::Complete(complete) => complete,
            WorTimingSequence::Empty(_) => return Err(WordSpeakerSplitRefusal::EmptyWordTiming),
            WorTimingSequence::Rejected(rejected) => {
                return Err(WordSpeakerSplitRefusal::IncompleteWordTiming {
                    issues: rejected.issues().to_vec(),
                });
            }
        };
        Ok(Self {
            source,
            timing: complete,
        })
    }

    /// The admitted source timing, not a caller-supplied replacement vector.
    pub fn timing(&self) -> &CompleteWorTimings<'source> {
        &self.timing
    }

    /// Bind acoustic evidence to the already admitted source-word population.
    ///
    /// Ownership is measured once per word, each a windowed scan of the
    /// timeline. The window's left edge is `word.start - longest_turn_ms`
    /// ([`DiarizationTimeline`] records the longest turn), so a single very
    /// long turn anywhere in the timeline widens every word's scan. The tie
    /// check reads the measured distribution in place; only a refusal
    /// allocates the list of tied tracks.
    ///
    /// # Errors
    /// Refuses uncovered or tied ownership and indivisible structural boundaries.
    pub fn bind_timeline(
        self,
        timeline: &DiarizationTimeline,
    ) -> Result<WordSpeakerSplitPlan<'source>, WordSpeakerSplitRefusal> {
        let Self {
            source,
            timing: complete,
        } = self;
        let mut ownership = Vec::with_capacity(complete.slots().len());
        let mut assignments = Vec::with_capacity(complete.slots().len());
        let mut run = 0usize;
        for (slot, word) in complete.slots().iter().enumerate() {
            let timing = word.timing();
            let measured = Bullet::new(timing.start().get(), timing.end().get());
            let held = TrackOwnership::of(&measured, timeline)
                .ok_or(WordSpeakerSplitRefusal::UncoveredWord { slot })?;
            // `shares` is never empty and is sorted by held time, descending,
            // so a tie for the lead is exactly a runner-up holding as much.
            if let [(_, lead), (_, runner_up), ..] = held.shares()
                && lead == runner_up
            {
                let lead = *lead;
                let tracks = held
                    .shares()
                    .iter()
                    .take_while(|(_, duration)| *duration == lead)
                    .map(|(track, _)| track.clone())
                    .collect();
                return Err(WordSpeakerSplitRefusal::TiedWord { slot, tracks });
            }
            if ownership
                .last()
                .is_some_and(|previous: &TrackOwnership| previous.winner() != held.winner())
            {
                run += 1;
            }
            assignments.push(run);
            ownership.push(held);
        }
        let partition = UtteranceSplitPlan::for_word_timing(source, &assignments)?;
        Ok(WordSpeakerSplitPlan {
            partition,
            ownership,
        })
    }
}

impl<'source> WordSpeakerSplitPlan<'source> {
    /// Admit source word timing and bind the supplied timeline in one transition.
    ///
    /// Pipelines requesting inference should instead retain [`WordSpeakerSource`]
    /// first, so ineligible source timing is refused before model work.
    ///
    /// # Errors
    /// Refuses invalid timing, uncovered or tied ownership and unsafe boundaries.
    pub fn admit(
        source: &'source Utterance,
        timeline: &DiarizationTimeline,
    ) -> Result<Self, WordSpeakerSplitRefusal> {
        WordSpeakerSource::admit(source)?.bind_timeline(timeline)
    }

    /// The source-word distributions underlying the admitted speaker runs.
    pub fn ownership(&self) -> &[TrackOwnership] {
        &self.ownership
    }

    /// Rebuild and relabel from the admitted ownership, retaining tier-loss receipts.
    ///
    /// Participants/IDs remain the caller's responsibility; these track labels
    /// are not a claim of personal identity. Complete construction is still
    /// required before writing a document containing the children.
    ///
    /// # Errors
    /// Reports a producer inconsistency rather than silently truncating a zip.
    pub fn execute(self) -> Result<WordSpeakerSplitOutcome<'source>, WordSpeakerSplitRefusal> {
        let mut tracks = Vec::new();
        for held in &self.ownership {
            if tracks.last() != Some(held.winner()) {
                tracks.push(held.winner().clone());
            }
        }
        let partition = match self.partition.execute() {
            SplitOutcome::Unchanged => {
                // One run is the only population that splits nothing.
                let speaker = match <[_; 1]>::try_from(tracks) {
                    Ok([speaker]) => speaker,
                    Err(tracks) => {
                        return Err(WordSpeakerSplitRefusal::ProducerChildCount {
                            children: 1,
                            runs: tracks.len(),
                        });
                    }
                };
                WordSpeakerPartition::Relabeled { speaker }
            }
            SplitOutcome::Split(mut split) => {
                if split.children.len() != tracks.len() {
                    return Err(WordSpeakerSplitRefusal::ProducerChildCount {
                        children: split.children.len(),
                        runs: tracks.len(),
                    });
                }
                for (child, track) in split.children.iter_mut().zip(tracks) {
                    child.main.speaker = track;
                }
                WordSpeakerPartition::Split(split)
            }
        };
        Ok(WordSpeakerSplitOutcome {
            partition,
            ownership: self.ownership,
        })
    }
}
