//! Caller-attested relative order, kept separate from acoustic timestamps.
use super::*;

/// A proposed cross-source ordering fact. Binding checks coordinates and
/// consistency, not the external evidence supporting the caller's assertion.
#[derive(Clone, Copy)]
pub enum RelativeOrderConstraint {
    /// The reference utterance precedes the selected donor utterance.
    ReferenceBefore {
        /// Ordinal in the bound reference source.
        reference: ReferenceIdx,
        /// Ordinal in the bound selected donor source.
        donor: DonorIdx,
    },
    /// The selected donor utterance precedes the reference utterance.
    DonorBefore {
        /// Ordinal in the bound selected donor source.
        donor: DonorIdx,
        /// Ordinal in the bound reference source.
        reference: ReferenceIdx,
    },
}

#[derive(Clone)]
pub(super) struct OrderConstraints {
    // Each reference has a closed interval of permissible donor insertion cuts.
    cuts: Vec<(usize, usize)>,
    proposals: Vec<RelativeOrderConstraint>,
}

impl OrderConstraints {
    pub(super) fn bind(
        references: usize,
        donors: usize,
        proposals: Vec<RelativeOrderConstraint>,
    ) -> Result<Self, MergeError> {
        let mut cuts = vec![(0, donors); references];
        for proposal in &proposals {
            let (reference, donor, before) = match *proposal {
                RelativeOrderConstraint::ReferenceBefore { reference, donor } => {
                    (reference.utterance().raw(), donor.utterance().raw(), true)
                }
                RelativeOrderConstraint::DonorBefore { reference, donor } => {
                    (reference.utterance().raw(), donor.utterance().raw(), false)
                }
            };
            let cut = cuts
                .get_mut(reference)
                .ok_or(MergeError::InvalidRelativeOrder)?;
            if donor >= donors {
                return Err(MergeError::InvalidRelativeOrder);
            }
            if before {
                cut.1 = cut.1.min(donor);
            } else {
                cut.0 = cut.0.max(donor + 1);
            }
        }
        let mut lower = 0;
        for cut in &mut cuts {
            lower = lower.max(cut.0);
            cut.0 = lower;
        }
        let mut upper = donors;
        for cut in cuts.iter_mut().rev() {
            upper = upper.min(cut.1);
            cut.1 = upper;
        }
        if cuts.iter().any(|(lower, upper)| lower > upper) {
            return Err(MergeError::InvalidRelativeOrder);
        }
        Ok(Self { cuts, proposals })
    }

    pub(super) fn precedes(&self, reference: MergeOrigin, donor: MergeOrigin) -> Option<bool> {
        let (MergeOrigin::Retained(reference), MergeOrigin::Inserted(donor)) = (reference, donor)
        else {
            return None;
        };
        let &(lower, upper) = self.cuts.get(reference.utterance().raw())?;
        if donor.utterance().raw() < lower {
            Some(false)
        } else if donor.utterance().raw() >= upper {
            Some(true)
        } else {
            None
        }
    }

    pub(super) fn verify(&self, origins: &[MergeOrigin]) -> Result<(), MergeError> {
        let mut references = std::collections::BTreeMap::new();
        let mut donors = std::collections::BTreeMap::new();
        for (position, origin) in origins.iter().enumerate() {
            match origin {
                MergeOrigin::Retained(index) => {
                    references.insert(index.utterance().raw(), position);
                }
                MergeOrigin::Inserted(index) => {
                    donors.insert(index.utterance().raw(), position);
                }
            }
        }
        for proposal in &self.proposals {
            let (reference, donor, before) = match *proposal {
                RelativeOrderConstraint::ReferenceBefore { reference, donor } => {
                    (reference, donor, true)
                }
                RelativeOrderConstraint::DonorBefore { reference, donor } => {
                    (reference, donor, false)
                }
            };
            let r = references
                .get(&reference.utterance().raw())
                .ok_or(MergeError::InvalidRelativeOrder)?;
            let d = donors
                .get(&donor.utterance().raw())
                .ok_or(MergeError::InvalidRelativeOrder)?;
            if (*r < *d) != before {
                return Err(MergeError::InvalidRelativeOrder);
            }
        }
        Ok(())
    }
}
