use nl2sh::config::{load_from, load_unvalidated, ApiType, Config, UiLanguage};
use std::fs;
use tempfile::tempdir;

#[test]
fn normal_toml_and_defaults() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let path = dir.path().join("config.toml");
    fs::write(
        &path,
        "model='local'\nendpoint='http://127.0.0.1:11434/v1'\napi_key=''\n",
    )?;
    let cfg = load_from(&path)?;
    assert_eq!(cfg.model, "local");
    assert_eq!(cfg.api_type, ApiType::Responses);
    assert_eq!(cfg.max_agent_steps, 8);
    assert_eq!(cfg.model_tool_output_max_bytes, 128 * 1024);
    assert_eq!(cfg.history_log_max_bytes, 10 * 1024 * 1024);
    assert_eq!(cfg.ui_language, UiLanguage::ZhCn);
    Ok(())
}

#[test]
fn missing_file_and_invalid_values_fail() -> anyhow::Result<()> {
    let dir = tempdir()?;
    assert!(load_from(&dir.path().join("missing")).is_err());
    let path = dir.path().join("bad.toml");
    fs::write(&path, "api_type='bogus'")?;
    assert!(load_from(&path).is_err());
    fs::write(
        &path,
        "endpoint='http://localhost'\nmodel='x'\nexecute_timeout_secs=0",
    )?;
    assert!(load_from(&path).is_err());
    Ok(())
}

#[test]
fn empty_key_is_valid_for_local_endpoint() {
    let cfg = Config {
        endpoint: "http://127.0.0.1:8080/v1".into(),
        api_key: String::new(),
        ..Config::default()
    };
    assert!(cfg.validate().is_ok());
}

#[test]
fn cli_layer_can_override_before_validation() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let path = dir.path().join("override.toml");
    fs::write(&path, "model=''\nendpoint='not-a-url'\n")?;
    let mut cfg = load_unvalidated(Some(&path))?;
    assert!(cfg.validate().is_err());
    cfg.model = "local".into();
    cfg.endpoint = "http://127.0.0.1:11434/v1".into();
    assert!(cfg.validate().is_ok());
    Ok(())
}
