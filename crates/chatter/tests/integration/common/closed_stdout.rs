//! A standard output whose only reader is already closed when the child starts.
//!
//! # Why the pipe is not made in the test process
//!
//! The obvious construction, `std::io::pipe()` and then dropping the reader
//! before spawning, is not a closed pipe in a test binary that spawns children
//! from many threads at once. The reader is closed only when its LAST copy is
//! closed, and two routes hand a copy to an unrelated child of a concurrent
//! test:
//!
//! - On macOS, which has no `pipe2`, std creates the pipe with `pipe(2)` and
//!   then sets `FD_CLOEXEC` with a separate `fcntl(2)`. A `posix_spawn` on
//!   another thread between the two calls inherits the reader as an ordinary
//!   descriptor and keeps it for that child's whole life. A loaded CI runner
//!   preempts the thread inside that gap often enough to matter.
//! - On every Unix, a sibling spawned through `fork` (any command with a
//!   `pre_exec` hook) holds every descriptor of the test process, close-on-exec
//!   or not, until its `exec`.
//!
//! While any such copy is open, the program's writes succeed (the pipe has a
//! reader and spare buffer), so it correctly exits 0, and the closed-stdout
//! contract looks broken when the fixture was. Here the pipe is created inside
//! the forked child, after `fork` and before `exec`: its reader never exists in
//! the test process, so no other process can ever hold it, and the child closes
//! it before the program's first instruction runs.
//!
//! On Windows std creates pipe handles non-inheritable and makes only the
//! spawned child's stdio handles inheritable, under its process-creation lock,
//! so a concurrent spawn cannot obtain the reader and the plain construction is
//! sound there.

use std::process::Command;

/// Give `command` a standard output whose only reader is closed before the
/// program starts, so its first write to stdout fails with a broken pipe.
///
/// On Unix the descriptor is installed after std's own stdio setup, so a later
/// `stdout(...)` on the same command cannot replace it.
pub fn attach(command: &mut Command) -> &mut Command {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Descriptor 1 is open (on the null device) before the hook runs, so
        // `pipe(2)`, which returns the lowest free descriptors, can never
        // return 1 itself: the `dup2` always replaces it and both closes leave
        // it intact.
        command.stdout(std::process::Stdio::null());
        // SAFETY: between `fork` and `exec` the hook calls only `pipe`, `dup2`
        // and `close`, all async-signal-safe, and touches no Rust allocation.
        unsafe {
            command.pre_exec(|| {
                let mut ends = [0; 2];
                if libc::pipe(ends.as_mut_ptr()) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                let [reader, writer] = ends;
                if libc::close(reader) == -1
                    || libc::dup2(writer, libc::STDOUT_FILENO) == -1
                    || libc::close(writer) == -1
                {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        command
    }
    #[cfg(not(unix))]
    {
        // The one sanctioned use: on Windows the reader cannot leak (see the
        // module documentation).
        #[allow(clippy::disallowed_methods)]
        let (reader, writer) = std::io::pipe().expect("stdout pipe");
        drop(reader);
        command.stdout(writer)
    }
}
