mod cli;
use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Mode};
use nl2sh::{
    agent::{AgentRunner, ConfirmationDecision, Confirmer, StdioConfirmer},
    config::{self},
    history::HistoryLog,
    llm::{build_client, ConversationMessage, LlmClient, LlmRequest, Role},
    security::assess,
    shell::{ConsoleOutput, OutputSink, ShellExecutor, SystemRootProbe},
    tui,
};
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let path = match &cli.config {
        Some(p) => p.clone(),
        None => config::default_config_path()?,
    };
    if cli.init {
        return config::run_wizard(&path);
    }
    if !path.exists() {
        if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
            config::run_wizard(&path)?;
            println!("Configuration saved; starting nl2sh.");
        } else {
            anyhow::bail!(
                "configuration not found at {}; run nl2sh --init",
                path.display()
            )
        }
    }
    let mut cfg = load_runtime_config(&path, &cli)?;
    let history_log = HistoryLog::open(&path, &cfg.history_log_file)?;
    let mut llm = build_client(&cfg)?;
    let probe = SystemRootProbe;
    let root = format!("{:?}", probe.status());
    if let Some(instruction) = cli.instruction.as_deref() {
        run_once(
            &cfg,
            &llm,
            cli.mode,
            instruction,
            cli.dry_run,
            &[],
            std::sync::Arc::new(ConsoleOutput),
        )
        .await?;
        return Ok(());
    }
    if matches!(cli.mode, Mode::Agent) {
        loop {
            match tui::run_agent_session(&cfg, &llm, root.clone(), history_log.clone()).await? {
                tui::SessionExit::Quit => return Ok(()),
                tui::SessionExit::Reconfigure => {
                    config::run_reconfigure(&path)?;
                    cfg = load_runtime_config(&path, &cli)?;
                    llm = build_client(&cfg)?;
                }
            }
        }
    }
    let mut model_history = Vec::new();
    let ui_history = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    loop {
        let snapshot = ui_history
            .lock()
            .map_err(|_| anyhow::anyhow!("TUI history lock is poisoned"))?
            .clone();
        let instruction = match tui::run(
            tui::TuiOptions {
                model: cfg.model.clone(),
                root: root.clone(),
                ascii: cfg.ascii_symbols,
                language: cfg.ui_language,
                api_type: format!("{:?}", cfg.api_type),
                mode: match (cfg.ui_language, cli.mode) {
                    (config::UiLanguage::ZhCn, Mode::Agent) => "智能体".into(),
                    (config::UiLanguage::ZhCn, Mode::Command) => "命令".into(),
                    (config::UiLanguage::En, mode) => format!("{mode:?}"),
                },
                turn: model_history.len(),
                max_context: cfg.max_context_turns,
            },
            snapshot,
        )? {
            Some(value) => value,
            None => return Ok(()),
        };
        if instruction.trim() == "/config" {
            config::run_reconfigure(&path)?;
            cfg = load_runtime_config(&path, &cli)?;
            llm = build_client(&cfg)?;
            ui_history
                .lock()
                .map_err(|_| anyhow::anyhow!("TUI history lock is poisoned"))?
                .push(match cfg.ui_language {
                    config::UiLanguage::ZhCn => "[CONFIG] 模型服务配置已重新加载。".into(),
                    config::UiLanguage::En => "[CONFIG] Provider configuration reloaded.".into(),
                });
            continue;
        }
        ui_history
            .lock()
            .map_err(|_| anyhow::anyhow!("TUI history lock is poisoned"))?
            .push(format!("> {instruction}"));
        let output = std::sync::Arc::new(UiOutput {
            history: ui_history.clone(),
            ascii: cfg.ascii_symbols,
        });
        match run_once(
            &cfg,
            &llm,
            cli.mode,
            &instruction,
            cli.dry_run,
            &model_history,
            output,
        )
        .await
        {
            Ok(Some(outcome)) => {
                let mut visible = ui_history
                    .lock()
                    .map_err(|_| anyhow::anyhow!("TUI history lock is poisoned"))?;
                append_tool_history(&mut visible, &outcome.transcript, cfg.ascii_symbols);
                visible.push(format!(
                    "{} {}",
                    if cfg.ascii_symbols { "[AGENT]" } else { "🤖" },
                    outcome.final_text
                ));
                model_history.push(outcome.transcript);
            }
            Ok(None) => ui_history
                .lock()
                .map_err(|_| anyhow::anyhow!("TUI history lock is poisoned"))?
                .push(match (cfg.ui_language, cfg.ascii_symbols) {
                    (config::UiLanguage::ZhCn, true) => "[OK] 命令执行完成。".into(),
                    (config::UiLanguage::ZhCn, false) => "✅ 命令执行完成。".into(),
                    (config::UiLanguage::En, true) => "[OK] Command completed.".into(),
                    (config::UiLanguage::En, false) => "✅ Command completed.".into(),
                }),
            Err(error) => ui_history
                .lock()
                .map_err(|_| anyhow::anyhow!("TUI history lock is poisoned"))?
                .push(format!(
                    "{} {error:#}",
                    if cfg.ascii_symbols { "[ERROR]" } else { "❌" }
                )),
        }
    }
}

fn load_runtime_config(path: &std::path::Path, cli: &Cli) -> Result<config::Config> {
    let mut cfg = config::load_unvalidated(Some(path))?;
    if let Some(endpoint) = cli.endpoint.clone() {
        cfg.endpoint = endpoint;
    }
    if let Some(model) = cli.model.clone() {
        cfg.model = model;
    }
    if let Some(api_type) = cli.api_type {
        cfg.api_type = api_type.into();
    }
    if cli.no_pty {
        cfg.enable_pty = false
    }
    if cli.ascii {
        cfg.ascii_symbols = true
    }
    cfg.validate()?;
    Ok(cfg)
}

async fn run_once(
    cfg: &config::Config,
    llm: &dyn LlmClient,
    mode: Mode,
    instruction: &str,
    dry_run: bool,
    history: &[Vec<nl2sh::llm::ConversationItem>],
    output: std::sync::Arc<dyn OutputSink>,
) -> Result<Option<nl2sh::agent::AgentOutcome>> {
    match mode {
        Mode::Agent => {
            let executor = ShellExecutor::new(cfg.clone()).with_output(output);
            let outcome = AgentRunner {
                config: cfg,
                llm,
                executor: &executor,
                confirmer: &StdioConfirmer,
            }
            .run_with_history(instruction, history)
            .await?;
            println!("{}", outcome.final_text);
            Ok(Some(outcome))
        }
        Mode::Command => {
            run_command(cfg, llm, instruction, dry_run, output).await?;
            Ok(None)
        }
    }
}

struct UiOutput {
    history: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    ascii: bool,
}

impl OutputSink for UiOutput {
    fn stdout(&self, text: &str) {
        ConsoleOutput.stdout(text);
        self.push(if self.ascii { "[OUT]" } else { "✅" }, text);
    }
    fn stderr(&self, text: &str) {
        ConsoleOutput.stderr(text);
        self.push(if self.ascii { "[ERR]" } else { "❌" }, text);
    }
}

impl UiOutput {
    fn push(&self, prefix: &str, text: &str) {
        if let Ok(mut history) = self.history.lock() {
            for line in text.lines() {
                history.push(format!("{prefix} {line}"));
            }
        }
    }
}

fn append_tool_history(
    history: &mut Vec<String>,
    transcript: &[nl2sh::llm::ConversationItem],
    ascii: bool,
) {
    for item in transcript {
        if let nl2sh::llm::ConversationItem::Tools(round) = item {
            for call in &round.calls {
                history.push(format!(
                    "{} {}",
                    if ascii { "[TOOL]" } else { "🔧" },
                    call.name
                ));
                if let Some(command) = call
                    .arguments
                    .get("command")
                    .and_then(serde_json::Value::as_str)
                {
                    history.push(format!("{} {command}", if ascii { "[CMD]" } else { "💻" }));
                }
            }
            for result in &round.results {
                let prefix = if result.success {
                    if ascii {
                        "[OK]"
                    } else {
                        "✅"
                    }
                } else if ascii {
                    "[ERROR]"
                } else {
                    "❌"
                };
                history.push(format!("{prefix} {}", result.output));
            }
        }
    }
}

async fn run_command(
    cfg: &config::Config,
    llm: &dyn LlmClient,
    input: &str,
    dry: bool,
    output: std::sync::Arc<dyn OutputSink>,
) -> Result<()> {
    let system="You are an Android shell command generator. Generate one executable command for Android adb shell. Output only the command. No Markdown, explanation, prefix, or alternatives. Do not assume Termux. Prefer toybox. If unsafe or unreliable return NL2SH_UNABLE_TO_GENERATE.";
    let r = llm
        .complete(LlmRequest {
            model: cfg.model.clone(),
            items: vec![
                nl2sh::llm::ConversationItem::Message(ConversationMessage::new(
                    Role::System,
                    system,
                )),
                nl2sh::llm::ConversationItem::Message(ConversationMessage::new(Role::User, input)),
            ],
            tools: vec![],
        })
        .await?;
    let raw = r.text.context("model returned no command")?;
    let mut command = clean(&raw);
    if command == "NL2SH_UNABLE_TO_GENERATE" {
        anyhow::bail!("model could not safely generate a command")
    }
    let mut a = assess(&command, cfg);
    println!("Command: {command}\nRisk: {:?}", a.risk_level);
    if dry {
        return Ok(());
    }
    let confirmer = StdioConfirmer;
    let mut interactive_override = None;
    while a.requires_confirmation {
        match confirmer.confirm(&command, &a).await? {
            ConfirmationDecision::Approve => break,
            ConfirmationDecision::ApproveForTask => break,
            ConfirmationDecision::ApproveInteractive => {
                interactive_override = Some(true);
                break;
            }
            ConfirmationDecision::ApproveCaptured => {
                interactive_override = Some(false);
                break;
            }
            ConfirmationDecision::Reject => {
                anyhow::bail!("command was not executed: confirmation refused or unavailable")
            }
            ConfirmationDecision::Edit(edited) => {
                command = edited;
                a = assess(&command, cfg);
                println!("Edited command: {command}\nRisk: {:?}", a.risk_level);
            }
        }
    }
    let executor = ShellExecutor::new(cfg.clone()).with_output(output);
    let result = nl2sh::shell::CommandExecutor::execute(
        &executor,
        &command,
        a.requires_root,
        interactive_override.unwrap_or_else(|| nl2sh::shell::is_interactive(&command, false)),
    )
    .await?;
    if result.timed_out {
        anyhow::bail!("command timed out")
    }
    if result.interrupted {
        anyhow::bail!("command interrupted")
    }
    if result.exit_code != Some(0) {
        anyhow::bail!("command exited with status {:?}", result.exit_code)
    }
    Ok(())
}
fn clean(s: &str) -> String {
    let trimmed = s.trim();
    let mut x = if trimmed.starts_with("```") {
        let mut lines = trimmed.lines();
        let _language = lines.next();
        lines
            .take_while(|line| line.trim() != "```")
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_owned()
    } else {
        trimmed.to_owned()
    };
    for p in ["Command:", "command:"] {
        if let Some(v) = x.strip_prefix(p) {
            x = v.trim().into()
        }
    }
    x.lines().next().unwrap_or("").trim().into()
}

#[cfg(test)]
mod tests {
    use super::clean;
    #[test]
    fn cleans_only_known_command_wrappers() {
        assert_eq!(clean("```sh\nid\n```"), "id");
        assert_eq!(clean("Command: pwd"), "pwd");
        assert_eq!(clean("id\npwd"), "id");
    }
}
