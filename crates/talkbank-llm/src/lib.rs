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

#![warn(missing_docs)]

mod http_provider;

pub use http_provider::{
    ApiKey, HttpJudgmentProvider, HttpProviderConfig, RetryCount, TimeoutSecs,
};
