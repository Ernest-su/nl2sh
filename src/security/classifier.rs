use super::{detector, rules, MatchedRule, RiskLevel, SecurityAssessment};
use crate::config::{Config, ConfirmPolicy, ExecuteUserMode, SecurityLevel};
use regex::Regex;

/// Classifies a raw command and applies configured confirmation policy.
pub fn assess(command: &str, config: &Config) -> SecurityAssessment {
    let normalized = detector::normalize(command);
    let mut risk = if detector::has_mutation(&normalized) {
        RiskLevel::Mutating
    } else {
        RiskLevel::ReadOnly
    };
    let mut matches = Vec::new();
    for (id, compiled, level, msg) in rules::builtins() {
        match compiled {
            Ok(re) if re.is_match(&normalized) => {
                risk = risk.max(level);
                matches.push(rules::matched(id, msg));
            }
            Err(_) => {
                risk = RiskLevel::Critical;
                matches.push(rules::matched(
                    id,
                    "internal security rule failed to compile",
                ));
            }
            Ok(_) => {}
        }
    }
    for rule in &config.security_rules {
        match Regex::new(&rule.pattern) {
            Ok(re) if re.is_match(&normalized) => {
                let level = parse_risk(&rule.risk);
                risk = risk.max(level);
                matches.push(MatchedRule {
                    id: rule.id.clone(),
                    message: rule.message.clone(),
                });
            }
            Err(_) => {
                risk = RiskLevel::Critical;
                matches.push(MatchedRule {
                    id: rule.id.clone(),
                    message: "custom security rule failed to compile".into(),
                });
            }
            Ok(_) => {}
        }
    }
    let double = risk >= RiskLevel::Dangerous;
    let confirm = double
        || match config.security_level {
            SecurityLevel::Strict => true,
            SecurityLevel::Balanced => risk >= RiskLevel::Mutating,
            SecurityLevel::Unsafe => false,
        }
        || matches!(config.execute_confirm_policy, ConfirmPolicy::Always)
        || (risk >= RiskLevel::Mutating
            && !matches!(config.execute_confirm_policy, ConfirmPolicy::Never));
    SecurityAssessment {
        risk_level: risk,
        matched_rules: matches,
        requires_confirmation: confirm,
        requires_double_confirmation: double,
        requires_root: match config.execute_user_mode {
            ExecuteUserMode::Root => true,
            ExecuteUserMode::Normal => false,
            ExecuteUserMode::Auto => detector::requires_root(&normalized),
        },
        explanation: format!("classified as {risk:?}"),
    }
}
fn parse_risk(s: &str) -> RiskLevel {
    match s.to_ascii_lowercase().as_str() {
        "readonly" | "read_only" => RiskLevel::ReadOnly,
        "mutating" => RiskLevel::Mutating,
        "dangerous" => RiskLevel::Dangerous,
        _ => RiskLevel::Critical,
    }
}
