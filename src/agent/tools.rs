use crate::llm::ToolDefinition;
use serde::Deserialize;
use serde_json::json;
#[derive(Debug, Deserialize)]
/// Validated arguments accepted from the built-in shell function tool.
pub struct ShellToolArgs {
    /// Shell source to assess locally.
    pub command: String,
    #[serde(default)]
    /// Model explanation, informational only.
    pub reason: String,
    #[serde(default)]
    /// Model interaction hint; local detection remains authoritative too.
    pub interactive: bool,
    #[serde(default)]
    /// Model privilege hint; never directly authorizes root elevation.
    pub requires_root: bool,
}
/// Returns the JSON-schema definition for the only built-in shell tool.
pub fn command_tool() -> ToolDefinition {
    ToolDefinition{name:"execute_shell_command".into(),description:"Execute a shell command in the Android shell environment after security evaluation and required user confirmation.".into(),parameters:json!({"type":"object","properties":{"command":{"type":"string"},"reason":{"type":"string"},"interactive":{"type":"boolean"},"requires_root":{"type":"boolean"}},"required":["command"],"additionalProperties":false})}
}
