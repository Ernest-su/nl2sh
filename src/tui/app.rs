use super::{events, i18n, input::Input, terminal::TerminalGuard, ui};
use crate::config::UiLanguage;
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use std::time::{Duration, Instant};
pub struct App {
    pub input: Input,
    pub input_history: Vec<String>,
    pub input_history_index: Option<usize>,
    pub input_history_draft: String,
    pub cursor_visible: bool,
    pub command_selection: usize,
    pub history: Vec<String>,
    /// Number of conversation rows kept above the automatic bottom position.
    pub conversation_scroll: usize,
    /// Whether completed tool results are expanded in the conversation view.
    pub tool_results_expanded: bool,
    pub model: String,
    pub root: String,
    pub ascii: bool,
    pub language: UiLanguage,
    pub api_type: String,
    pub mode: String,
    pub turn: usize,
    pub max_context: usize,
    pub status: String,
    pub popup: Option<PopupView>,
}
pub struct PopupView {
    pub title: String,
    pub lines: Vec<String>,
    /// Whether the local assessment requires danger styling in addition to text.
    pub dangerous: bool,
}
/// Immutable labels and counters displayed by one TUI prompt.
pub struct TuiOptions {
    /// Configured model name.
    pub model: String,
    /// Root capability label.
    pub root: String,
    /// Enables ASCII-only UI labels.
    pub ascii: bool,
    /// Language used for interface labels and startup help.
    pub language: UiLanguage,
    /// API dialect label.
    pub api_type: String,
    /// Agent or command mode label.
    pub mode: String,
    /// Number of completed user turns.
    pub turn: usize,
    /// Maximum retained turns.
    pub max_context: usize,
}
/// Opens the terminal UI and returns one submitted natural-language request.
pub fn run(options: TuiOptions, history: Vec<String>) -> Result<Option<String>> {
    let old = std::panic::take_hook();
    std::panic::set_hook(Box::new(|info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        );
        eprintln!("{info}");
    }));
    let result = run_inner(options, history);
    std::panic::set_hook(old);
    result
}
fn run_inner(options: TuiOptions, mut history: Vec<String>) -> Result<Option<String>> {
    let mut term = TerminalGuard::enter()?;
    if history.is_empty() {
        history = i18n::startup_history(options.language, options.ascii);
    }
    let mut app = App {
        input: Input::default(),
        input_history: Vec::new(),
        input_history_index: None,
        input_history_draft: String::new(),
        cursor_visible: true,
        command_selection: 0,
        history,
        conversation_scroll: 0,
        tool_results_expanded: false,
        model: options.model,
        root: options.root,
        ascii: options.ascii,
        language: options.language,
        api_type: options.api_type,
        mode: options.mode,
        turn: options.turn,
        max_context: options.max_context,
        status: i18n::idle(options.language).into(),
        popup: None,
    };
    let mut last_cursor_blink = Instant::now();
    loop {
        if last_cursor_blink.elapsed() >= Duration::from_millis(500) {
            app.cursor_visible = !app.cursor_visible;
            last_cursor_blink = Instant::now();
        }
        term.terminal().draw(|f| ui::draw(f, &app))?;
        if let Some(event) = events::next()? {
            match event {
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => app.scroll_conversation_up(3),
                    MouseEventKind::ScrollDown => app.scroll_conversation_down(3),
                    _ => {}
                },
                Event::Key(k) if k.kind == KeyEventKind::Press => {
                    app.cursor_visible = true;
                    last_cursor_blink = Instant::now();
                    match (k.code, k.modifiers) {
                        (KeyCode::Char('q'), KeyModifiers::CONTROL) => return Ok(None),
                        (KeyCode::Char('c'), KeyModifiers::CONTROL) => app.input.clear(),
                        (KeyCode::Esc, _) => {}
                        (KeyCode::PageUp, _) => app.scroll_conversation_up(10),
                        (KeyCode::PageDown, _) => app.scroll_conversation_down(10),
                        (KeyCode::F(2), _) => {
                            app.tool_results_expanded = !app.tool_results_expanded
                        }
                        (KeyCode::Enter, _) => {
                            if app.complete_selected_command() {
                                continue;
                            }
                            let s = app.take_input();
                            if !s.trim().is_empty() {
                                return Ok(Some(s));
                            }
                        }
                        (KeyCode::Up, _) if app.command_menu_visible() => {
                            app.select_previous_command()
                        }
                        (KeyCode::Down, _) if app.command_menu_visible() => {
                            app.select_next_command()
                        }
                        (KeyCode::Up, _) => app.previous_input(),
                        (KeyCode::Down, _) => app.next_input(),
                        (KeyCode::Left, _) => app.input.move_left(),
                        (KeyCode::Right, _) => app.input.move_right(),
                        (KeyCode::Home, _) => app.input.move_home(),
                        (KeyCode::End, _) => app.input.move_end(),
                        (KeyCode::Backspace, _) => app.input.backspace(),
                        (KeyCode::Delete, _) => app.input.delete(),
                        (KeyCode::Char(c), _) => app.input.push(c),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
}

impl App {
    pub(crate) fn command_suggestions(&self) -> Vec<&'static str> {
        const COMMANDS: &[&str] = &["/config"];
        let query = self.input.text.trim();
        if !query.starts_with('/') || query.contains(char::is_whitespace) {
            return Vec::new();
        }
        COMMANDS
            .iter()
            .copied()
            .filter(|command| command.starts_with(query))
            .collect()
    }

    pub(crate) fn command_menu_visible(&self) -> bool {
        !self.command_suggestions().is_empty()
    }

    pub(crate) fn select_previous_command(&mut self) {
        let count = self.command_suggestions().len();
        if count > 0 {
            self.command_selection = self.command_selection.checked_sub(1).unwrap_or(count - 1);
        }
    }

    pub(crate) fn select_next_command(&mut self) {
        let count = self.command_suggestions().len();
        if count > 0 {
            self.command_selection = (self.command_selection + 1) % count;
        }
    }

    pub(crate) fn complete_selected_command(&mut self) -> bool {
        let suggestions = self.command_suggestions();
        let Some(command) = suggestions.get(self.command_selection % suggestions.len().max(1))
        else {
            return false;
        };
        if self.input.text == *command {
            return false;
        }
        self.input.set((*command).into());
        true
    }

    pub(crate) fn take_input(&mut self) -> String {
        let input = self.input.take();
        if !input.trim().is_empty()
            && self.input_history.last().map(String::as_str) != Some(input.as_str())
        {
            self.input_history.push(input.clone());
        }
        self.input_history_index = None;
        self.input_history_draft.clear();
        input
    }

    pub(crate) fn previous_input(&mut self) {
        if self.input_history.is_empty() {
            return;
        }
        let index = match self.input_history_index {
            Some(index) => index.saturating_sub(1),
            None => {
                self.input_history_draft = self.input.text.clone();
                self.input_history.len() - 1
            }
        };
        self.input_history_index = Some(index);
        self.input.set(self.input_history[index].clone());
    }

    pub(crate) fn next_input(&mut self) {
        let Some(index) = self.input_history_index else {
            return;
        };
        if index + 1 < self.input_history.len() {
            let next = index + 1;
            self.input_history_index = Some(next);
            self.input.set(self.input_history[next].clone());
        } else {
            self.input_history_index = None;
            self.input
                .set(std::mem::take(&mut self.input_history_draft));
        }
    }

    pub(crate) fn scroll_conversation_up(&mut self, rows: usize) {
        self.conversation_scroll = self
            .conversation_scroll
            .saturating_add(rows)
            .min(u16::MAX as usize);
    }

    pub(crate) fn scroll_conversation_down(&mut self, rows: usize) {
        self.conversation_scroll = self.conversation_scroll.saturating_sub(rows);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_history(rows: usize) -> App {
        App {
            input: Input::default(),
            input_history: Vec::new(),
            input_history_index: None,
            input_history_draft: String::new(),
            cursor_visible: true,
            command_selection: 0,
            history: (0..rows).map(|row| row.to_string()).collect(),
            conversation_scroll: 0,
            tool_results_expanded: false,
            model: "test".into(),
            root: "Normal".into(),
            ascii: true,
            language: UiLanguage::En,
            api_type: "Responses".into(),
            mode: "Agent".into(),
            turn: 0,
            max_context: 10,
            status: "idle".into(),
            popup: None,
        }
    }

    #[test]
    fn conversation_scroll_is_bounded_and_can_return_to_bottom() {
        let mut app = app_with_history(20);
        app.scroll_conversation_up(3);
        assert_eq!(app.conversation_scroll, 3);
        app.scroll_conversation_up(100);
        assert_eq!(app.conversation_scroll, 103);
        app.scroll_conversation_down(5);
        assert_eq!(app.conversation_scroll, 98);
        app.scroll_conversation_down(100);
        assert_eq!(app.conversation_scroll, 0);
    }

    #[test]
    fn input_history_restores_the_draft_after_navigation() {
        let mut app = app_with_history(0);
        app.input_history = vec!["first".into(), "second".into()];
        app.input.set("draft".into());
        app.previous_input();
        assert_eq!(app.input.text, "second");
        app.previous_input();
        assert_eq!(app.input.text, "first");
        app.next_input();
        app.next_input();
        assert_eq!(app.input.text, "draft");
    }

    #[test]
    fn slash_command_can_be_completed_without_hiding_exact_command() {
        let mut app = app_with_history(0);
        app.input.set("/c".into());
        assert_eq!(app.command_suggestions(), vec!["/config"]);
        assert!(app.complete_selected_command());
        assert_eq!(app.input.text, "/config");
        assert!(!app.complete_selected_command());
    }
}
