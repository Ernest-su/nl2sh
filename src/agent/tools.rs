use crate::file_tools::{ApplyPatchArgs, ListDirArgs, ReadFileArgs, SearchTextArgs};
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
/// Returns the JSON-schema definition for the shell tool.
pub fn command_tool() -> ToolDefinition {
    ToolDefinition{name:"execute_shell_command".into(),description:"Execute a shell command in the Android shell environment after security evaluation and required user confirmation.".into(),parameters:json!({"type":"object","properties":{"command":{"type":"string"},"reason":{"type":"string"},"interactive":{"type":"boolean"},"requires_root":{"type":"boolean"}},"required":["command"],"additionalProperties":false})}
}

/// Returns all built-in tools exposed to the model.
pub fn builtin_tools() -> Vec<ToolDefinition> {
    vec![
        command_tool(),
        ToolDefinition { name: "read_file".into(), description: "Read a size-limited UTF-8 text file. Absolute paths, parent components, and symlinks are supported.".into(), parameters: json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}) },
        ToolDefinition { name: "list_dir".into(), description: "List a bounded number of direct children without using shell commands. Absolute paths are supported.".into(), parameters: json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}) },
        ToolDefinition { name: "search_text".into(), description: "Search recursively for literal text in bounded UTF-8 files. Paths are not confined to the current workspace and symlinks are followed with cycle detection.".into(), parameters: json!({"type":"object","properties":{"query":{"type":"string"},"path":{"type":"string","default":"."}},"required":["query"],"additionalProperties":false}) },
        ToolDefinition { name: "apply_patch".into(), description: "Replace exactly one occurrence of old_text in any accessible file, or create a file when old_text is empty. A diff is always shown for local user confirmation before writing.".into(), parameters: json!({"type":"object","properties":{"path":{"type":"string"},"old_text":{"type":"string"},"new_text":{"type":"string"}},"required":["path","old_text","new_text"],"additionalProperties":false}) },
    ]
}

pub(crate) fn parse_read_file(value: serde_json::Value) -> serde_json::Result<ReadFileArgs> {
    serde_json::from_value(value)
}
pub(crate) fn parse_list_dir(value: serde_json::Value) -> serde_json::Result<ListDirArgs> {
    serde_json::from_value(value)
}
pub(crate) fn parse_search_text(value: serde_json::Value) -> serde_json::Result<SearchTextArgs> {
    serde_json::from_value(value)
}
pub(crate) fn parse_apply_patch(value: serde_json::Value) -> serde_json::Result<ApplyPatchArgs> {
    serde_json::from_value(value)
}

#[cfg(test)]
mod tests {
    use super::builtin_tools;

    #[test]
    fn exposes_shell_and_structured_file_tools() {
        let names = builtin_tools()
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            [
                "execute_shell_command",
                "read_file",
                "list_dir",
                "search_text",
                "apply_patch"
            ]
        );
    }
}
