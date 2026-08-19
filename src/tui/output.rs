use crate::{
    agent::AgentOutcome,
    history::HistoryLog,
    limits::{truncate_text, TRUNCATION_LABEL},
    llm::ConversationItem,
    shell::OutputSink,
};
use anyhow::Result;
use std::sync::{Arc, Mutex};

pub(super) const LIVE_OUTPUT_PREFIX: &str = "\u{1e}LIVE:";
pub(super) const TOOL_RESULT_PREFIX: &str = "\u{1e}RESULT:";

pub(super) struct SessionOutput {
    pub history: Arc<Mutex<Vec<String>>>,
    pub ascii: bool,
    pub log: HistoryLog,
    pub max_bytes: usize,
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
            let used = history
                .iter()
                .filter(|entry| entry.starts_with(LIVE_OUTPUT_PREFIX))
                .map(String::len)
                .sum::<usize>();
            if used >= self.max_bytes {
                if !history.iter().any(|entry| {
                    entry.starts_with(LIVE_OUTPUT_PREFIX) && entry.contains(TRUNCATION_LABEL)
                }) {
                    history.push(format!(
                        "{LIVE_OUTPUT_PREFIX}{prefix} [... {TRUNCATION_LABEL}: live UI limit {} bytes reached; later live output omitted ...]",
                        self.max_bytes
                    ));
                }
                return;
            }
            let bounded = truncate_text(text, self.max_bytes - used);
            let mut remaining = self.max_bytes - used;
            for line in bounded.lines() {
                let entry = format!("{LIVE_OUTPUT_PREFIX}{prefix} {line}");
                if entry.len() > remaining {
                    let marker = format!(
                        "{LIVE_OUTPUT_PREFIX}{prefix} [... {TRUNCATION_LABEL}: live UI limit {} bytes reached ...]",
                        self.max_bytes
                    );
                    if marker.len() <= remaining {
                        history.push(marker);
                    }
                    break;
                }
                remaining -= entry.len();
                history.push(entry);
            }
        }
    }
}

pub(super) fn append_transcript(
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

pub(super) fn encode_tool_result(prefix: &str, output: &str) -> String {
    format!("{TOOL_RESULT_PREFIX}{prefix}\n{output}")
}

pub(super) fn finalize_live_output(history: &Arc<Mutex<Vec<String>>>) -> Result<()> {
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

pub(super) fn push_history(
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

pub(super) fn snapshot(history: &Arc<Mutex<Vec<String>>>) -> Result<Vec<String>> {
    Ok(history
        .lock()
        .map_err(|_| anyhow::anyhow!("TUI history lock is poisoned"))?
        .clone())
}
