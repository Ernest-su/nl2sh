//! Reusable nl2sh core. UI and CLI are deliberately thin adapters.
/// Agent loop, conversation policy, confirmation, and built-in tools.
pub mod agent;
/// Validated TOML configuration and initialization wizard.
pub mod config;
/// Persistent structured interaction history for diagnostics.
pub mod history;
/// Provider-neutral LLM types and OpenAI-compatible HTTP clients.
pub mod llm;
/// Local shell command classification and confirmation requirements.
pub mod security;
/// PTY/pipeline execution, process cleanup, and Android root selection.
pub mod shell;
/// Ratatui/crossterm terminal input interface.
pub mod tui;
