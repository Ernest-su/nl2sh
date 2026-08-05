use super::{ApiType, Config, ConfirmPolicy, ExecuteUserMode, UiLanguage};
use anyhow::{bail, Context, Result};
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
};

/// Interactively creates a new configuration without overwriting an existing file.
pub fn run_wizard(path: &Path) -> Result<()> {
    if path.exists() {
        bail!(
            "configuration already exists at {}; refusing to overwrite",
            path.display()
        )
    }
    println!("正在创建配置 / Creating configuration: {}", path.display());
    let language = prompt("界面语言 / UI language (zh_cn/en)", "zh_cn")?;
    let ui_language = parse_language(&language)?;
    let endpoint = prompt(
        label(ui_language, "API 服务地址", "API Base URL"),
        "https://api.openai.com/v1",
    )?;
    let api_key = prompt_api_key(label(
        ui_language,
        "API Key（隐藏输入，本地服务可留空）：",
        "API Key (input hidden; empty for local service): ",
    ))?;
    let model = prompt(label(ui_language, "模型", "Model"), "gpt-4o-mini")?;
    let api = prompt(
        label(
            ui_language,
            "API 类型 (responses/chat_completions)",
            "API type (responses/chat_completions)",
        ),
        "responses",
    )?;
    let confirm = prompt(
        label(
            ui_language,
            "确认策略 (always/risk_only/never)",
            "Confirmation policy (always/risk_only/never)",
        ),
        "risk_only",
    )?;
    let root = prompt(
        label(
            ui_language,
            "执行用户 (auto/normal/root)",
            "Execution user (auto/normal/root)",
        ),
        "auto",
    )?;
    let api_type = parse(
        &api,
        &[
            ("responses", ApiType::Responses),
            ("chat_completions", ApiType::ChatCompletions),
        ],
    )?;
    let execute_confirm_policy = parse(
        &confirm,
        &[
            ("always", ConfirmPolicy::Always),
            ("risk_only", ConfirmPolicy::RiskOnly),
            ("never", ConfirmPolicy::Never),
        ],
    )?;
    let execute_user_mode = parse(
        &root,
        &[
            ("auto", ExecuteUserMode::Auto),
            ("normal", ExecuteUserMode::Normal),
            ("root", ExecuteUserMode::Root),
        ],
    )?;
    let cfg = Config {
        api_key,
        endpoint,
        model,
        api_type,
        execute_confirm_policy,
        execute_user_mode,
        ui_language,
        ..Config::default()
    };
    cfg.validate()?;
    write_new(path, &cfg)
}

/// Reconfigures provider credentials while preserving execution and safety settings.
pub fn run_reconfigure(path: &Path) -> Result<()> {
    // Read the stored value directly: an NL2SH_API_KEY environment override
    // must never be copied into config.toml merely by opening `/config`.
    let stored = fs::read_to_string(path)
        .with_context(|| format!("cannot read config {}", path.display()))?;
    let mut cfg: Config =
        toml::from_str(&stored).with_context(|| format!("invalid config {}", path.display()))?;
    cfg.validate()?;
    let default_language = match cfg.ui_language {
        UiLanguage::ZhCn => "zh_cn",
        UiLanguage::En => "en",
    };
    let language = prompt("界面语言 / UI language (zh_cn/en)", default_language)?;
    cfg.ui_language = parse_language(&language)?;
    println!(
        "{} {}",
        label(
            cfg.ui_language,
            "正在重新配置：",
            "Reconfiguring provider at"
        ),
        path.display()
    );
    cfg.endpoint = prompt(
        label(cfg.ui_language, "API 服务地址", "API Base URL"),
        &cfg.endpoint,
    )?;
    let api_key = prompt_api_key(label(
        cfg.ui_language,
        "API Key（隐藏输入，留空保持当前值）：",
        "API Key (input hidden; empty keeps current): ",
    ))?;
    if !api_key.is_empty() {
        cfg.api_key = api_key;
    }
    cfg.model = prompt(label(cfg.ui_language, "模型", "Model"), &cfg.model)?;
    let default_api = match cfg.api_type {
        ApiType::Responses => "responses",
        ApiType::ChatCompletions => "chat_completions",
    };
    let api = prompt(
        label(
            cfg.ui_language,
            "API 类型 (responses/chat_completions)",
            "API type (responses/chat_completions)",
        ),
        default_api,
    )?;
    cfg.api_type = parse(
        &api,
        &[
            ("responses", ApiType::Responses),
            ("chat_completions", ApiType::ChatCompletions),
        ],
    )?;
    cfg.validate()?;
    write_replace(path, &cfg)
}

fn parse_language(value: &str) -> Result<UiLanguage> {
    parse(
        value,
        &[("zh_cn", UiLanguage::ZhCn), ("en", UiLanguage::En)],
    )
}

fn label(language: UiLanguage, zh_cn: &'static str, en: &'static str) -> &'static str {
    match language {
        UiLanguage::ZhCn => zh_cn,
        UiLanguage::En => en,
    }
}

fn prompt_api_key(label: &str) -> Result<String> {
    rpassword::prompt_password(label).context("failed to read API key")
}

fn serialize(cfg: &Config) -> Result<String> {
    toml::to_string_pretty(cfg).context("failed to serialize configuration")
}

fn write_new(path: &Path, cfg: &Config) -> Result<()> {
    let text = serialize(cfg)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .with_context(|| format!("cannot create {}", path.display()))?;
    file.write_all(text.as_bytes())
        .context("cannot write configuration")?;
    Ok(())
}

fn write_replace(path: &Path, cfg: &Config) -> Result<()> {
    let text = serialize(cfg)?;
    let temporary = path.with_extension("toml.nl2sh-new");
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .with_context(|| format!("cannot create temporary config {}", temporary.display()))?;
    if let Err(error) = file
        .write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
    {
        let _ = fs::remove_file(&temporary);
        return Err(error).context("cannot write replacement configuration");
    }
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error).context("cannot replace configuration");
    }
    Ok(())
}

fn prompt(label: &str, default: &str) -> Result<String> {
    print!("{label} [{default}]: ");
    io::stdout().flush()?;
    let mut s = String::new();
    io::stdin().read_line(&mut s)?;
    let s = s.trim();
    Ok(if s.is_empty() {
        default.into()
    } else {
        s.into()
    })
}
fn parse<T: Copy>(value: &str, choices: &[(&str, T)]) -> Result<T> {
    choices
        .iter()
        .find(|x| x.0 == value)
        .map(|x| x.1)
        .with_context(|| format!("invalid choice: {value}"))
}
