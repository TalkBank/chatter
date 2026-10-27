//! Typed command completion and the process exit contract.

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
                ReportedFailure::Input => 1,
                ReportedFailure::Precondition => 2,
                ReportedFailure::Internal => 3,
                ReportedFailure::NeedsReview => 4,
                ReportedFailure::Interrupted => 130,
            },
            Self::Usage(error) => {
                if let Err(error) = error.print() {
                    return Self::Output(error).finish();
                }
                2
            }
            Self::MachineOutput(_error) => 1,
            Self::Output(error) => {
                if error.kind() != std::io::ErrorKind::BrokenPipe {
                    eprintln!("chatter: cannot write to standard output: {error}");
                }
                1
            }
        };
        std::process::ExitCode::from(code)
    }
}
