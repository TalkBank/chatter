//! The holistic LLM judgment: typed output, typed input, the provider seam,
//! deterministic sampling, prompt rendering, and deterministic consumption.
//! `talkbank-transform` stays model-free; the HTTP implementation of the
//! provider trait lives in the `talkbank-llm` crate.

pub mod consume;
pub mod context;
pub mod output;
pub mod prompt;
pub mod provider;
pub mod request;
pub mod sample;
pub mod session_context;

pub use consume::{ConsumeError, ProvenanceMeta, judgment_to_pending};
pub use context::{JudgmentContext, session_context};
pub use output::{AdultRole, DonorCode, HolisticJudgment, SampleTypeVerdict, SpeakerVerdict};
pub use prompt::{CURRENT_PROMPT_VERSION, ChatMessage, Role, render_messages};
pub use provider::{JudgmentError, JudgmentProvider};
pub use request::{JudgmentRequest, SampledUtterance, SessionId, SpeakerSamples};
pub use sample::{SampleBudget, sample_session};
pub use session_context::{
    AgeMonths, BlankLabelError, ConsentTierLabel, LabelKind, RoleLabel, SampleTypeLabel,
    SessionContextError, SessionContextFile, SessionContextRecord,
};
