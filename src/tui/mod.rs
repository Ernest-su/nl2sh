mod app;
mod events;
mod i18n;
mod input;
mod markdown;
mod output;
mod session;
mod terminal;
mod theme;
mod ui;
pub use app::{run, TuiOptions};
pub use session::{run_agent_session, ConfigTarget, SessionExit};

/// Returns localized help lines for display by a TUI frontend.
pub fn help_history(language: crate::config::UiLanguage, ascii: bool) -> Vec<String> {
    i18n::help_history(language, ascii)
}
