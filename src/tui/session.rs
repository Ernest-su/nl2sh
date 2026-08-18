use super::{
    app::{App, PopupView},
    i18n,
    input::Input,
    terminal::TerminalGuard,
    ui,
};
use crate::{
    agent::{AgentOutcome, AgentRunner, ConfirmationDecision, Confirmer},
    config::{Config, UiLanguage},
    history::HistoryLog,
    llm::{ConversationItem, LlmClient},
    security::SecurityAssessment,
    shell::{OutputSink, ShellExecutor},
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind,
};
use nix::{
    sys::signal::{kill, Signal},
    unistd::getpid,
};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, oneshot};

/// Reason a live Agent TUI session returned to its caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionExit {
    /// The user requested a normal exit.
    Quit,
    /// The user entered `/config` and requested provider reconfiguration.
    Reconfigure,
}

/// Runs the default Agent as a live single-frame TUI session.
pub async fn run_agent_session(
    config: &Config,
    llm: &dyn LlmClient,
    root: String,
    log: HistoryLog,
) -> Result<SessionExit> {
    let old_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::event::DisableMouseCapture,
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        );
        eprintln!("{info}");
    }));
    let result = run_inner(config, llm, root, log).await;
    std::panic::set_hook(old_hook);
    result
}

async fn run_inner(
    config: &Config,
    llm: &dyn LlmClient,
    root: String,
    log: HistoryLog,
) -> Result<SessionExit> {
    log.record("session_start", "Agent TUI started")?;
    let history = Arc::new(Mutex::new(i18n::startup_history(
        config.ui_language,
        config.ascii_symbols,
    )));
    let output: Arc<dyn OutputSink> = Arc::new(SessionOutput {
        history: history.clone(),
        ascii: config.ascii_symbols,
        log: log.clone(),
    });
    let suspended = Arc::new(AtomicBool::new(false));
    let executor = ShellExecutor::new(config.clone())
        .with_output(output)
        .with_tui_active(true)
        .with_tui_suspend_flag(suspended.clone());
    let (confirm_tx, mut confirm_rx) = mpsc::unbounded_channel();
    let confirmer = SessionConfirmer { tx: confirm_tx };
    let runner = AgentRunner {
        config,
        llm,
        executor: &executor,
        confirmer: &confirmer,
    };
    let mut terminal = TerminalGuard::enter()?;
    let mut app = App {
        input: Input::default(),
        input_history: Vec::new(),
        input_history_index: None,
        input_history_draft: String::new(),
        cursor_visible: true,
        history: snapshot(&history)?,
        conversation_scroll: 0,
        tool_results_expanded: false,
        model: config.model.clone(),
        root,
        ascii: config.ascii_symbols,
        language: config.ui_language,
        api_type: format!("{:?}", config.api_type),
        mode: i18n::mode_agent(config.ui_language).into(),
        turn: 0,
        max_context: config.max_context_turns,
        status: i18n::idle(config.ui_language).into(),
        popup: None,
    };
    let mut model_history: Vec<Vec<ConversationItem>> = Vec::new();
    let mut active: Option<Pin<Box<dyn Future<Output = Result<AgentOutcome>> + '_>>> = None;
    let mut confirmation: Option<ConfirmationUi> = None;
    let mut quit_after_cancel = false;
    let mut cancel_signal_pending = false;
    let mut was_suspended = false;
    let mut last_cursor_blink = Instant::now();

    loop {
        if last_cursor_blink.elapsed() >= Duration::from_millis(500) {
            app.cursor_visible = !app.cursor_visible;
            last_cursor_blink = Instant::now();
        }
        let is_suspended = suspended.load(Ordering::Acquire);
        if was_suspended && !is_suspended {
            // The interactive child returned to a fresh alternate screen.
            // Invalidate ratatui's retained buffer so unchanged frame regions
            // are repainted instead of remaining blank.
            terminal
                .terminal()
                .clear()
                .context("clear terminal after interactive command")?;
        }
        was_suspended = is_suspended;
        if !is_suspended {
            app.history = snapshot(&history)?;
            app.turn = model_history.len();
            app.popup = confirmation.as_ref().map(ConfirmationUi::view);
            terminal.terminal().draw(|frame| ui::draw(frame, &app))?;
        }

        let mut completed = None;
        if let Some(future) = active.as_mut() {
            tokio::select! {
                result = future.as_mut() => completed = Some(result),
                request = confirm_rx.recv(), if confirmation.is_none() => {
                    if let Some(request) = request {
                        app.status = localized_status(
                            config.ui_language,
                            "等待安全确认",
                            "waiting for confirmation",
                        );
                        confirmation = Some(ConfirmationUi::new(request, config.ui_language));
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(30)) => {}
            }
        } else {
            tokio::time::sleep(Duration::from_millis(30)).await;
        }

        if let Some(result) = completed {
            active = None;
            confirmation = None;
            match result {
                Ok(outcome) => {
                    append_transcript(&history, &outcome, config.ascii_symbols, &log)?;
                    model_history.push(outcome.transcript);
                    while model_history.len() > config.max_context_turns {
                        model_history.remove(0);
                    }
                    app.status = match config.ui_language {
                        UiLanguage::ZhCn => format!("空闲；上次执行 {} 步", outcome.steps),
                        UiLanguage::En => format!("idle; last agent steps: {}", outcome.steps),
                    };
                }
                Err(error) => {
                    finalize_live_output(&history)?;
                    push_history(
                        &history,
                        format!(
                            "{} {error:#}",
                            if config.ascii_symbols {
                                "[ERROR]"
                            } else {
                                "❌"
                            }
                        ),
                        &log,
                        "error",
                    )?;
                    app.status =
                        localized_status(config.ui_language, "出错后空闲", "idle after error");
                }
            }
            if quit_after_cancel {
                return Ok(SessionExit::Quit);
            }
        }

        if cancel_signal_pending && active.is_some() && confirmation.is_none() {
            signal_cancel()?;
            cancel_signal_pending = false;
            app.status = localized_status(config.ui_language, "正在取消", "cancelling");
        }

        if is_suspended {
            continue;
        }
        while event::poll(Duration::ZERO)? {
            let event = event::read()?;
            if let Event::Mouse(mouse) = event {
                match mouse.kind {
                    MouseEventKind::ScrollUp => app.scroll_conversation_up(3),
                    MouseEventKind::ScrollDown => app.scroll_conversation_down(3),
                    _ => {}
                }
                continue;
            }
            let Event::Key(key) = event else { continue };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            app.cursor_visible = true;
            last_cursor_blink = Instant::now();
            if key.code == KeyCode::Char('q') && key.modifiers == KeyModifiers::CONTROL {
                if active.is_some() {
                    if let Some(pending) = confirmation.as_mut() {
                        pending.finish(ConfirmationDecision::Reject);
                        confirmation = None;
                        cancel_signal_pending = true;
                    } else {
                        signal_cancel()?;
                    }
                    quit_after_cancel = true;
                    app.status = localized_status(
                        config.ui_language,
                        "退出前正在取消任务",
                        "cancelling before quit",
                    );
                } else {
                    return Ok(SessionExit::Quit);
                }
                continue;
            }
            if active.is_some()
                && key.code == KeyCode::Char('c')
                && key.modifiers == KeyModifiers::CONTROL
            {
                if let Some(pending) = confirmation.as_mut() {
                    pending.finish(ConfirmationDecision::Reject);
                    confirmation = None;
                    cancel_signal_pending = true;
                } else {
                    signal_cancel()?;
                }
                app.status = localized_status(config.ui_language, "正在取消", "cancelling");
                continue;
            }
            if let Some(pending) = confirmation.as_mut() {
                if pending.handle_key(key) {
                    confirmation = None;
                    app.status =
                        localized_status(config.ui_language, "智能体运行中", "agent running");
                }
                continue;
            }
            if active.is_some() {
                continue;
            }
            match (key.code, key.modifiers) {
                (KeyCode::PageUp, _) => app.scroll_conversation_up(10),
                (KeyCode::PageDown, _) => app.scroll_conversation_down(10),
                (KeyCode::F(2), _) => app.tool_results_expanded = !app.tool_results_expanded,
                (KeyCode::Char('c'), KeyModifiers::CONTROL) => app.input.clear(),
                (KeyCode::Esc, _) => {}
                (KeyCode::Enter, _) => {
                    let input = app.take_input();
                    if input.trim() == "/config" {
                        return Ok(SessionExit::Reconfigure);
                    }
                    if !input.trim().is_empty() {
                        push_history(&history, format!("> {input}"), &log, "user")?;
                        app.status = localized_status(
                            config.ui_language,
                            "正在请求模型 / 执行工具",
                            "requesting LLM / executing tools",
                        );
                        active = Some(Box::pin(
                            runner.run_with_history_owned(input, model_history.clone()),
                        ));
                    }
                }
                (KeyCode::Up, _) => app.previous_input(),
                (KeyCode::Down, _) => app.next_input(),
                (KeyCode::Left, _) => app.input.move_left(),
                (KeyCode::Right, _) => app.input.move_right(),
                (KeyCode::Home, _) => app.input.move_home(),
                (KeyCode::End, _) => app.input.move_end(),
                (KeyCode::Backspace, _) => app.input.backspace(),
                (KeyCode::Delete, _) => app.input.delete(),
                (KeyCode::Char(character), _) => app.input.push(character),
                _ => {}
            }
        }
    }
}

fn signal_cancel() -> Result<()> {
    kill(getpid(), Signal::SIGINT).context("failed to deliver cancellation signal")?;
    Ok(())
}

fn localized_status(language: UiLanguage, zh_cn: &str, en: &str) -> String {
    match language {
        UiLanguage::ZhCn => zh_cn,
        UiLanguage::En => en,
    }
    .into()
}

struct ConfirmRequest {
    command: String,
    assessment: SecurityAssessment,
    reply: oneshot::Sender<ConfirmationDecision>,
}

struct SessionConfirmer {
    tx: mpsc::UnboundedSender<ConfirmRequest>,
}

#[async_trait]
impl Confirmer for SessionConfirmer {
    async fn confirm(
        &self,
        command: &str,
        assessment: &SecurityAssessment,
    ) -> Result<ConfirmationDecision> {
        let (reply, response) = oneshot::channel();
        self.tx
            .send(ConfirmRequest {
                command: command.into(),
                assessment: assessment.clone(),
                reply,
            })
            .map_err(|_| anyhow::anyhow!("TUI confirmation channel closed"))?;
        response.await.context("TUI confirmation was cancelled")
    }
}

enum ConfirmationStage {
    Initial,
    Double,
    Edit,
}

struct ConfirmationUi {
    request: Option<ConfirmRequest>,
    stage: ConfirmationStage,
    text: String,
    approval: ConfirmationDecision,
    language: UiLanguage,
}

impl ConfirmationUi {
    fn new(request: ConfirmRequest, language: UiLanguage) -> Self {
        Self {
            request: Some(request),
            stage: ConfirmationStage::Initial,
            text: String::new(),
            approval: ConfirmationDecision::Approve,
            language,
        }
    }

    fn view(&self) -> PopupView {
        let Some(request) = &self.request else {
            return match self.language {
                UiLanguage::ZhCn => PopupView {
                    title: "安全确认".into(),
                    lines: vec!["正在关闭…".into()],
                },
                UiLanguage::En => PopupView {
                    title: "Confirmation".into(),
                    lines: vec!["Closing…".into()],
                },
            };
        };
        let mut lines = match self.language {
            UiLanguage::ZhCn => vec![
                format!("命令：{}", request.command),
                format!(
                    "风险：{}{}",
                    i18n::risk(self.language, request.assessment.risk_level),
                    if request.assessment.requires_root {
                        " | ROOT"
                    } else {
                        ""
                    }
                ),
                "命令已由本地安全规则重新分类。".into(),
            ],
            UiLanguage::En => vec![
                format!("Command: {}", request.command),
                format!(
                    "Risk: {}{}",
                    i18n::risk(self.language, request.assessment.risk_level),
                    if request.assessment.requires_root {
                        " | ROOT"
                    } else {
                        ""
                    }
                ),
                request.assessment.explanation.clone(),
            ],
        };
        match self.stage {
            ConfirmationStage::Initial => lines.push(match self.language {
                UiLanguage::ZhCn => "Y 默认执行 | I 交互终端 | T 捕获输出 | N 取消 | E 编辑".into(),
                UiLanguage::En => {
                    "Y default | I interactive terminal | T captured | N cancel | E edit".into()
                }
            }),
            ConfirmationStage::Double => {
                lines.push(match self.language {
                    UiLanguage::ZhCn => "高风险操作：输入 YES 后按 Enter：".into(),
                    UiLanguage::En => "High risk: type YES, then Enter:".into(),
                });
                lines.push(self.text.clone());
            }
            ConfirmationStage::Edit => {
                lines.push(match self.language {
                    UiLanguage::ZhCn => "编辑命令后按 Enter（执行前会重新分类）：".into(),
                    UiLanguage::En => {
                        "Edit command, then Enter (reclassified before execution):".into()
                    }
                });
                lines.push(self.text.clone());
            }
        }
        PopupView {
            title: match self.language {
                UiLanguage::ZhCn => "安全确认",
                UiLanguage::En => "Security confirmation",
            }
            .into(),
            lines,
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match self.stage {
            ConfirmationStage::Initial => match key.code {
                KeyCode::Char('y' | 'Y' | 'i' | 'I' | 't' | 'T') => {
                    self.approval = match key.code {
                        KeyCode::Char('i' | 'I') => ConfirmationDecision::ApproveInteractive,
                        KeyCode::Char('t' | 'T') => ConfirmationDecision::ApproveCaptured,
                        _ => ConfirmationDecision::Approve,
                    };
                    if self
                        .request
                        .as_ref()
                        .is_some_and(|r| r.assessment.requires_double_confirmation)
                    {
                        self.stage = ConfirmationStage::Double;
                        false
                    } else {
                        self.finish(self.approval.clone())
                    }
                }
                KeyCode::Char('e' | 'E') => {
                    self.text = self
                        .request
                        .as_ref()
                        .map(|r| r.command.clone())
                        .unwrap_or_default();
                    self.stage = ConfirmationStage::Edit;
                    false
                }
                KeyCode::Char('n' | 'N') | KeyCode::Esc => {
                    self.finish(ConfirmationDecision::Reject)
                }
                _ => false,
            },
            ConfirmationStage::Double => match key.code {
                KeyCode::Enter => {
                    let decision = if self.text == "YES" {
                        self.approval.clone()
                    } else {
                        ConfirmationDecision::Reject
                    };
                    self.finish(decision)
                }
                KeyCode::Esc => self.finish(ConfirmationDecision::Reject),
                KeyCode::Backspace => {
                    self.text.pop();
                    false
                }
                KeyCode::Char(character) => {
                    self.text.push(character);
                    false
                }
                _ => false,
            },
            ConfirmationStage::Edit => match key.code {
                KeyCode::Enter => {
                    let edited = self.text.trim().to_owned();
                    if edited.is_empty() {
                        self.finish(ConfirmationDecision::Reject)
                    } else {
                        self.finish(ConfirmationDecision::Edit(edited))
                    }
                }
                KeyCode::Esc => self.finish(ConfirmationDecision::Reject),
                KeyCode::Backspace => {
                    self.text.pop();
                    false
                }
                KeyCode::Char(character) => {
                    self.text.push(character);
                    false
                }
                _ => false,
            },
        }
    }

    fn finish(&mut self, decision: ConfirmationDecision) -> bool {
        if let Some(request) = self.request.take() {
            let _ = request.reply.send(decision);
        }
        true
    }
}

struct SessionOutput {
    history: Arc<Mutex<Vec<String>>>,
    ascii: bool,
    log: HistoryLog,
}

impl OutputSink for SessionOutput {
    fn stdout(&self, text: &str) {
        self.push(if self.ascii { "[OUT]" } else { "✅" }, "stdout", text);
    }
    fn stderr(&self, text: &str) {
        self.push(if self.ascii { "[ERR]" } else { "❌" }, "stderr", text);
    }
}

impl SessionOutput {
    fn push(&self, prefix: &str, event: &str, text: &str) {
        let _ = self.log.record(event, text);
        if let Ok(mut history) = self.history.lock() {
            for line in text.lines() {
                history.push(format!("{LIVE_OUTPUT_PREFIX}{prefix} {line}"));
            }
        }
    }
}

fn append_transcript(
    history: &Arc<Mutex<Vec<String>>>,
    outcome: &AgentOutcome,
    ascii: bool,
    log: &HistoryLog,
) -> Result<()> {
    let mut visible = history
        .lock()
        .map_err(|_| anyhow::anyhow!("TUI history lock is poisoned"))?;
    visible.retain(|entry| !entry.starts_with(LIVE_OUTPUT_PREFIX));
    for item in &outcome.transcript {
        if let ConversationItem::Tools(round) = item {
            for call in &round.calls {
                log.record("tool_call", &call.name)?;
                visible.push(format!(
                    "{} {}",
                    if ascii { "[TOOL]" } else { "🔧" },
                    call.name
                ));
                if let Some(command) = call
                    .arguments
                    .get("command")
                    .and_then(serde_json::Value::as_str)
                {
                    log.record("command", command)?;
                    visible.push(format!("{} {command}", if ascii { "[CMD]" } else { "💻" }));
                }
            }
            for result in &round.results {
                log.record(
                    if result.success {
                        "tool_result"
                    } else {
                        "tool_error"
                    },
                    &result.output,
                )?;
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
                visible.push(encode_tool_result(prefix, &result.output));
            }
        }
    }
    visible.push(format!(
        "{} {}",
        if ascii { "[AGENT]" } else { "🤖" },
        outcome.final_text
    ));
    log.record("agent", &outcome.final_text)?;
    Ok(())
}

pub(super) const LIVE_OUTPUT_PREFIX: &str = "\u{1e}LIVE:";
pub(super) const TOOL_RESULT_PREFIX: &str = "\u{1e}RESULT:";

pub(super) fn encode_tool_result(prefix: &str, output: &str) -> String {
    format!("{TOOL_RESULT_PREFIX}{prefix}\n{output}")
}

fn finalize_live_output(history: &Arc<Mutex<Vec<String>>>) -> Result<()> {
    let mut history = history
        .lock()
        .map_err(|_| anyhow::anyhow!("TUI history lock is poisoned"))?;
    for entry in history.iter_mut() {
        if let Some(visible) = entry.strip_prefix(LIVE_OUTPUT_PREFIX) {
            *entry = visible.to_owned();
        }
    }
    Ok(())
}

fn push_history(
    history: &Arc<Mutex<Vec<String>>>,
    value: String,
    log: &HistoryLog,
    event: &str,
) -> Result<()> {
    log.record(event, &value)?;
    history
        .lock()
        .map_err(|_| anyhow::anyhow!("TUI history lock is poisoned"))?
        .push(value);
    Ok(())
}

fn snapshot(history: &Arc<Mutex<Vec<String>>>) -> Result<Vec<String>> {
    Ok(history
        .lock()
        .map_err(|_| anyhow::anyhow!("TUI history lock is poisoned"))?
        .clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Config, security::assess};

    fn request(command: &str) -> (ConfirmationUi, oneshot::Receiver<ConfirmationDecision>) {
        let (reply, response) = oneshot::channel();
        let assessment = assess(command, &Config::default());
        (
            ConfirmationUi::new(
                ConfirmRequest {
                    command: command.into(),
                    assessment,
                    reply,
                },
                UiLanguage::ZhCn,
            ),
            response,
        )
    }

    #[test]
    fn dangerous_confirmation_requires_exact_yes() {
        let (mut ui, mut response) = request("rm -rf /");
        assert!(!ui.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)));
        for character in ['Y', 'E', 'S'] {
            assert!(!ui.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE,)));
        }
        assert!(ui.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)));
        assert_eq!(response.try_recv(), Ok(ConfirmationDecision::Approve));
    }

    #[test]
    fn edited_command_is_returned_for_agent_reclassification() {
        let (mut ui, mut response) = request("touch /tmp/old");
        assert!(!ui.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE)));
        ui.text = "rm -rf /".into();
        assert!(ui.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)));
        assert_eq!(
            response.try_recv(),
            Ok(ConfirmationDecision::Edit("rm -rf /".into()))
        );
    }

    #[test]
    fn user_can_force_interactive_execution() {
        let (mut ui, mut response) = request("touch /tmp/file");
        assert!(ui.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE,)));
        assert_eq!(
            response.try_recv(),
            Ok(ConfirmationDecision::ApproveInteractive)
        );
    }
}
