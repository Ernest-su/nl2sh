#![cfg(unix)]

use nl2sh::{
    config::Config,
    shell::{CommandExecutor, ShellExecutor},
};

#[tokio::test]
async fn pty_captures_merged_output_and_exit_code() -> anyhow::Result<()> {
    let executor = ShellExecutor::new(Config {
        enable_pty: true,
        ..Config::default()
    });
    let result = executor.execute("printf 'pty-ok\\n'", false, false).await?;
    assert_eq!(result.exit_code, Some(0));
    assert!(result.stdout.contains("pty-ok"));
    assert!(result.stderr.is_empty());
    Ok(())
}

#[tokio::test]
async fn pty_timeout_reaps_child() -> anyhow::Result<()> {
    let executor = ShellExecutor::new(Config {
        enable_pty: true,
        execute_timeout_secs: 1,
        ..Config::default()
    });
    let result = executor.execute("sleep 5", false, false).await?;
    assert!(result.timed_out);
    Ok(())
}
