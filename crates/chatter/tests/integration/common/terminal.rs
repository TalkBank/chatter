//! Owned PTY sessions; input stays open until the child completes.

use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, ExitStatus, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// An admitted terminal pair, including the original line-discipline flags.
pub struct Pty {
    pub master: File,
    pub slave: File,
    pub original_flags: libc::tcflag_t,
}

impl Pty {
    pub fn open() -> Self {
        let mut master = std::mem::MaybeUninit::uninit();
        let mut slave = std::mem::MaybeUninit::uninit();
        let mut size = libc::winsize {
            ws_row: 24,
            ws_col: 100,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: openpty initializes both descriptors on success; no names or
        // termios pointer are requested. Each descriptor is then owned once.
        let result = unsafe {
            libc::openpty(
                master.as_mut_ptr(),
                slave.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                // libc declares this pointer const on Linux and mutable on
                // Darwin. A mutable raw pointer type-checks against both ABIs.
                &raw mut size,
            )
        };
        assert_eq!(result, 0, "openpty: {}", std::io::Error::last_os_error());
        let master = unsafe { File::from_raw_fd(master.assume_init()) };
        let slave = unsafe { File::from_raw_fd(slave.assume_init()) };
        let original_flags = terminal_flags(&slave);
        assert_eq!(
            terminal_flags(&master),
            original_flags,
            "both ends observe the same terminal"
        );
        Self {
            master,
            slave,
            original_flags,
        }
    }

    pub fn launch(
        self,
        mut command: std::process::Command,
        output: TerminalOutput,
    ) -> RunningTerminal {
        command
            .stdin(self.slave.try_clone().unwrap())
            .stderr(self.slave.try_clone().unwrap());
        match output {
            TerminalOutput::Terminal => {
                command.stdout(self.slave.try_clone().unwrap());
            }
            TerminalOutput::Closed => {
                let (reader, writer) = std::io::pipe().unwrap();
                drop(reader);
                command.stdout(Stdio::from(writer));
            }
        }
        // SAFETY: only async-signal-safe system calls run between fork and exec.
        // The child has its own session and stdin is already the slave terminal.
        unsafe {
            command.pre_exec(|| {
                // Darwin exposes the request as u32; Linux uses c_ulong.
                #[allow(clippy::useless_conversion)]
                let request: libc::c_ulong = libc::TIOCSCTTY.into();
                if libc::setsid() == -1 || libc::ioctl(0, request, 0) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command.spawn().unwrap();
        let mut reader = self.master.try_clone().unwrap();
        let (sender, output) = channel();
        std::thread::spawn(move || {
            let mut buffer = [0; 4096];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(count) => {
                        if sender.send(buffer[..count].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        RunningTerminal {
            child,
            pty: self,
            output,
        }
    }
}

pub fn terminal_flags(file: &File) -> libc::tcflag_t {
    let mut attributes = std::mem::MaybeUninit::uninit();
    // SAFETY: tcgetattr writes termios on success; the File owns a live PTY fd.
    assert_eq!(
        unsafe { libc::tcgetattr(file.as_raw_fd(), attributes.as_mut_ptr()) },
        0,
        "tcgetattr: {}",
        std::io::Error::last_os_error()
    );
    let attributes = unsafe { attributes.assume_init() };
    attributes.c_lflag & (libc::ICANON | libc::ECHO | libc::ISIG)
}

pub enum TerminalOutput {
    Terminal,
    Closed,
}

/// Own the child and its PTY together so failed assertions cannot leave a live UI.
pub struct RunningTerminal {
    child: Child,
    pub pty: Pty,
    output: Receiver<Vec<u8>>,
}

impl RunningTerminal {
    /// Supply input only after the child has presented its prompt.
    pub fn answer_after(mut self, prompt: &[u8], answer: &[u8]) -> Self {
        self.wait_for(prompt);
        self.pty.master.write_all(answer).unwrap();
        self
    }

    pub fn wait_for(&self, marker: &[u8]) {
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut seen = Vec::new();
        loop {
            seen.extend(
                self.output
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .unwrap_or_else(|error| {
                        panic!(
                            "waiting for {marker:?}: {error}; {}",
                            String::from_utf8_lossy(&seen)
                        )
                    }),
            );
            if seen.windows(marker.len()).any(|window| window == marker) {
                return;
            }
        }
    }

    pub fn wait(&mut self) -> ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            assert!(Instant::now() < deadline, "TUI did not return");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn assert_restored(&self) {
        // Darwin revokes the slave descriptor when its controlling session
        // ends. The retained master observes the same terminal without reopening
        // (and potentially reinitializing) the slave.
        assert_eq!(terminal_flags(&self.pty.master), self.pty.original_flags);
    }
}

impl Drop for RunningTerminal {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
