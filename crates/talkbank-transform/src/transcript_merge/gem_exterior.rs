//! Opt-in ordering of speech strictly outside a fully timed reference gem.
use super::*;
use talkbank_model::model::Bullet;

/// Which exterior of the reference gem contains a complete donor interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GemExterior {
    /// The donor ends strictly before the gem's earliest speech.
    Before,
    /// The donor starts strictly after the gem's latest speech.
    After,
}

/// Source-derived placement evidence; not acoustic or lexical authority.
#[derive(Debug, Clone)]
pub struct GemExteriorPlacement {
    /// Selected donor coordinate, translated to original parents by the caller.
    pub donor: DonorIdx,
    /// Exact, case-sensitive reference gem label.
    pub label: String,
    /// Observed hull of every utterance inside the reference gem.
    pub reference_interval: std::ops::RangeInclusive<u64>,
    /// Unmodified selected donor interval.
    pub donor_interval: std::ops::RangeInclusive<u64>,
    /// Strict interval relation used for placement.
    pub exterior: GemExterior,
}

#[derive(Clone)]
pub(super) struct TimedGem {
    label: String,
    start: u64,
    end: u64,
}

impl TimedGem {
    pub(super) fn bind(reference: &ChatFile, label: &str) -> Result<Self, MergeError> {
        // A unique, explicitly paired gem is required. Nested markers are not
        // flattened, and an untimed row cannot disappear from the extent proof.
        let mut opening = None;
        let mut closing = None;
        for (index, line) in reference.lines.iter().enumerate() {
            let Line::Header { header, .. } = line else {
                continue;
            };
            // Select the one marker slot this header fills; a second marker
            // with the same label is refused whichever side it is on.
            let slot = match header.as_ref() {
                Header::BeginGem { label: Some(value) } if value.as_str() == label => &mut opening,
                Header::EndGem { label: Some(value) } if value.as_str() == label => &mut closing,
                _ => continue,
            };
            if slot.replace(index).is_some() {
                return Err(MergeError::InvalidGemExterior);
            }
        }
        let (Some(opening), Some(closing)) = (opening, closing) else {
            return Err(MergeError::InvalidGemExterior);
        };
        if opening >= closing {
            return Err(MergeError::InvalidGemExterior);
        }
        let mut extent: Option<(u64, u64)> = None;
        for line in &reference.lines[opening + 1..closing] {
            match line {
                Line::Utterance(row) => {
                    let Some(bullet) = row.main.content.bullet.as_ref() else {
                        return Err(MergeError::InvalidGemExterior);
                    };
                    let (start, end) = (bullet.timing.start_ms, bullet.timing.end_ms);
                    if start > end {
                        return Err(MergeError::InvalidGemExterior);
                    }
                    extent = Some(match extent {
                        None => (start, end),
                        Some((before, after)) => (before.min(start), after.max(end)),
                    });
                }
                Line::Header { header, .. }
                    if matches!(
                        header.as_ref(),
                        Header::BeginGem { .. } | Header::EndGem { .. } | Header::LazyGem { .. }
                    ) =>
                {
                    return Err(MergeError::InvalidGemExterior);
                }
                Line::Header { .. } => {}
            }
        }
        let Some((start, end)) = extent else {
            return Err(MergeError::InvalidGemExterior);
        };
        Ok(Self {
            label: label.into(),
            start,
            end,
        })
    }

    fn classify(&self, bullet: &Bullet) -> Option<GemExterior> {
        if bullet.timing.start_ms > bullet.timing.end_ms {
            return None;
        }
        if bullet.timing.end_ms < self.start {
            Some(GemExterior::Before)
        } else if bullet.timing.start_ms > self.end {
            Some(GemExterior::After)
        } else {
            None
        }
    }

    pub(super) fn precedes(&self, header: &Header, bullet: &Bullet) -> Option<bool> {
        match (header, self.classify(bullet)) {
            (Header::BeginGem { label: Some(label) }, Some(GemExterior::Before))
                if label.as_str() == self.label =>
            {
                Some(false)
            }
            (Header::EndGem { label: Some(label) }, Some(GemExterior::After))
                if label.as_str() == self.label =>
            {
                Some(true)
            }
            _ => None,
        }
    }

    pub(super) fn evidence(
        &self,
        donor: DonorIdx,
        bullet: &Bullet,
    ) -> Option<GemExteriorPlacement> {
        Some(GemExteriorPlacement {
            donor,
            label: self.label.clone(),
            reference_interval: self.start..=self.end,
            donor_interval: bullet.timing.start_ms..=bullet.timing.end_ms,
            exterior: self.classify(bullet)?,
        })
    }
}
