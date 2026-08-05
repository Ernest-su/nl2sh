use clap::{Parser, ValueEnum};
use std::path::PathBuf;
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Mode {
    Agent,
    Command,
}
#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub enum ApiTypeArg {
    ChatCompletions,
    Responses,
}
impl From<ApiTypeArg> for nl2sh::config::ApiType {
    fn from(value: ApiTypeArg) -> Self {
        match value {
            ApiTypeArg::ChatCompletions => Self::ChatCompletions,
            ApiTypeArg::Responses => Self::Responses,
        }
    }
}
#[derive(Debug, Parser)]
#[command(version, about = "Natural Language to Shell for Android")]
pub struct Cli {
    pub instruction: Option<String>,
    #[arg(long)]
    pub init: bool,
    #[arg(long)]
    pub config: Option<PathBuf>,
    #[arg(long, value_enum, default_value = "agent")]
    pub mode: Mode,
    #[arg(long)]
    pub endpoint: Option<String>,
    #[arg(long)]
    pub model: Option<String>,
    #[arg(long, value_enum)]
    pub api_type: Option<ApiTypeArg>,
    #[arg(long)]
    pub no_pty: bool,
    #[arg(long)]
    pub ascii: bool,
    #[arg(long)]
    pub dry_run: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_provider_overrides() {
        let cli = Cli::try_parse_from([
            "nl2sh",
            "--endpoint",
            "http://localhost:11434/v1",
            "--model",
            "local",
            "--api-type",
            "chat_completions",
            "task",
        ])
        .expect("valid CLI should parse");
        assert_eq!(cli.endpoint.as_deref(), Some("http://localhost:11434/v1"));
        assert_eq!(cli.model.as_deref(), Some("local"));
        assert!(matches!(cli.api_type, Some(ApiTypeArg::ChatCompletions)));
    }
}
