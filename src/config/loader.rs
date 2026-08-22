use super::Config;
use anyhow::{Context, Result};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

/// Resolves `config.toml` beside the canonical executable path.
pub fn default_config_path() -> Result<PathBuf> {
    let exe = env::current_exe().context("cannot locate nl2sh executable")?;
    let canonical = fs::canonicalize(&exe).unwrap_or(exe);
    let parent = canonical
        .parent()
        .context("executable has no parent directory")?;
    Ok(parent.join("config.toml"))
}

/// Loads an explicit path or the executable-relative default.
pub fn load(explicit: Option<&Path>) -> Result<Config> {
    let config = load_unvalidated(explicit)?;
    config.validate()?;
    Ok(config)
}

/// Loads and overlays configuration without validation so CLI overrides can
/// be applied before the single authoritative validation pass.
pub fn load_unvalidated(explicit: Option<&Path>) -> Result<Config> {
    let path = match explicit {
        Some(p) => p.to_path_buf(),
        None => default_config_path()?,
    };
    load_from_unvalidated(&path)
}

/// Loads a configuration when present, or returns defaults associated with
/// the requested path so the TUI can start before provider setup.
pub fn load_or_default_unvalidated(path: &Path) -> Result<Config> {
    if path.exists() {
        return load_from_unvalidated(path);
    }
    let mut config = Config {
        source: Some(path.to_path_buf()),
        ..Config::default()
    };
    if let Ok(key) = env::var("NL2SH_API_KEY") {
        config.api_key = key;
    }
    Ok(config)
}

/// Loads, overlays the API-key environment variable, and validates TOML.
pub fn load_from(path: &Path) -> Result<Config> {
    let config = load_from_unvalidated(path)?;
    config.validate()?;
    Ok(config)
}

fn load_from_unvalidated(path: &Path) -> Result<Config> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("cannot read config {}", path.display()))?;
    let mut config: Config =
        toml::from_str(&text).with_context(|| format!("invalid config {}", path.display()))?;
    if let Ok(key) = env::var("NL2SH_API_KEY") {
        config.api_key = key;
    }
    config.source = Some(path.to_path_buf());
    Ok(config)
}
