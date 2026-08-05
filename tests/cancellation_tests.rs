#![cfg(target_os = "linux")]

use nix::{
    sys::signal::{kill, Signal},
    unistd::Pid,
};
use serde_json::json;
use std::{process::Stdio, time::Duration};
use tempfile::tempdir;
use tokio::{
    process::Command,
    time::{sleep, timeout, Instant},
};
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

#[tokio::test]
async fn sigint_interrupts_agent_and_reaps_pty_process_group() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "output": [{
                "type": "function_call",
                "call_id": "cancel-call",
                "name": "execute_shell_command",
                "arguments": "{\"command\":\"sleep 30\"}"
            }]
        })))
        .mount(&server)
        .await;

    let directory = tempdir()?;
    let config = directory.path().join("config.toml");
    std::fs::write(
        &config,
        format!(
            "api_key=''\nmodel='test'\nendpoint='{}/v1'\napi_type='responses'\nexecute_timeout_secs=60\n",
            server.uri()
        ),
    )?;

    let child = Command::new(env!("CARGO_BIN_EXE_nl2sh"))
        .arg("--config")
        .arg(&config)
        .arg("run a cancellable wait")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    let nl2sh_pid = child
        .id()
        .ok_or_else(|| anyhow::anyhow!("nl2sh child has no pid"))?;
    let shell_pid = wait_for_child_pid(nl2sh_pid).await?;

    kill(Pid::from_raw(nl2sh_pid as i32), Signal::SIGINT)?;
    let output = timeout(Duration::from_secs(5), child.wait_with_output()).await??;
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("interrupted"),
        "unexpected stderr: {stderr}"
    );

    let deadline = Instant::now() + Duration::from_secs(2);
    while kill(Pid::from_raw(shell_pid), None).is_ok() && Instant::now() < deadline {
        sleep(Duration::from_millis(25)).await;
    }
    assert!(
        kill(Pid::from_raw(shell_pid), None).is_err(),
        "PTY child {shell_pid} was not reaped"
    );
    Ok(())
}

async fn wait_for_child_pid(parent: u32) -> anyhow::Result<i32> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Ok(tasks) = std::fs::read_dir(format!("/proc/{parent}/task")) {
            for task in tasks.flatten() {
                let path = task.path().join("children");
                if let Ok(contents) = std::fs::read_to_string(path) {
                    if let Some(pid) = contents.split_whitespace().next() {
                        return Ok(pid.parse()?);
                    }
                }
            }
        }
        if Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for nl2sh PTY child")
        }
        sleep(Duration::from_millis(20)).await;
    }
}
