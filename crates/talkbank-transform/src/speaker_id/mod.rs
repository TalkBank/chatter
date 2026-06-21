//! Speaker identification, rewrite a CHAT file's speaker codes
//! per an operator-supplied mapping, or pick the mapping by text
//! similarity against a reference transcript.
//!
//! Currently implements explicit-mapping mode and the text-similarity
//! "identify" step of reference mode. Override-file mode is defined
//! in the user contract but not yet implemented.
//!
//! See `book/src/chatter/user-guide/speaker-id.md` for the user
//! contract.

mod apply;
mod error;
mod identify;
pub mod judgment;
mod mapping;
mod override_file;
mod provenance;
mod types;

pub use apply::{apply_mapping, apply_mapping_chat};
pub use error::SpeakerIdError;
pub use identify::{DEFAULT_CONFIDENCE_THRESHOLD, DonorMatchReport, identify_mapping};
pub use judgment::{
    AdultRole, AgeMonths, BlankLabelError, CURRENT_PROMPT_VERSION, ChatMessage, ConsentTierLabel,
    ConsumeError, DonorCode, HolisticJudgment, JudgmentContext, JudgmentError, JudgmentProvider,
    JudgmentRequest, LabelKind, ProvenanceMeta, Role, RoleLabel, SampleBudget, SampleTypeLabel,
    SampleTypeVerdict, SampledUtterance, SessionContextError, SessionContextFile,
    SessionContextRecord, SessionId, SpeakerSamples, SpeakerVerdict, judgment_to_pending,
    render_messages, sample_session, session_context,
};
pub use mapping::{MappingSpec, SpeakerAssignment, parse_mapping_spec};
pub use override_file::{
    CURRENT_SCHEMA_VERSION, InsertedRoleSpec, MergeOverride, OverrideFile, OverrideFileError,
    OverrideMode, SpeakerAction,
};
pub use provenance::{
    Confidence, ConfidenceField, DecisionEngine, EndpointUrl, JudgmentProvenance, ModelId,
    PromptVersion,
};
pub use types::{ConfidenceMargin, ConfidenceThreshold, JaccardScore};
