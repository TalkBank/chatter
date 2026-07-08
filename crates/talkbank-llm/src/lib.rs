//! OpenAI-compatible HTTP implementation of `talkbank_transform`'s
//! `JudgmentProvider`. This is the only crate in the toolchain that makes a
//! network call to a model endpoint; `talkbank-transform` stays model-free.
//!
//! The provider renders the deterministic prompt (via
//! `talkbank_transform::speaker_id::render_messages`), POSTs it to an
//! OpenAI-compatible `/chat/completions` endpoint with `temperature = 0` and
//! a JSON-object response format, and parses the single returned message
//! content into a `HolisticJudgment`. Every failure mode is mapped to the
//! provider-agnostic `JudgmentError` so callers never see a transport- or
//! parser-specific type.


// Unit-test modules: panic-family clippy lints relaxed by policy
// (see the workspace [lints] table for the production deny).
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented,
    )
)]

#![warn(missing_docs)]

mod cache;
mod http_provider;

pub use cache::{CacheError, CachePath, ResponseCache};
pub use http_provider::{
    ApiKey, HttpJudgmentProvider, HttpProviderConfig, RetryCount, TimeoutSecs,
};
