use super::{events, i18n, input::Input, terminal::TerminalGuard, ui};
use crate::config::UiLanguage;
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
pub struct App {
    pub input: Input,
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
    loop {
        term.terminal().draw(|f| ui::draw(f, &app))?;
        if let Some(event) = events::next()? {
            match event {
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => app.scroll_conversation_up(3),
                    MouseEventKind::ScrollDown => app.scroll_conversation_down(3),
                    _ => {}
                },
                Event::Key(k) if k.kind == KeyEventKind::Press => match (k.code, k.modifiers) {
                    (KeyCode::Char('q'), KeyModifiers::CONTROL) => return Ok(None),
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => app.input.text.clear(),
                    (KeyCode::Esc, _) => {}
                    (KeyCode::PageUp, _) => app.scroll_conversation_up(10),
                    (KeyCode::PageDown, _) => app.scroll_conversation_down(10),
                    (KeyCode::F(2), _) => app.tool_results_expanded = !app.tool_results_expanded,
                    (KeyCode::Enter, _) => {
                        let s = app.input.take();
                        if !s.trim().is_empty() {
                            return Ok(Some(s));
                        }
                    }
                    (KeyCode::Backspace, _) => app.input.backspace(),
                    (KeyCode::Char(c), _) => app.input.push(c),
                    _ => {}
                },
                _ => {}
            }
        }
    }
}

impl App {
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
}
