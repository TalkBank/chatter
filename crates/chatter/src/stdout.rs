//! Standard output: the one route every text writer takes.
//!
//! `println!` and `print!` panic when standard output is closed, as it is
//! when a consumer such as `head` stops reading. Every text writer here writes
//! through [`outln!`] and [`out!`] instead, which call [`write`]: a failed
//! write ends the command with [`EXIT_OUTPUT_INCOMPLETE`], the status
//! `validate --format json` gives when its consumer closes the pipe, silently
//! for a closed pipe (the consumer chose to stop) and with one line on stderr
//! for any other failure. No panic, and no run that failed exits 0 because
//! its summary had nowhere to go.
//!
//! [`EXIT_OUTPUT_INCOMPLETE`]: crate::exit_codes::EXIT_OUTPUT_INCOMPLETE

use std::io::Write;

/// Write `args` to standard output, or end the command if it cannot be
/// written (see the module documentation).
pub fn write(args: std::fmt::Arguments<'_>) {
    if let Err(error) = std::io::stdout().lock().write_fmt(args) {
        closed(error)
    }
}

/// End the command because standard output failed with `error`: silently
/// for a closed pipe, with one line on stderr otherwise, and in both cases
/// with [`EXIT_OUTPUT_INCOMPLETE`](crate::exit_codes::EXIT_OUTPUT_INCOMPLETE).
/// For a writer that holds its own handle (a whole document written at once).
pub fn closed(error: std::io::Error) -> ! {
    if error.kind() != std::io::ErrorKind::BrokenPipe {
        eprintln!("chatter: cannot write to standard output: {error}");
    }
    std::process::exit(crate::exit_codes::EXIT_OUTPUT_INCOMPLETE)
}

/// `println!` through [`write`]: a closed standard output ends the command
/// instead of panicking.
macro_rules! outln {
    () => {
        $crate::stdout::write(format_args!("\n"))
    };
    ($($arg:tt)*) => {
        $crate::stdout::write(format_args!("{}\n", format_args!($($arg)*)))
    };
}

/// `print!` through [`write`]: a closed standard output ends the command
/// instead of panicking.
macro_rules! out {
    ($($arg:tt)*) => {
        $crate::stdout::write(format_args!($($arg)*))
    };
}
