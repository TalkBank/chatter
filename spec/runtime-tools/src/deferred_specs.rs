//! Deferred code status does not suspend legal or subsumption claims.

use generators::spec::error::ErrorSpec;
use generators::spec::metadata::{SpecErrorCode, Status};
use talkbank_spec_vocabulary::frontmatter::Claim;

use crate::error_spec_validation::emit_for;

enum ClaimEvidence {
    Planned,
    Verified,
}

struct ReviewedExample {
    source: String,
    ordinal: usize,
    status: Status,
    claim: Claim,
    codes: Vec<String>,
    evidence: ClaimEvidence,
}

/// A report whose deferred codes are absent and whose executable claims hold.
///
/// Only admission can construct it. Both the command and the test gate consume
/// this same result, rather than interpreting diagnostic observations separately.
pub struct ReviewedDeferred {
    examples: Vec<ReviewedExample>,
}

impl ReviewedDeferred {
    /// Observe deferred examples and admit the complete report, or retain every
    /// contradiction in the refusal. Planned violations do not reactivate codes.
    pub fn from_specs(specs: &[ErrorSpec]) -> Result<Self, String> {
        let parser = talkbank_parser::TreeSitterParser::new().map_err(|e| e.to_string())?;
        let mut examples = Vec::new();
        let mut failures = Vec::new();
        for spec in specs {
            if spec.status() == Status::Implemented {
                continue;
            }
            let own = &spec.error.code;
            for (index, example) in spec.error.examples.iter().enumerate() {
                let codes = emit_for(&parser, example).all_distinct_codes();
                let fired =
                    |code: &SpecErrorCode| codes.iter().any(|observed| observed == code.as_str());
                let evidence = if fired(own) {
                    Err(format!("deferred code {own} already emitted"))
                } else {
                    match &example.claim {
                        Claim::Violates => Ok(ClaimEvidence::Planned),
                        Claim::Legal | Claim::SubsumedBy(_) => {
                            if example.claim.satisfied_by(own, fired) {
                                Ok(ClaimEvidence::Verified)
                            } else {
                                Err(format!("claim CONTRADICTED: {:?}", example.claim))
                            }
                        }
                    }
                };
                match evidence {
                    Ok(evidence) => examples.push(ReviewedExample {
                        source: spec.source_file().to_owned(),
                        ordinal: index + 1,
                        status: spec.status(),
                        claim: example.claim.clone(),
                        codes,
                        evidence,
                    }),
                    Err(reason) => failures.push(format!(
                        "{} ex{}: {reason}; observed: {}",
                        spec.source_file(),
                        index + 1,
                        codes.join(", ")
                    )),
                }
            }
        }
        if failures.is_empty() {
            Ok(Self { examples })
        } else {
            Err(format!(
                "{} deferred spec discrepancy/discrepancies require adjudication:\n  {}",
                failures.len(),
                failures.join("\n  ")
            ))
        }
    }
}

impl std::fmt::Display for ReviewedDeferred {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (mut verified, mut planned) = (0, 0);
        for example in &self.examples {
            let evidence = match example.evidence {
                ClaimEvidence::Verified => {
                    verified += 1;
                    "claim verified"
                }
                ClaimEvidence::Planned => {
                    planned += 1;
                    "planned claim"
                }
            };
            writeln!(
                f,
                "  {:<44} ex{} {:<16} own code absent; {evidence}; claim: {:?}; emits: {}",
                example.source,
                example.ordinal,
                example.status,
                example.claim,
                if example.codes.is_empty() {
                    "nothing".to_owned()
                } else {
                    example.codes.join(", ")
                },
            )?;
        }
        write!(
            f,
            "\n  {verified} verified legal/subsumption claims; {planned} planned violation claims; no contradictions.\n  \
             Deferred code status is not a count of missing validation rules.\n  \
             Verified claims do not implement or reactivate their historical codes."
        )
    }
}
