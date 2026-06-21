//! The seam between chatter (deterministic) and a model endpoint. The
//! HTTP-backed implementation lives in the `talkbank-llm` crate; tests use
//! an in-crate mock. This keeps `talkbank-transform` model-free.

use super::output::HolisticJudgment;
use super::request::JudgmentRequest;

/// Why a judgment could not be produced. Provider-agnostic on purpose: the
/// HTTP crate maps its transport/parse failures into these variants.
#[derive(Debug, thiserror::Error)]
pub enum JudgmentError {
    /// The provider (network, endpoint, auth) failed before a usable
    /// response was obtained.
    #[error("judgment provider failed: {0}")]
    Provider(String),
    /// A response was obtained but did not parse into a [`HolisticJudgment`].
    #[error("malformed judgment response: {0}")]
    MalformedResponse(String),
}

/// A source of holistic judgments. One call per session.
pub trait JudgmentProvider {
    /// Produce a judgment for `request`, or a typed error. Implementations
    /// must not panic.
    fn judge(&self, request: &JudgmentRequest) -> Result<HolisticJudgment, JudgmentError>;
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::speaker_id::judgment::output::{
        AdultRole, DonorCode, SampleTypeVerdict, SpeakerVerdict,
    };
    use crate::speaker_id::judgment::request::{JudgmentRequest, SessionId};
    use talkbank_model::SpeakerCode;

    struct CannedProvider(HolisticJudgment);
    impl JudgmentProvider for CannedProvider {
        fn judge(&self, _req: &JudgmentRequest) -> Result<HolisticJudgment, JudgmentError> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn provider_is_object_safe_and_returns_canned() {
        let canned = HolisticJudgment {
            speaker_mapping: BTreeMap::from([(DonorCode("PAR1".into()), SpeakerVerdict::Adult)]),
            adult_roles: BTreeMap::from([(DonorCode("PAR1".into()), AdultRole::Inv)]),
            sample_type: SampleTypeVerdict::Confirmed,
            merge_applicable: true,
            confidence: BTreeMap::new(),
            reasoning: "canned".into(),
        };
        let provider: Box<dyn JudgmentProvider> = Box::new(CannedProvider(canned));
        let req = JudgmentRequest {
            session_id: SessionId("s1".into()),
            sample_type: None,
            declared_roles: Vec::new(),
            consent_tier: None,
            age_months: None,
            anchor: SpeakerCode::new("CHI"),
            samples: Vec::new(),
        };
        let out = provider.judge(&req).expect("canned judge");
        assert!(out.merge_applicable);
    }
}
