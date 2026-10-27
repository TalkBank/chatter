//! Fallible standard output shared by text command renderers.
//!
//! Every write returns through the command's typed result. The macros propagate
//! failure immediately, so a closed pipe cannot permit later mutations. Owned
//! stages and terminal guards are dropped before the program chooses its status.

use std::io::Write;

/// Write one formatted fragment or return its output failure to the caller.
pub fn write(args: std::fmt::Arguments<'_>) -> crate::exit_codes::CommandResult {
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_fmt(args)
        .and_then(|()| stdout.flush())
        .map_err(crate::exit_codes::CommandStop::Output)
}

/// Write a line and immediately propagate a failed write.
macro_rules! outln {
    () => {
        $crate::stdout::write(format_args!("\n"))?
    };
    ($($arg:tt)*) => {
        $crate::stdout::write(format_args!("{}\n", format_args!($($arg)*)))?
    };
}

/// Write a fragment and immediately propagate a failed write.
macro_rules! out {
    ($($arg:tt)*) => {
        $crate::stdout::write(format_args!($($arg)*))?
    };
}
