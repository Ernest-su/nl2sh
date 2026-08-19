use super::{process, ExecutionRequest, ExecutionResult};
use crate::limits::BoundedText;
use anyhow::{Context, Result};
use std::process::Stdio;
use tokio::{
    io::{AsyncReadExt, BufReader},
    process::Command,
    time::Duration,
};
pub async fn execute(req: ExecutionRequest) -> Result<ExecutionResult> {
    let mut cmd = Command::new(&req.program);
    cmd.args(&req.args)
        .stdin(if req.interactive {
            Stdio::inherit()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    unsafe {
        cmd.pre_exec(|| {
            if libc::setpgid(0, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = cmd.spawn().context("failed to spawn shell")?;
    let pid = child.id().context("child has no pid")?;
    let out = child.stdout.take().context("missing stdout")?;
    let err = child.stderr.take().context("missing stderr")?;
    let stdout_task = tokio::spawn(read(out, false, req.capture_max_bytes, req.output.clone()));
    let stderr_task = tokio::spawn(read(err, true, req.capture_max_bytes, req.output.clone()));
    enum End {
        Status(std::process::ExitStatus),
        Timeout,
        Interrupted,
    }
    let end = if req.timeout_secs == 0 {
        tokio::select! {
            status = child.wait() => End::Status(status?),
            signal = tokio::signal::ctrl_c() => { signal?; End::Interrupted }
        }
    } else {
        tokio::select! {
            status = child.wait() => End::Status(status?),
            _ = tokio::time::sleep(Duration::from_secs(req.timeout_secs)) => End::Timeout,
            signal = tokio::signal::ctrl_c() => { signal?; End::Interrupted }
        }
    };
    let (timed, interrupted, status) = match end {
        End::Status(status) => (false, false, Some(status)),
        End::Timeout | End::Interrupted => {
            let interrupted = matches!(end, End::Interrupted);
            if interrupted {
                process::signal_group(pid, nix::sys::signal::Signal::SIGINT);
                tokio::time::sleep(Duration::from_millis(250)).await;
                if child.try_wait()?.is_none() {
                    process::signal_group(pid, nix::sys::signal::Signal::SIGTERM);
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            } else {
                process::signal_group(pid, nix::sys::signal::Signal::SIGTERM);
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            if child.try_wait()?.is_none() {
                process::signal_group(pid, nix::sys::signal::Signal::SIGKILL);
            }
            let status = child.wait().await.ok();
            (!interrupted, interrupted, status)
        }
    };
    let stdout = stdout_task.await.context("stdout reader task failed")?;
    let stderr = stderr_task.await.context("stderr reader task failed")?;
    Ok(ExecutionResult {
        stdout,
        stderr,
        exit_code: status.and_then(|s| s.code()),
        timed_out: timed,
        interrupted,
    })
}
async fn read<R: tokio::io::AsyncRead + Unpin>(
    r: R,
    e: bool,
    max_bytes: usize,
    output: std::sync::Arc<dyn super::OutputSink>,
) -> String {
    let mut r = BufReader::new(r);
    let mut b = [0; 4096];
    let mut captured = BoundedText::new(max_bytes);
    loop {
        match r.read(&mut b).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let text = super::filter_unsafe_ansi(&String::from_utf8_lossy(&b[..n]));
                if e {
                    output.stderr(&text)
                } else {
                    output.stdout(&text)
                }
                captured.push(text.as_bytes());
            }
        }
    }
    captured.finish()
}
