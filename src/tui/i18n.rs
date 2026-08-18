use crate::{config::UiLanguage, security::RiskLevel};

pub(crate) fn startup_history(language: UiLanguage, ascii: bool) -> Vec<String> {
    let agent = if ascii { "[AGENT]" } else { "🤖" };
    let hint = if ascii { "[HINT]" } else { "💡" };
    match language {
        UiLanguage::ZhCn => vec![
            format!("{agent} 欢迎使用 nl2sh，请直接描述要完成的 Android Shell 任务。"),
            format!("{hint} 常用示例：查看已安装应用及版本信息"),
            format!("{hint} 常用示例：查看系统版本、CPU、内存和存储空间"),
            format!("{hint} 常用示例：查找占用空间最大的十个文件"),
            format!("{hint} 常用示例：查看正在运行的进程和网络连接"),
            format!("{hint} 常用命令：/config 重新配置模型服务"),
            format!("{hint} 操作说明：滚轮浏览历史；Shift+拖选文字后用右键菜单复制"),
            format!("{hint} 操作说明：Ctrl+C 取消任务或清空输入；Ctrl+Q 安全退出"),
        ],
        UiLanguage::En => vec![
            format!("{agent} Welcome to nl2sh. Describe an Android shell task to begin."),
            format!("{hint} Example: show installed applications and version information"),
            format!("{hint} Example: show Android version, CPU, memory, and storage"),
            format!("{hint} Example: find the ten largest files"),
            format!("{hint} Example: show running processes and network connections"),
            format!("{hint} Command: /config reconfigure the model provider"),
            format!("{hint} Controls: wheel browses history; Shift+drag selects text for context-menu copy"),
            format!("{hint} Controls: Ctrl+C cancels or clears input; Ctrl+Q quits safely"),
        ],
    }
}

pub(crate) fn mode_agent(language: UiLanguage) -> &'static str {
    match language {
        UiLanguage::ZhCn => "智能体",
        UiLanguage::En => "Agent",
    }
}

pub(crate) fn idle(language: UiLanguage) -> &'static str {
    match language {
        UiLanguage::ZhCn => "空闲",
        UiLanguage::En => "idle",
    }
}

pub(crate) fn risk(language: UiLanguage, risk: RiskLevel) -> &'static str {
    match (language, risk) {
        (UiLanguage::ZhCn, RiskLevel::ReadOnly) => "只读",
        (UiLanguage::ZhCn, RiskLevel::Mutating) => "修改",
        (UiLanguage::ZhCn, RiskLevel::Dangerous) => "危险",
        (UiLanguage::ZhCn, RiskLevel::Critical) => "严重危险",
        (UiLanguage::En, RiskLevel::ReadOnly) => "ReadOnly",
        (UiLanguage::En, RiskLevel::Mutating) => "Mutating",
        (UiLanguage::En, RiskLevel::Dangerous) => "Dangerous",
        (UiLanguage::En, RiskLevel::Critical) => "Critical",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_help_is_populated_in_both_languages() {
        let chinese = startup_history(UiLanguage::ZhCn, true);
        let english = startup_history(UiLanguage::En, true);
        assert!(chinese.len() >= 8);
        assert!(english.len() >= 8);
        assert!(chinese.iter().any(|line| line.contains("应用")));
        assert!(chinese.iter().any(|line| line.contains("/config")));
        assert!(chinese.iter().any(|line| line.contains("Shift+拖选")));
        assert!(english.iter().any(|line| line.contains("applications")));
        assert!(english.iter().any(|line| line.contains("Shift+drag")));
        assert!(english.iter().any(|line| line.contains("Ctrl+Q")));
    }
}
