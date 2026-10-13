//! Catalog eligibility is distinct from absent repair policy and source faults.

/// Why an actual diagnosed site does not license this repair.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CatalogIneligibility {
    /// No required generated carrier encloses the diagnosis.
    #[error("the required source carrier is absent")]
    MissingCarrier,
    /// The local carrier contains parser recovery.
    #[error("the repair carrier contains recovery")]
    RecoveredCarrier,
    /// The diagnosis does not select the complete required token.
    #[error("the diagnosis does not select the complete repair token")]
    TokenMismatch,
    /// Source spelling does not have the catalog's proposed repair.
    #[error("the source token does not need this repair")]
    NoRepairableToken,
    /// More than one eligible source carrier would require a choice.
    #[error("the repair carrier is not unique")]
    NonUniqueCarrier,
    /// Removing the main tier would affect its dependent tiers.
    #[error("the utterance also owns dependent tiers")]
    DependentTiers,
    /// Headers disagree or do not establish an earlier identical declaration.
    #[error("no earlier identical header without conflicting declarations")]
    HeaderConflict,
    /// A media token is invalid, remote or already canonical.
    #[error("the media token does not license normalization")]
    MediaToken,
    /// Whitespace is not exactly the catalog's group-edge separator.
    #[error("the diagnosis is not the owned group-edge whitespace")]
    GroupEdge,
}

/// A source capability failed, rather than a CHAT repair being unsupported.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CatalogFault {
    /// Generated traversal refused its source, grammar or admitted-kind evidence.
    #[error(transparent)]
    SourceCapability(#[from] talkbank_parser::CstFailure),
    /// A source coordinate exceeds the existing representation.
    #[error("source coordinate {bytes} does not fit in u32")]
    CoordinateExtent {
        /// The original, unnarrowed byte coordinate.
        bytes: usize,
    },
}

pub(super) enum CatalogRefusal {
    Ineligible(CatalogIneligibility),
    Fault(CatalogFault),
}

impl From<CatalogIneligibility> for CatalogRefusal {
    fn from(reason: CatalogIneligibility) -> Self {
        Self::Ineligible(reason)
    }
}
impl From<talkbank_parser::generated_traversal::SourceBindingError> for CatalogRefusal {
    fn from(fault: talkbank_parser::generated_traversal::SourceBindingError) -> Self {
        Self::Fault(CatalogFault::SourceCapability(fault.into()))
    }
}

/// The catalog's complete resolution of an owner-issued finding.
#[derive(Debug)]
pub enum CatalogResolution {
    /// Existing policy offers raw candidate parts, not application authority.
    Offered(super::CatalogFix),
    /// This code has no repair policy; no content is guessed.
    Unsupported,
    /// The actual source carrier does not license the existing repair.
    Ineligible(CatalogIneligibility),
    /// The actual parse did not retain source structure for catalog traversal.
    SourceUnavailable,
    /// Source evidence itself failed, not a recoverable CHAT repair policy.
    Fault(CatalogFault),
}

impl CatalogResolution {
    /// Consume into intentional raw candidate inspection, discarding the reason.
    /// This cannot issue a selected or application result.
    pub fn into_offer(self) -> Option<super::CatalogFix> {
        match self {
            Self::Offered(fix) => Some(fix),
            Self::Unsupported | Self::Ineligible(_) | Self::SourceUnavailable | Self::Fault(_) => {
                None
            }
        }
    }
}
