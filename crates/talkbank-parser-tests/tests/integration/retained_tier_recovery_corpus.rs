//! Canonical source-derived tier recovery across parsing, JSON and transforms.
//!
//! These contracts retain their authored/reference inputs and independent
//! recovery expectations. A producer fault cannot count as successful detection
//! of invalid CHAT, even when its presentation severity has been downgraded.

use talkbank_model::{CompletedDiagnostics, ErrorCollector};

fn completed(errors: ErrorCollector) -> CompletedDiagnostics {
    CompletedDiagnostics::admit(errors.into_vec())
        .expect("source workflow must not fail internally")
}

#[path = "gra_recovery_from_source.rs"]
mod gra_recovery;
#[path = "mor_recovery_from_source.rs"]
mod mor_recovery;
#[path = "phon_recovery_from_source.rs"]
mod phon_recovery;
#[path = "phonology_recovery_from_source.rs"]
mod phonology_recovery;
#[path = "text_recovery_from_source.rs"]
mod text_recovery;
#[path = "wor_recovery_from_source.rs"]
mod wor_recovery;
