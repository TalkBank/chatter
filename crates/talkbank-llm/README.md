# talkbank-llm

**Last modified:** 2026-09-28 20:59 EDT

HTTP implementation of the `talkbank-transform` judgment-provider interface.
The Chatter CLI uses this crate for explicitly requested model-assisted work;
ordinary CHAT parsing and validation remain in the shared, model-free crates.

`HttpJudgmentProvider` uses `HttpProviderConfig` and the typed `ApiKey`,
`RetryCount` and `TimeoutSecs` configuration values. `ResponseCache` and
`CachePath` support response caching. Transport and response failures are
returned through the transform's provider error contract.

Model-assisted requests send their supplied content to the configured endpoint.
Review data-sharing permissions before enabling them. Do not put API keys or
private transcript content in bug reports or checked-in configuration.

This crate is included in the CLI's publication dependency closure; it is not a
standalone executable. See the [Chatter book](https://talkbank.github.io/chatter/)
and generated Rust API documentation for integration details. Pre-1.0 API changes
are recorded in the repository changelog.
