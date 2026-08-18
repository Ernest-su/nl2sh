use crate::security::{RiskLevel, SecurityAssessment};
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
    /// Execute and remember this exact command for the current Agent task.
    ApproveForTask,
    /// Do not execute it.
    Reject,
    /// Replace it and run security assessment again.
    Edit(String),
}
/// Line-oriented confirmer that refuses approval when no TTY is available.
pub struct StdioConfirmer;

/// Returns whether one approval may be remembered for an identical command in this Agent task.
///
/// Root, strong-confirmation, Dangerous, and Critical commands must always be approved
/// individually, regardless of the confirmation interface.
pub fn can_remember_approval(assessment: &SecurityAssessment) -> bool {
    !assessment.requires_root
        && !assessment.requires_double_confirmation
        && assessment.risk_level <= RiskLevel::Mutating
}

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
        println!("  1. Allow once [y]");
        if can_remember_approval(a) {
            println!("  2. Always allow this exact command for this Agent task [a]");
        } else {
            println!("  2. Always allow is unavailable for root or high-risk commands");
        }
        println!("  3. Reject [n]");
        println!("  4. Edit and reassess [e]");
        println!("  5. Run in interactive terminal [i]");
        println!("  6. Run with captured output [t]");
        print!("Select an option [1-6/y/n/a/e/i/t]: ");
        io::stdout().flush()?;
        let mut choice = String::new();
        io::stdin().read_line(&mut choice)?;
        if matches!(choice.trim(), "4" | "e" | "E") {
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
            "1" | "y" | "Y" => ConfirmationDecision::Approve,
            "2" | "a" | "A" if can_remember_approval(a) => ConfirmationDecision::ApproveForTask,
            "5" | "i" | "I" => ConfirmationDecision::ApproveInteractive,
            "6" | "t" | "T" => ConfirmationDecision::ApproveCaptured,
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
