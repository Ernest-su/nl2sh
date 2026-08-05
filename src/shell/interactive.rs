/// Combines a model hint with local known-command detection.
pub fn is_interactive(command: &str, model_hint: bool) -> bool {
    if model_hint {
        return true;
    }
    let first = shell_words::split(command)
        .ok()
        .and_then(|v| v.first().cloned())
        .unwrap_or_default();
    matches!(
        first.as_str(),
        "vi" | "vim" | "top" | "less" | "more" | "ssh" | "passwd" | "su" | "sh" | "bash"
    ) || command.contains("logcat") && !command.contains(" -d")
}
