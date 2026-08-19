use super::{filter_unsafe_ansi, ExecutionRequest, ExecutionResult};
use crate::limits::BoundedText;
use anyhow::{Context, Result};
use nix::{
    fcntl::{fcntl, FcntlArg, OFlag},
    pty::{openpty, Winsize},
    sys::signal::{killpg, Signal},
    unistd::Pid,
};
use std::{
    fs::File,
    io::{ErrorKind, Read, Write},
    os::fd::{FromRawFd, IntoRawFd},
    os::unix::process::CommandExt,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

/// Executes through a real Unix PTY. The slave is the child's controlling
/// terminal and all three standard streams; output therefore arrives merged.
pub async fn execute(req: ExecutionRequest) -> Result<ExecutionResult> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancelled = cancelled.clone();
    let mut worker = tokio::task::spawn_blocking(move || execute_blocking(req, worker_cancelled));
    tokio::select! {
        result = &mut worker => result.context("PTY worker panicked or was cancelled")?,
        signal = tokio::signal::ctrl_c() => {
            signal?;
            cancelled.store(true, Ordering::Release);
            worker.await.context("PTY worker panicked after cancellation")?
        }
    }
}

fn execute_blocking(req: ExecutionRequest, cancelled: Arc<AtomicBool>) -> Result<ExecutionResult> {
    let _interactive_guard = if req.interactive {
        Some(InteractiveTerminalGuard::enter(req.tui_active)?)
    } else {
        None
    };
    let size = local_winsize();
    let pair = openpty(Some(&size), None).context("openpty failed")?;
    let mut master = File::from(pair.master);
    let slave = File::from(pair.slave);
    let stdin = slave.try_clone().context("clone PTY slave for stdin")?;
    let stdout = slave.try_clone().context("clone PTY slave for stdout")?;
    let raw = master.into_raw_fd();
    let flags = OFlag::from_bits_truncate(fcntl(raw, FcntlArg::F_GETFL)?);
    fcntl(raw, FcntlArg::F_SETFL(flags | OFlag::O_NONBLOCK))?;
    master = unsafe { File::from_raw_fd(raw) };

    let mut command = Command::new(&req.program);
    command
        .args(&req.args)
        .stdin(Stdio::from(stdin))
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(slave));
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::ioctl(libc::STDIN_FILENO, libc::TIOCSCTTY as _, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().context("spawn command on PTY")?;
    let pid = child.id();
    let started = Instant::now();
    let mut bytes = BoundedText::new(req.capture_max_bytes);
    let mut buffer = [0_u8; 8192];
    let mut timed_out = false;
    let mut interrupted = false;
    let mut last_size = size;
    let status = loop {
        match master.read(&mut buffer) {
            Ok(0) => {}
            Ok(n) => {
                bytes.push(&buffer[..n]);
                if req.interactive {
                    std::io::stdout().write_all(&buffer[..n])?;
                    std::io::stdout().flush()?;
                } else {
                    req.output
                        .stdout(&filter_unsafe_ansi(&String::from_utf8_lossy(&buffer[..n])));
                }
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            Err(error) if error.raw_os_error() == Some(libc::EIO) => {}
            Err(error) => return Err(error).context("read PTY master"),
        }
        if let Some(status) = child.try_wait().context("wait for PTY child")? {
            break Some(status);
        }
        if req.interactive {
            forward_available_stdin(&mut master)?;
            let size = local_winsize();
            if size.ws_row != last_size.ws_row || size.ws_col != last_size.ws_col {
                unsafe { libc::ioctl(master.as_raw_fd(), libc::TIOCSWINSZ as _, &size) };
                last_size = size;
            }
        }
        if cancelled.load(Ordering::Acquire) {
            interrupted = true;
            let group = Pid::from_raw(pid as i32);
            let _ = killpg(group, Signal::SIGINT);
            thread::sleep(Duration::from_millis(250));
            if child.try_wait()?.is_none() {
                let _ = killpg(group, Signal::SIGTERM);
            }
            thread::sleep(Duration::from_millis(250));
            if child.try_wait()?.is_none() {
                let _ = killpg(group, Signal::SIGKILL);
            }
            break Some(child.wait().context("reap interrupted PTY child")?);
        }
        if req.timeout_secs > 0 && started.elapsed() >= Duration::from_secs(req.timeout_secs) {
            timed_out = true;
            let group = Pid::from_raw(pid as i32);
            let _ = killpg(group, Signal::SIGTERM);
            thread::sleep(Duration::from_millis(500));
            if child.try_wait()?.is_none() {
                let _ = killpg(group, Signal::SIGKILL);
            }
            break Some(child.wait().context("reap timed-out PTY child")?);
        }
        thread::sleep(Duration::from_millis(10));
    };
    loop {
        match master.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => bytes.push(&buffer[..n]),
            Err(error)
                if error.kind() == ErrorKind::WouldBlock
                    || error.raw_os_error() == Some(libc::EIO) =>
            {
                break
            }
            Err(error) => return Err(error).context("drain PTY master"),
        }
    }
    let merged = bytes.finish();
    Ok(ExecutionResult {
        stdout: filter_unsafe_ansi(&merged),
        stderr: String::new(),
        exit_code: status.and_then(|s| s.code()),
        timed_out,
        interrupted,
    })
}

use std::os::fd::AsRawFd;

fn local_winsize() -> Winsize {
    let mut size = Winsize {
        ws_row: 24,
        ws_col: 80,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ as _, &mut size) };
    if size.ws_row == 0 {
        size.ws_row = 24;
    }
    if size.ws_col == 0 {
        size.ws_col = 80;
    }
    size
}

fn forward_available_stdin(master: &mut File) -> Result<()> {
    let mut poll_fd = libc::pollfd {
        fd: libc::STDIN_FILENO,
        events: libc::POLLIN,
        revents: 0,
    };
    let ready = unsafe { libc::poll(&mut poll_fd, 1, 0) };
    if ready > 0 && poll_fd.revents & libc::POLLIN != 0 {
        let mut input = [0_u8; 1024];
        let count =
            unsafe { libc::read(libc::STDIN_FILENO, input.as_mut_ptr().cast(), input.len()) };
        if count > 0 {
            master
                .write_all(&input[..count as usize])
                .context("write interactive PTY input")?;
        }
    }
    Ok(())
}

struct InteractiveTerminalGuard {
    restore_tui: bool,
}
impl InteractiveTerminalGuard {
    fn enter(restore_tui: bool) -> Result<Self> {
        if restore_tui {
            crossterm::terminal::disable_raw_mode().ok();
            crossterm::execute!(
                std::io::stdout(),
                crossterm::event::DisableMouseCapture,
                crossterm::terminal::LeaveAlternateScreen,
                crossterm::cursor::Show
            )?;
        }
        crossterm::terminal::enable_raw_mode()
            .context("enable raw mode for interactive command")?;
        Ok(Self { restore_tui })
    }
}
impl Drop for InteractiveTerminalGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
        if self.restore_tui {
            let _ = crossterm::execute!(
                std::io::stdout(),
                crossterm::terminal::EnterAlternateScreen,
                crossterm::event::EnableMouseCapture,
                crossterm::cursor::Hide
            );
            let _ = crossterm::terminal::enable_raw_mode();
        } else {
            let _ = crossterm::execute!(
                std::io::stdout(),
                crossterm::terminal::LeaveAlternateScreen,
                crossterm::cursor::Show
            );
        }
    }
}
