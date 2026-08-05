use crate::security::SecurityAssessment;
use anyhow::Result;
use async_trait::async_trait;
use std::io::{self, IsTerminal, Write};
#[async_trait]
/// UI-independent approval interface invoked after every assessment.
pub trait Confirmer: Send + Sync {
    /// Requests approval, rejection, or an edited replacement command.
    async fn confirm(
        &self,
        command: &str,
        assessment: &SecurityAssessment,
    ) -> Result<ConfirmationDecision>;
}
#[derive(Debug, Clone, PartialEq, Eq)]
/// User decision returned by a confirmation interface.
pub enum ConfirmationDecision {
    /// Execute the currently assessed command.
    Approve,
    /// Execute using the bidirectional interactive terminal bridge.
    ApproveInteractive,
    /// Force captured execution inside the ordinary output stream.
    ApproveCaptured,
    /// Do not execute it.
    Reject,
    /// Replace it and run security assessment again.
    Edit(String),
}
/// Line-oriented confirmer that refuses approval when no TTY is available.
pub struct StdioConfirmer;
#[async_trait]
impl Confirmer for StdioConfirmer {
    async fn confirm(&self, command: &str, a: &SecurityAssessment) -> Result<ConfirmationDecision> {
        if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
            eprintln!(
                "Refusing {:?} command without an interactive TTY: {command}",
                a.risk_level
            );
            return Ok(ConfirmationDecision::Reject);
        }
        println!(
            "⚠️ {:?}{}: {command}",
            a.risk_level,
            if a.requires_root { " ROOT" } else { "" }
        );
        print!("Run, interactive, captured, edit, or cancel? [y/I/T/E/N] ");
        io::stdout().flush()?;
        let mut choice = String::new();
        io::stdin().read_line(&mut choice)?;
        if choice.trim().eq_ignore_ascii_case("e") {
            print!("Edited command [{command}]: ");
            io::stdout().flush()?;
            let mut edited = String::new();
            io::stdin().read_line(&mut edited)?;
            let edited = edited.trim();
            return Ok(if edited.is_empty() {
                ConfirmationDecision::Reject
            } else {
                ConfirmationDecision::Edit(edited.into())
            });
        }
        let approval = match choice.trim() {
            "y" | "Y" => ConfirmationDecision::Approve,
            "i" | "I" => ConfirmationDecision::ApproveInteractive,
            "t" | "T" => ConfirmationDecision::ApproveCaptured,
            _ => return Ok(ConfirmationDecision::Reject),
        };
        if a.requires_double_confirmation {
            Ok(if ask_exact_yes("High risk: type YES to confirm: ")? {
                approval
            } else {
                ConfirmationDecision::Reject
            })
        } else {
            Ok(approval)
        }
    }
}
fn ask_exact_yes(prompt: &str) -> Result<bool> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut s = String::new();
    io::stdin().read_line(&mut s)?;
    Ok(s.trim() == "YES")
}
