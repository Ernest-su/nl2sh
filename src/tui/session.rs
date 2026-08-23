use super::{
    app::{App, PopupView},
    i18n,
    input::Input,
    output::{
        advance_llm_gradient, append_llm_delta, append_transcript, begin_llm_stream,
        discard_llm_stream, finalize_live_output, push_history, snapshot, SessionOutput,
    },
    terminal::{best_effort_restore, TerminalGuard},
    ui,
};
use crate::{
    agent::{can_remember_approval, AgentOutcome, AgentRunner, ConfirmationDecision, Confirmer},
    config::{Config, UiLanguage},
    history::HistoryLog,
    llm::{ConversationItem, LlmClient, TextDeltaSink},
    provider_account::{build_account_client, AccountBalance},
    security::{RiskLevel, SecurityAssessment},
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

const BALANCE_REFRESH_INTERVAL: Duration = Duration::from_secs(60);

/// Reason a live Agent TUI session returned to its caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionExit {
    /// The user requested a normal exit.
    Quit,
    /// The user requested a configuration flow.
    Configure(ConfigTarget),
}

/// Configuration section selected by a local slash command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigTarget {
    /// Provider, model, interface, and execution defaults.
    All,
    /// API endpoint, credential, and wire protocol.
    Provider,
    /// Model identifier only.
    Model,
    /// Fetch models from the configured provider, with manual fallback.
    Models,
    /// Query a documented provider balance endpoint without persisting the result.
    Balance,
    /// Reload proxy settings already saved by the in-TUI editor.
    Proxy,
}

/// Runs the default Agent as a live single-frame TUI session.
pub async fn run_agent_session(
    config: &Config,
    llm: &dyn LlmClient,
    root: String,
    log: HistoryLog,
    provider_configured: bool,
) -> Result<SessionExit> {
    let old_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|info| {
        best_effort_restore();
        eprintln!("{info}");
    }));
    let result = run_inner(config, llm, root, log, provider_configured).await;
    std::panic::set_hook(old_hook);
    result
}

async fn run_inner(
    config: &Config,
    llm: &dyn LlmClient,
    root: String,
    log: HistoryLog,
    provider_configured: bool,
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
        max_bytes: config.ui_live_output_max_bytes,
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
    let stream_sink = SessionTextSink {
        history: history.clone(),
        max_bytes: config.ui_live_output_max_bytes,
        needs_full_redraw: Arc::new(AtomicBool::new(false)),
    };
    let stream_redraw = stream_sink.needs_full_redraw.clone();
    let mut terminal = TerminalGuard::enter()?;
    let mut app = App {
        input: Input::default(),
        input_history: Vec::new(),
        input_history_index: None,
        input_history_draft: String::new(),
        cursor_visible: true,
        command_selection: 0,
        history: snapshot(&history)?,
        conversation_scroll: 0,
        tool_results_expanded: false,
        welcome_train_frame: Some(0),
        model: config.model.clone(),
        root,
        ascii: config.ascii_symbols,
        language: config.ui_language,
        api_type: format!("{:?}", config.api_type),
        mode: i18n::mode_agent(config.ui_language).into(),
        turn: 0,
        max_context: config.max_context_turns,
        status: if provider_configured {
            i18n::idle(config.ui_language).into()
        } else {
            localized_status(
                config.ui_language,
                "尚未配置模型服务；使用 /config、/provider 或 /model",
                "provider not configured; use /config, /provider, or /model",
            )
        },
        provider_balance: None,
        popup: None,
    };
    let mut model_history: Vec<Vec<ConversationItem>> = Vec::new();
    let mut active: Option<Pin<Box<dyn Future<Output = Result<AgentOutcome>> + '_>>> = None;
    let mut balance_active: Option<
        Pin<Box<dyn Future<Output = Result<Vec<AccountBalance>>> + Send>>,
    > = None;
    let balance_supported = provider_configured && build_account_client(config).is_ok();
    let mut balance_manual = false;
    let mut last_balance_refresh: Option<Instant> = None;
    let mut confirmation: Option<ConfirmationUi> = None;
    let mut proxy_editor: Option<ProxyEditor> = None;
    let mut quit_after_cancel = false;
    let mut cancel_signal_pending = false;
    let mut was_suspended = false;
    let mut last_cursor_blink = Instant::now();
    let mut last_train_frame = Instant::now();
    let mut last_gradient_frame = Instant::now();
    let mut fragmented_arrow = super::events::FragmentedArrowFilter::default();

    loop {
        if proxy_editor.is_some() && fragmented_arrow.take_expired_escape(Duration::from_millis(35))
        {
            proxy_editor = None;
            app.status = localized_status(
                config.ui_language,
                "代理配置未修改",
                "proxy configuration unchanged",
            );
        }
        if balance_supported
            && active.is_none()
            && balance_active.is_none()
            && last_balance_refresh.is_none_or(|last| last.elapsed() >= BALANCE_REFRESH_INTERVAL)
        {
            let account_config = config.clone();
            balance_active = Some(Box::pin(async move {
                let client = build_account_client(&account_config)?;
                client.balances(&account_config).await
            }));
            last_balance_refresh = Some(Instant::now());
        }
        if last_cursor_blink.elapsed() >= Duration::from_millis(500) {
            app.cursor_visible = !app.cursor_visible;
            last_cursor_blink = Instant::now();
        }
        if last_train_frame.elapsed() >= super::app::WELCOME_TRAIN_FRAME_INTERVAL {
            let viewport_width = terminal.terminal().size()?.width.saturating_sub(2) as usize;
            app.advance_welcome_train(viewport_width);
            last_train_frame = Instant::now();
        }
        if active.is_some() && last_gradient_frame.elapsed() >= Duration::from_millis(70) {
            advance_llm_gradient(&history)?;
            last_gradient_frame = Instant::now();
        }
        let is_suspended = suspended.load(Ordering::Acquire);
        if !is_suspended && stream_redraw.swap(false, Ordering::AcqRel) {
            terminal
                .terminal()
                .clear()
                .context("clear terminal after LLM stream")?;
        }
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
            app.popup = confirmation
                .as_ref()
                .map(ConfirmationUi::view)
                .or_else(|| proxy_editor.as_ref().map(ProxyEditor::view));
            terminal.terminal().draw(|frame| ui::draw(frame, &app))?;
        }

        let mut completed = None;
        let mut balance_completed = None;
        if let (Some(balance_future), Some(agent_future)) =
            (balance_active.as_mut(), active.as_mut())
        {
            tokio::select! {
                result = balance_future.as_mut() => balance_completed = Some(result),
                result = agent_future.as_mut() => completed = Some(result),
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
                _ = tokio::time::sleep(Duration::from_millis(11)) => {}
            }
        } else if let Some(future) = balance_active.as_mut() {
            tokio::select! {
                result = future.as_mut() => balance_completed = Some(result),
                _ = tokio::time::sleep(Duration::from_millis(11)) => {}
            }
        } else if let Some(future) = active.as_mut() {
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
                _ = tokio::time::sleep(Duration::from_millis(11)) => {}
            }
        } else {
            tokio::time::sleep(Duration::from_millis(11)).await;
        }

        if let Some(result) = balance_completed {
            balance_active = None;
            match result {
                Ok(balances) if !balances.is_empty() => {
                    app.provider_balance = Some(format_balances(&balances));
                    if balance_manual {
                        app.status =
                            localized_status(config.ui_language, "余额已刷新", "balance refreshed");
                    }
                }
                Ok(_) if balance_manual => {
                    app.status = localized_status(
                        config.ui_language,
                        "Provider 未返回余额",
                        "provider returned no balance",
                    );
                }
                Err(_) if balance_manual => {
                    app.status = localized_status(
                        config.ui_language,
                        "余额刷新失败，保留上次结果",
                        "balance refresh failed; keeping last value",
                    );
                }
                Ok(_) | Err(_) => {}
            }
            balance_manual = false;
        }

        if let Some(result) = completed {
            active = None;
            confirmation = None;
            match result {
                Ok(outcome) => {
                    append_transcript(&history, &outcome, config.ascii_symbols, &log)?;
                    for _ in 0..outcome.history_turns_evicted.min(model_history.len()) {
                        model_history.remove(0);
                    }
                    model_history.push(outcome.transcript);
                    while model_history.len() > config.max_context_turns {
                        model_history.remove(0);
                    }
                    let context = context_usage(
                        outcome.final_input_tokens,
                        config.effective_context_window(),
                    );
                    app.status = match config.ui_language {
                        UiLanguage::ZhCn => format!(
                            "空闲；上次 {} 步，Token 输入 {} / 输出 {} / 总计 {}，上下文 {}",
                            outcome.steps,
                            usage_value(outcome.usage.input_tokens),
                            usage_value(outcome.usage.output_tokens),
                            usage_value(outcome.usage.total_tokens()),
                            context
                        ),
                        UiLanguage::En => format!(
                            "idle; last {} steps, tokens in {} / out {} / total {}, context {}",
                            outcome.steps,
                            usage_value(outcome.usage.input_tokens),
                            usage_value(outcome.usage.output_tokens),
                            usage_value(outcome.usage.total_tokens()),
                            context
                        ),
                    };
                }
                Err(error) => {
                    discard_llm_stream(&history)?;
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
            if let Some(editor) = proxy_editor.as_mut() {
                let Some(key) = fragmented_arrow.normalize(key) else {
                    continue;
                };
                match editor.handle_key(key) {
                    ProxyEditorAction::Continue => {}
                    ProxyEditorAction::Cancel => {
                        proxy_editor = None;
                        app.status = localized_status(
                            config.ui_language,
                            "代理配置未修改",
                            "proxy configuration unchanged",
                        );
                    }
                    ProxyEditorAction::Save => {
                        let mut updated = config.clone();
                        editor.apply(&mut updated);
                        let Some(path) = updated.source.clone() else {
                            app.status = localized_status(
                                config.ui_language,
                                "无法定位配置文件",
                                "cannot locate configuration file",
                            );
                            continue;
                        };
                        match crate::config::save_config(&path, &updated) {
                            Ok(()) => return Ok(SessionExit::Configure(ConfigTarget::Proxy)),
                            Err(_) => {
                                app.status = localized_status(
                                    config.ui_language,
                                    "代理配置无效或保存失败",
                                    "invalid proxy configuration or save failed",
                                );
                            }
                        }
                    }
                }
                continue;
            }
            if active.is_some() {
                continue;
            }
            let Some(key) = fragmented_arrow.normalize(key) else {
                continue;
            };
            app.welcome_train_frame = None;
            match (key.code, key.modifiers) {
                (KeyCode::PageUp, _) => app.scroll_conversation_up(10),
                (KeyCode::PageDown, _) => app.scroll_conversation_down(10),
                (KeyCode::F(2), _) => app.tool_results_expanded = !app.tool_results_expanded,
                (KeyCode::Char('c'), KeyModifiers::CONTROL) => app.input.clear(),
                (KeyCode::Esc, _) => {}
                (KeyCode::Enter, _) => {
                    if app.complete_selected_command() {
                        continue;
                    }
                    let input = app.take_input();
                    match input.trim() {
                        "/exit" => {
                            log.record("local_command", "/exit")?;
                            return Ok(SessionExit::Quit);
                        }
                        "/config" => return Ok(SessionExit::Configure(ConfigTarget::All)),
                        "/provider" => return Ok(SessionExit::Configure(ConfigTarget::Provider)),
                        "/model" => return Ok(SessionExit::Configure(ConfigTarget::Model)),
                        "/models" => return Ok(SessionExit::Configure(ConfigTarget::Models)),
                        "/proxy" => {
                            proxy_editor = Some(ProxyEditor::new(config));
                            app.status = localized_status(
                                config.ui_language,
                                "正在配置网络代理",
                                "configuring network proxy",
                            );
                            continue;
                        }
                        "/balance" => {
                            if !balance_supported {
                                app.status = localized_status(
                                    config.ui_language,
                                    "当前 Provider 不支持余额查询",
                                    "current provider does not support balance lookup",
                                );
                                continue;
                            }
                            let account_config = config.clone();
                            balance_active = Some(Box::pin(async move {
                                let client = build_account_client(&account_config)?;
                                client.balances(&account_config).await
                            }));
                            balance_manual = true;
                            last_balance_refresh = Some(Instant::now());
                            app.status = localized_status(
                                config.ui_language,
                                "正在网络查询 Provider 余额",
                                "fetching provider balance",
                            );
                            continue;
                        }
                        "/help" => {
                            log.record("local_command", "/help")?;
                            history
                                .lock()
                                .map_err(|_| anyhow::anyhow!("TUI history lock is poisoned"))?
                                .extend(i18n::help_history(
                                    config.ui_language,
                                    config.ascii_symbols,
                                ));
                            app.conversation_scroll = 0;
                            continue;
                        }
                        "/clear" => {
                            log.record("local_command", "/clear")?;
                            history
                                .lock()
                                .map_err(|_| anyhow::anyhow!("TUI history lock is poisoned"))?
                                .clear();
                            model_history.clear();
                            app.clear_session_state();
                            app.status = localized_status(
                                config.ui_language,
                                "当前会话历史已清空",
                                "current session history cleared",
                            );
                            continue;
                        }
                        _ => {}
                    }
                    if !input.trim().is_empty() {
                        if !provider_configured {
                            log.record("local_rejection", "provider is not configured")?;
                            history
                                .lock()
                                .map_err(|_| anyhow::anyhow!("TUI history lock is poisoned"))?
                                .push(match config.ui_language {
                                    UiLanguage::ZhCn => "⚠️ 尚未配置模型服务，请先使用 /config 或 /provider；可用 /model 单独修改模型。".into(),
                                    UiLanguage::En => "[WARN] Provider is not configured. Use /config or /provider first; /model changes only the model.".into(),
                                });
                            app.status = localized_status(
                                config.ui_language,
                                "等待配置模型服务",
                                "waiting for provider configuration",
                            );
                            continue;
                        }
                        push_history(&history, format!("> {input}"), &log, "user")?;
                        app.status = localized_status(
                            config.ui_language,
                            "正在请求模型 / 执行工具",
                            "requesting LLM / executing tools",
                        );
                        active = Some(Box::pin(runner.run_with_history_streaming_owned(
                            input,
                            model_history.clone(),
                            &stream_sink,
                        )));
                    }
                }
                (KeyCode::Up, _) if app.command_menu_visible() => app.select_previous_command(),
                (KeyCode::Down, _) if app.command_menu_visible() => app.select_next_command(),
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

fn usage_value(value: Option<u64>) -> String {
    value.map_or_else(|| "?".into(), |value| value.to_string())
}

fn context_usage(input: Option<u64>, window: Option<u64>) -> String {
    match (input, window) {
        (Some(input), Some(window)) if window > 0 => {
            format!("{:.1}%", input as f64 * 100.0 / window as f64)
        }
        _ => "?".into(),
    }
}

fn format_balances(balances: &[AccountBalance]) -> String {
    balances
        .iter()
        .map(|balance| format!("{} {}", balance.currency, balance.amount))
        .collect::<Vec<_>>()
        .join(" / ")
}

struct ProxyEditor {
    enabled: bool,
    proxy_type: crate::config::ProxyType,
    address: String,
    username: String,
    password: String,
    bypass: String,
    selected: usize,
    language: UiLanguage,
}

enum ProxyEditorAction {
    Continue,
    Cancel,
    Save,
}

impl ProxyEditor {
    fn new(config: &Config) -> Self {
        Self {
            enabled: config.proxy_enabled,
            proxy_type: config.proxy_type,
            address: config.proxy_address.clone(),
            username: config.proxy_username.clone(),
            password: config.proxy_password.clone(),
            bypass: config.proxy_bypass.clone(),
            selected: 0,
            language: config.ui_language,
        }
    }

    fn view(&self) -> PopupView {
        let marker = |index| if self.selected == index { ">" } else { " " };
        let kind = match self.proxy_type {
            crate::config::ProxyType::Http => "HTTP/HTTPS CONNECT",
            crate::config::ProxyType::Socks5 => "SOCKS5 (local DNS)",
            crate::config::ProxyType::Socks5h => "SOCKS5H (proxy DNS, recommended)",
        };
        let password = if self.password.is_empty() {
            String::new()
        } else {
            "*".repeat(self.password.chars().count())
        };
        let (title, enabled, labels, help) = match self.language {
            UiLanguage::ZhCn => (
                "网络代理配置",
                if self.enabled {
                    "启用"
                } else {
                    "关闭（保留配置）"
                },
                [
                    "总开关",
                    "类型",
                    "地址 host:port",
                    "用户名",
                    "密码",
                    "绕过列表",
                ],
                "↑/↓ 选择；←/→ 切换；输入编辑；Delete 清空；Enter 下一项/保存；Esc 取消",
            ),
            UiLanguage::En => (
                "Network proxy configuration",
                if self.enabled {
                    "enabled"
                } else {
                    "off (settings retained)"
                },
                [
                    "Master switch",
                    "Type",
                    "Address host:port",
                    "Username",
                    "Password",
                    "Bypass list",
                ],
                "Up/Down select; Left/Right toggle; type to edit; Delete clears; Enter next/save; Esc cancel",
            ),
        };
        let values = [
            enabled,
            kind,
            &self.address,
            &self.username,
            &password,
            &self.bypass,
        ];
        let mut lines = labels
            .into_iter()
            .zip(values)
            .enumerate()
            .map(|(index, (label, value))| format!("{} {label}: {value}", marker(index)))
            .collect::<Vec<_>>();
        lines.push(String::new());
        lines.push(help.into());
        PopupView {
            title: title.into(),
            lines,
            dangerous: false,
            informational: true,
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> ProxyEditorAction {
        match key.code {
            KeyCode::Esc => ProxyEditorAction::Cancel,
            KeyCode::Up => {
                self.selected = self.selected.checked_sub(1).unwrap_or(5);
                ProxyEditorAction::Continue
            }
            KeyCode::Down | KeyCode::Tab => {
                self.selected = (self.selected + 1) % 6;
                ProxyEditorAction::Continue
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ') if self.selected < 2 => {
                if self.selected == 0 {
                    self.enabled = !self.enabled;
                } else {
                    self.proxy_type = match self.proxy_type {
                        crate::config::ProxyType::Http => crate::config::ProxyType::Socks5,
                        crate::config::ProxyType::Socks5 => crate::config::ProxyType::Socks5h,
                        crate::config::ProxyType::Socks5h => crate::config::ProxyType::Http,
                    };
                }
                ProxyEditorAction::Continue
            }
            KeyCode::Enter if self.selected == 5 => ProxyEditorAction::Save,
            KeyCode::Enter => {
                self.selected += 1;
                ProxyEditorAction::Continue
            }
            KeyCode::Backspace if self.selected >= 2 => {
                self.selected_text_mut().pop();
                ProxyEditorAction::Continue
            }
            KeyCode::Delete if self.selected >= 2 => {
                self.selected_text_mut().clear();
                ProxyEditorAction::Continue
            }
            KeyCode::Char(character)
                if self.selected >= 2 && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.selected_text_mut().push(character);
                ProxyEditorAction::Continue
            }
            _ => ProxyEditorAction::Continue,
        }
    }

    fn selected_text_mut(&mut self) -> &mut String {
        match self.selected {
            2 => &mut self.address,
            3 => &mut self.username,
            4 => &mut self.password,
            _ => &mut self.bypass,
        }
    }

    fn apply(&self, config: &mut Config) {
        config.proxy_enabled = self.enabled;
        config.proxy_type = self.proxy_type;
        config.proxy_address = self.address.trim().into();
        config.proxy_username = self.username.clone();
        config.proxy_password = self.password.clone();
        config.proxy_bypass = self.bypass.trim().into();
    }
}

struct SessionTextSink {
    history: Arc<Mutex<Vec<String>>>,
    max_bytes: usize,
    needs_full_redraw: Arc<AtomicBool>,
}

impl TextDeltaSink for SessionTextSink {
    fn begin(&self) {
        let _ = begin_llm_stream(&self.history);
    }

    fn delta(&self, text: &str) {
        let _ = append_llm_delta(&self.history, text, self.max_bytes);
    }

    fn end(&self, completed: bool) {
        if !completed {
            let _ = discard_llm_stream(&self.history);
        }
        self.needs_full_redraw.store(true, Ordering::Release);
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
    selection: usize,
    language: UiLanguage,
}

#[derive(Clone, Copy)]
enum InitialAction {
    AllowOnce,
    AllowForTask,
    Reject,
    Edit,
    Interactive,
    Captured,
}

const INITIAL_ACTIONS: [InitialAction; 6] = [
    InitialAction::AllowOnce,
    InitialAction::AllowForTask,
    InitialAction::Reject,
    InitialAction::Edit,
    InitialAction::Interactive,
    InitialAction::Captured,
];

impl ConfirmationUi {
    fn new(request: ConfirmRequest, language: UiLanguage) -> Self {
        Self {
            request: Some(request),
            stage: ConfirmationStage::Initial,
            text: String::new(),
            approval: ConfirmationDecision::Approve,
            selection: 0,
            language,
        }
    }

    fn view(&self) -> PopupView {
        let Some(request) = &self.request else {
            return match self.language {
                UiLanguage::ZhCn => PopupView {
                    title: "安全确认".into(),
                    lines: vec!["正在关闭…".into()],
                    dangerous: false,
                    informational: false,
                },
                UiLanguage::En => PopupView {
                    title: "Confirmation".into(),
                    lines: vec!["Closing…".into()],
                    dangerous: false,
                    informational: false,
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
            ConfirmationStage::Initial => lines.extend(self.initial_option_lines(request)),
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
            dangerous: matches!(
                request.assessment.risk_level,
                RiskLevel::Dangerous | RiskLevel::Critical
            ),
            informational: false,
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match self.stage {
            ConfirmationStage::Initial => match key.code {
                KeyCode::Up => {
                    self.move_selection(-1);
                    false
                }
                KeyCode::Down => {
                    self.move_selection(1);
                    false
                }
                KeyCode::Enter => self.choose_initial(INITIAL_ACTIONS[self.selection]),
                KeyCode::Char('1' | 'y') => self.choose_initial(InitialAction::AllowOnce),
                KeyCode::Char('2' | 'a') => self.choose_initial(InitialAction::AllowForTask),
                KeyCode::Char('3' | 'n') => self.choose_initial(InitialAction::Reject),
                KeyCode::Char('4' | 'e') => self.choose_initial(InitialAction::Edit),
                KeyCode::Char('5' | 'i') => self.choose_initial(InitialAction::Interactive),
                KeyCode::Char('6' | 't') => self.choose_initial(InitialAction::Captured),
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

    fn initial_option_lines(&self, request: &ConfirmRequest) -> Vec<String> {
        INITIAL_ACTIONS
            .iter()
            .enumerate()
            .map(|(index, action)| {
                let selected = if index == self.selection { ">" } else { " " };
                let disabled = matches!(action, InitialAction::AllowForTask)
                    && !can_remember_approval(&request.assessment);
                let label = match (self.language, action, disabled) {
                    (UiLanguage::ZhCn, InitialAction::AllowOnce, _) => "仅允许本次 [y]",
                    (UiLanguage::ZhCn, InitialAction::AllowForTask, false) => {
                        "本次任务总是允许完全相同的命令 [a]"
                    }
                    (UiLanguage::ZhCn, InitialAction::AllowForTask, true) => {
                        "总是允许（Root 或高风险命令不可用）"
                    }
                    (UiLanguage::ZhCn, InitialAction::Reject, _) => "拒绝 [n]",
                    (UiLanguage::ZhCn, InitialAction::Edit, _) => "编辑并重新分类 [e]",
                    (UiLanguage::ZhCn, InitialAction::Interactive, _) => "交互终端执行 [i]",
                    (UiLanguage::ZhCn, InitialAction::Captured, _) => "捕获输出执行 [t]",
                    (UiLanguage::En, InitialAction::AllowOnce, _) => "Allow once [y]",
                    (UiLanguage::En, InitialAction::AllowForTask, false) => {
                        "Always allow the exact command for this task [a]"
                    }
                    (UiLanguage::En, InitialAction::AllowForTask, true) => {
                        "Always allow (unavailable for root or high risk)"
                    }
                    (UiLanguage::En, InitialAction::Reject, _) => "Reject [n]",
                    (UiLanguage::En, InitialAction::Edit, _) => "Edit and reassess [e]",
                    (UiLanguage::En, InitialAction::Interactive, _) => {
                        "Run in interactive terminal [i]"
                    }
                    (UiLanguage::En, InitialAction::Captured, _) => "Run with captured output [t]",
                };
                format!("{selected} {}. {label}", index + 1)
            })
            .collect()
    }

    fn move_selection(&mut self, direction: isize) {
        for _ in 0..INITIAL_ACTIONS.len() {
            self.selection = if direction < 0 {
                self.selection
                    .checked_sub(1)
                    .unwrap_or(INITIAL_ACTIONS.len() - 1)
            } else {
                (self.selection + 1) % INITIAL_ACTIONS.len()
            };
            if self.selection != 1 || self.can_remember_current() {
                break;
            }
        }
    }

    fn can_remember_current(&self) -> bool {
        self.request
            .as_ref()
            .is_some_and(|request| can_remember_approval(&request.assessment))
    }

    fn choose_initial(&mut self, action: InitialAction) -> bool {
        match action {
            InitialAction::Reject => self.finish(ConfirmationDecision::Reject),
            InitialAction::Edit => {
                self.text = self
                    .request
                    .as_ref()
                    .map(|request| request.command.clone())
                    .unwrap_or_default();
                self.stage = ConfirmationStage::Edit;
                false
            }
            InitialAction::AllowForTask if !self.can_remember_current() => false,
            InitialAction::AllowOnce
            | InitialAction::AllowForTask
            | InitialAction::Interactive
            | InitialAction::Captured => {
                self.approval = match action {
                    InitialAction::AllowOnce => ConfirmationDecision::Approve,
                    InitialAction::AllowForTask => ConfirmationDecision::ApproveForTask,
                    InitialAction::Interactive => ConfirmationDecision::ApproveInteractive,
                    InitialAction::Captured => ConfirmationDecision::ApproveCaptured,
                    InitialAction::Reject | InitialAction::Edit => ConfirmationDecision::Reject,
                };
                if self
                    .request
                    .as_ref()
                    .is_some_and(|request| request.assessment.requires_double_confirmation)
                {
                    self.stage = ConfirmationStage::Double;
                    false
                } else {
                    self.finish(self.approval.clone())
                }
            }
        }
    }

    fn finish(&mut self, decision: ConfirmationDecision) -> bool {
        if let Some(request) = self.request.take() {
            let _ = request.reply.send(decision);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balance_format_contains_only_display_values() {
        let text = format_balances(&[AccountBalance {
            currency: "CNY".into(),
            amount: "12.34".into(),
        }]);
        assert_eq!(text, "CNY 12.34");
    }

    #[test]
    fn proxy_editor_masks_password_and_preserves_fields_when_disabled() {
        let mut config = Config {
            proxy_enabled: true,
            proxy_address: "proxy.example:1080".into(),
            proxy_username: "user".into(),
            proxy_password: "secret".into(),
            ..Config::default()
        };
        let mut editor = ProxyEditor::new(&config);
        editor.enabled = false;
        let view = editor.view();
        assert!(view.lines.iter().any(|line| line.contains("******")));
        assert!(!view.lines.iter().any(|line| line.contains("secret")));
        editor.apply(&mut config);
        assert!(!config.proxy_enabled);
        assert_eq!(config.proxy_address, "proxy.example:1080");
        assert_eq!(config.proxy_password, "secret");
    }
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

    #[test]
    fn numbered_menu_supports_task_approval_and_reject_aliases() {
        let (mut ui, mut response) = request("touch /tmp/file");
        let view = ui.view();
        assert!(view.lines.iter().any(|line| line.starts_with("> 1.")));
        assert!(view.lines.iter().any(|line| line.contains("[a]")));
        assert!(ui.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE,)));
        assert_eq!(
            response.try_recv(),
            Ok(ConfirmationDecision::ApproveForTask)
        );

        let (mut rejected, mut rejected_response) = request("touch /tmp/file");
        assert!(rejected.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE,)));
        assert_eq!(
            rejected_response.try_recv(),
            Ok(ConfirmationDecision::Reject)
        );
    }

    #[test]
    fn high_risk_menu_disables_always_allow() {
        let (mut ui, mut response) = request("rm -rf /");
        let view = ui.view();
        assert!(view.lines.iter().any(|line| line.contains("不可用")));
        assert!(!ui.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE,)));
        assert!(matches!(
            response.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        ));

        assert!(!ui.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)));
        assert_eq!(ui.selection, 2);
    }

    #[test]
    fn arrow_navigation_and_fragmented_escape_sequence_keep_approval_open() {
        let (mut ui, mut response) = request("touch /tmp/file");
        for code in [KeyCode::Down, KeyCode::Up, KeyCode::Down, KeyCode::Up] {
            assert!(!ui.handle_key(KeyEvent::new(code, KeyModifiers::NONE)));
        }
        for code in [KeyCode::Esc, KeyCode::Char('['), KeyCode::Char('A')] {
            assert!(!ui.handle_key(KeyEvent::new(code, KeyModifiers::NONE)));
        }
        assert!(matches!(
            response.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        ));
        assert!(ui.request.is_some());
    }
}
