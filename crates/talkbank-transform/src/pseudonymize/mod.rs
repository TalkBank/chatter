//! Selective name-pseudonymization boundaries.
//!
//! [`NameMap`] admits a private runtime mapping without echoing its contents
//! in errors or debug output. Document admission, source-bound planning and
//! [`PseudonymizationInput::prepare_output`] establish distinct input/review/output
//! states. Accepted output is validated and stable under a follow-up plan, not
//! certified fully de-identified. Private receipt persistence and CLI integration
//! remain separate unfinished requirements; nothing is written automatically.
//!
//! The versioned runtime wire format is TOML:
//!
//! ```toml
//! version = 1
//! [[transcripts]]
//! key = "sample"
//! [[transcripts.names]]
//! original = "Rose"
//! replacement = "PersonA"
//! ```
//!
//! Keep this input private. Keys are exact caller-provided identities, not
//! automatically inferred basenames. Multiple names may share a placeholder,
//! but a placeholder must not itself match any source name in that transcript.
//! Mapping admission proves neither that a file has matches nor that all
//! identifying content has been found.

mod free_text;
mod header_fields;
mod input;
mod lexical_source;
mod morphology;
mod name_map;
mod output;
mod phonology;
mod plan;
mod timing;
mod word;

pub use free_text::{
    FreeTextDecision, FreeTextFinding, FreeTextOwner, FreeTextRefusal, ProseLocation,
};
pub use header_fields::{
    HeaderFieldLocation, HeaderFieldRefusal, HeaderFieldReview, HeaderNameField,
};
pub use input::{InputRefusal, PseudonymizationInput};
pub use lexical_source::{LexicalEdit, LexicalSourceRefusal};
pub use morphology::{
    LemmaFinding, LemmaFindingKind, LemmaLocation, LemmaPart, LemmaSource, MorphologyOutcome,
    MorphologyRefusal, MorphologyReview,
};
pub use name_map::{NameDecision, NameMap, NameMapError, TranscriptNames};
pub use output::{
    AppliedEdit, EditKind, EditOrigin, OutputRefusal, OutputRefusalReason, PseudonymizedDocument,
};
pub use phonology::{PronunciationEvidence, PronunciationRefusal};
pub use plan::{LexicalPlan, RefusedWord, WordLocation, WordPreview, WordSpelling};
pub use timing::{TimingOutcome, TimingRefusal, TimingReview, TimingWordPreview};
pub use word::{ComponentDecision, WordRefusal};
