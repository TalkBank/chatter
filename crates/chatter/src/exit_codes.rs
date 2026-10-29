//! Typed command completion and the process exit contract.

/// The process statuses shared by execution, human help and machine discovery.
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum ExitStatus {
    /// The selected operation completed.
    Success = 0,
    /// Input, publication or output failed, or the operation is incomplete.
    Failed = 1,
    /// Arguments or operation prerequisites were refused.
    Usage = 2,
    /// An implementation or startup fault occurred.
    Internal = 3,
    /// Evidence awaits an operator decision.
    NeedsReview = 4,
    /// The operator forced termination.
    Interrupted = 130,
}

/// A status description whose numeric value comes from its typed owner.
#[derive(serde::Serialize)]
pub(crate) struct ExitDescription {
    status: ExitStatus,
    code: u8,
    meaning: &'static str,
}

impl ExitStatus {
    const ALL: [Self; 6] = [
        Self::Success,
        Self::Failed,
        Self::Usage,
        Self::Internal,
        Self::NeedsReview,
        Self::Interrupted,
    ];

    const fn meaning(self) -> &'static str {
        match self {
            Self::Success => "Selected operation completed; inspect results and validation policy",
            Self::Failed => "Invalid input, failed publication/output, or incomplete operation",
            Self::Usage => "Invalid arguments or unmet operation prerequisites",
            Self::Internal => "Internal implementation or startup failure",
            Self::NeedsReview => "Operator review required; no accepted final result",
            Self::Interrupted => "Forced interruption requested by the operator",
        }
    }

    pub(crate) fn descriptions() -> [ExitDescription; 6] {
        Self::ALL.map(|status| ExitDescription {
            status,
            code: status as u8,
            meaning: status.meaning(),
        })
    }

    pub(crate) fn help() -> String {
        let mut help = String::from("Exit codes:\n");
        for status in Self::ALL {
            help.push_str(&format!("  {:<4} {}\n", status as u8, status.meaning()));
        }
        help
    }

    /// Return this status at the outer process boundary.
    pub fn finish(self) -> std::process::ExitCode {
        std::process::ExitCode::from(self as u8)
    }
}

/// A command either completes or returns a classified stop to the program
/// boundary. No command owns process termination, so owned stages and terminal
/// guards are dropped before the exit status is returned to the operating system.
pub type CommandResult<T = ()> = Result<T, CommandStop>;

/// Failures already explained by the command's domain-specific renderer.
#[derive(Debug, Clone, Copy)]
pub enum ReportedFailure {
    /// Invalid or unreadable input, failed publication, or an incomplete run (1).
    Input,
    /// Usage or semantic precondition refused before the operation (2).
    Precondition,
    /// Startup or encoding failed inside the implementation (3).
    Internal,
    /// Pending evidence needs an operator decision (4).
    NeedsReview,
    /// The operator requested forced termination (130).
    Interrupted,
}

/// A reported domain failure or an error whose presentation belongs to the
/// program boundary. Keeping these separate prevents duplicate diagnostics.
#[derive(Debug)]
pub enum CommandStop {
    /// A diagnostic was already rendered in its domain-specific form.
    Reported(ReportedFailure),
    /// Clap owns a usage diagnostic that still needs presentation.
    Usage(clap::Error),
    /// Human-readable or artifact stdout failed.
    Output(std::io::Error),
    /// Machine output failed; its protocol promises empty stderr.
    MachineOutput(std::io::Error),
}

impl From<ReportedFailure> for CommandStop {
    fn from(failure: ReportedFailure) -> Self {
        Self::Reported(failure)
    }
}

impl CommandStop {
    /// Render once, after command resources have been released.
    pub fn finish(self) -> std::process::ExitCode {
        let code = match self {
            Self::Reported(failure) => match failure {
                ReportedFailure::Input => ExitStatus::Failed,
                ReportedFailure::Precondition => ExitStatus::Usage,
                ReportedFailure::Internal => ExitStatus::Internal,
                ReportedFailure::NeedsReview => ExitStatus::NeedsReview,
                ReportedFailure::Interrupted => ExitStatus::Interrupted,
            },
            Self::Usage(error) => {
                if let Err(error) = error.print() {
                    return Self::Output(error).finish();
                }
                ExitStatus::Usage
            }
            Self::MachineOutput(_error) => ExitStatus::Failed,
            Self::Output(error) => {
                if error.kind() != std::io::ErrorKind::BrokenPipe {
                    eprintln!("chatter: cannot write to standard output: {error}");
                }
                ExitStatus::Failed
            }
        };
        code.finish()
    }
}
