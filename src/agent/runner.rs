use super::{command_tool, ConfirmationDecision, Confirmer, ConversationContext, ShellToolArgs};
use crate::{
    config::Config,
    llm::{
        ConversationItem, ConversationMessage, LlmClient, LlmRequest, Role, ToolResult, ToolRound,
    },
    security::assess,
    shell::CommandExecutor,
};
use anyhow::{bail, Context, Result};
use std::collections::HashSet;
/// Dependencies for one bounded Agent tool loop.
pub struct AgentRunner<'a> {
    /// Validated policy and provider configuration.
    pub config: &'a Config,
    /// Provider-neutral model client.
    pub llm: &'a dyn LlmClient,
    /// Command execution boundary.
    pub executor: &'a dyn CommandExecutor,
    /// User approval boundary.
    pub confirmer: &'a dyn Confirmer,
}
#[derive(Debug)]
/// Successful final response and the number of model steps used.
pub struct AgentOutcome {
    /// Model's evidence-based final response.
    pub final_text: String,
    /// Count of completed LLM requests.
    pub steps: usize,
    /// Complete current interaction, including inseparable tool rounds.
    pub transcript: Vec<ConversationItem>,
}
impl AgentRunner<'_> {
    /// Runs one natural-language request until final text or the step limit.
    pub async fn run(&self, input: &str) -> Result<AgentOutcome> {
        self.run_with_history(input, &[]).await
    }

    /// Runs a request with prior complete interactions, truncating only whole turns.
    pub async fn run_with_history(
        &self,
        input: &str,
        history: &[Vec<ConversationItem>],
    ) -> Result<AgentOutcome> {
        let mut ctx=ConversationContext::new("You are an Android shell agent. Use execute_shell_command for evidence. Never claim unexecuted results. Write the final answer in the user's language for a human reader. Summarize conclusions instead of dumping raw tool protocol output. Use a concise Markdown table when comparing multiple items or presenting repeated structured fields; otherwise use clear concise text.",self.config.max_context_turns.saturating_sub(1));
        for turn in history {
            ctx.push_turn(turn.clone());
        }
        let mut current = vec![ConversationItem::Message(ConversationMessage::new(
            Role::User,
            input,
        ))];
        let mut task_approvals = HashSet::new();
        for step in 1..=self.config.max_agent_steps {
            let mut items = ctx.items();
            items.extend(current.clone());
            let response = self
                .llm
                .complete(LlmRequest {
                    model: self.config.model.clone(),
                    items,
                    tools: vec![command_tool()],
                })
                .await?;
            if response.tool_calls.is_empty() {
                let final_text = response
                    .text
                    .unwrap_or_else(|| "Agent returned no text.".into());
                current.push(ConversationItem::Message(ConversationMessage::new(
                    Role::Assistant,
                    final_text.clone(),
                )));
                return Ok(AgentOutcome {
                    final_text,
                    steps: step,
                    transcript: current,
                });
            }
            let calls = response.tool_calls;
            let mut results = Vec::new();
            'tool_calls: for call in calls.iter().cloned() {
                if call.name != "execute_shell_command" {
                    results.push(ToolResult {
                        call_id: call.id,
                        output: "unsupported tool".into(),
                        success: false,
                    });
                    continue;
                }
                let args: ShellToolArgs = serde_json::from_value(call.arguments)
                    .context("invalid shell tool arguments")?;
                let mut command = args.command;
                let mut interactive_override = None;
                let assessment = loop {
                    let assessment = assess(&command, self.config);
                    if !assessment.requires_confirmation {
                        break assessment;
                    }
                    if super::can_remember_approval(&assessment)
                        && task_approvals.contains(&command)
                    {
                        break assessment;
                    }
                    match self.confirmer.confirm(&command, &assessment).await? {
                        ConfirmationDecision::Approve => break assessment,
                        ConfirmationDecision::ApproveForTask => {
                            if super::can_remember_approval(&assessment) {
                                task_approvals.insert(command.clone());
                            }
                            break assessment;
                        }
                        ConfirmationDecision::ApproveInteractive => {
                            interactive_override = Some(true);
                            break assessment;
                        }
                        ConfirmationDecision::ApproveCaptured => {
                            interactive_override = Some(false);
                            break assessment;
                        }
                        ConfirmationDecision::Edit(edited) => {
                            command = edited;
                        }
                        ConfirmationDecision::Reject => {
                            results.push(ToolResult {
                                call_id: call.id,
                                output: format!(
                                    "risk={:?} root={}\nNot executed: user rejected command.",
                                    assessment.risk_level, assessment.requires_root
                                ),
                                success: false,
                            });
                            continue 'tool_calls;
                        }
                    }
                };
                let execution = self
                    .executor
                    .execute(
                        &command,
                        assessment.requires_root,
                        interactive_override.unwrap_or_else(|| {
                            crate::shell::is_interactive(&command, args.interactive)
                        }),
                    )
                    .await;
                let result = match execution {
                    Ok(x) if x.interrupted => {
                        bail!("agent interrupted during command execution")
                    }
                    Ok(x) => ToolResult {
                        call_id: call.id,
                        output: format!(
                            "executed_command={}\nrisk={:?} root={} matched_rules={}\nexit={:?} timed_out={} interrupted={}\nstdout:\n{}\nstderr:\n{}",
                            command,
                            assessment.risk_level,
                            assessment.requires_root,
                            assessment.matched_rules.iter().map(|rule| rule.id.as_str()).collect::<Vec<_>>().join(","),
                            x.exit_code, x.timed_out, x.interrupted, x.stdout, x.stderr
                        ),
                        success: x.exit_code == Some(0) && !x.timed_out && !x.interrupted,
                    },
                    Err(e) => ToolResult {
                        call_id: call.id,
                        output: format!(
                            "executed_command={command}\nrisk={:?} root={}\nExecution failed: {e:#}",
                            assessment.risk_level, assessment.requires_root
                        ),
                        success: false,
                    },
                };
                results.push(result)
            }
            current.push(ConversationItem::Tools(ToolRound { calls, results }));
        }
        let final_text = format!(
            "Agent stopped after reaching the maximum of {} steps; the last tool results were retained.",
            self.config.max_agent_steps
        );
        current.push(ConversationItem::Message(ConversationMessage::new(
            Role::Assistant,
            final_text.clone(),
        )));
        Ok(AgentOutcome {
            final_text,
            steps: self.config.max_agent_steps,
            transcript: current,
        })
    }

    /// Owned-history variant suitable for a UI-managed background future.
    pub async fn run_with_history_owned(
        &self,
        input: String,
        history: Vec<Vec<ConversationItem>>,
    ) -> Result<AgentOutcome> {
        self.run_with_history(&input, &history).await
    }
}
