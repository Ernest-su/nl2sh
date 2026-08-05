use crate::config::ExecuteUserMode;
use anyhow::{bail, Result};
use std::ffi::OsString;
use std::{env, os::unix::fs::PermissionsExt, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Effective privilege capabilities detected at startup.
pub enum RootStatus {
    /// Process already has UID zero.
    Root,
    /// Process is unprivileged but a `su` executable exists.
    SuAvailable,
    /// Only the current unprivileged shell is available.
    Normal,
}
/// Injectable source of UID and `su` availability information.
pub trait RootProbe: Send + Sync {
    /// Returns the effective numeric UID.
    fn uid(&self) -> u32;
    /// Returns whether a plausible `su` executable is installed.
    fn su_available(&self) -> bool;
}
/// Root probe backed by `geteuid` and filesystem/PATH inspection.
pub struct SystemRootProbe;
impl RootProbe for SystemRootProbe {
    fn uid(&self) -> u32 {
        nix::unistd::geteuid().as_raw()
    }
    fn su_available(&self) -> bool {
        let in_path = env::var_os("PATH").is_some_and(|paths| {
            env::split_paths(&paths).any(|directory| is_executable(&directory.join("su")))
        });
        in_path
            || ["/system/xbin/su", "/system/bin/su", "/sbin/su"]
                .iter()
                .any(|candidate| is_executable(Path::new(candidate)))
    }
}

fn is_executable(path: &Path) -> bool {
    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}
impl SystemRootProbe {
    /// Classifies current root capabilities without invoking an authorization prompt.
    pub fn status(&self) -> RootStatus {
        if self.uid() == 0 {
            RootStatus::Root
        } else if self.su_available() {
            RootStatus::SuAvailable
        } else {
            RootStatus::Normal
        }
    }
}

/// Resolves user mode and local assessment into a safe program/argv pair.
pub fn resolve_invocation(
    command: &str,
    mode: ExecuteUserMode,
    needs_root: bool,
    probe: &dyn RootProbe,
) -> Result<(OsString, Vec<OsString>)> {
    let root = probe.uid() == 0;
    let elevate = match mode {
        ExecuteUserMode::Normal => false,
        ExecuteUserMode::Root => !root,
        ExecuteUserMode::Auto => needs_root && !root,
    };
    if elevate {
        if !probe.su_available() {
            bail!("root execution requested but su is unavailable")
        }
        Ok(("su".into(), vec!["-c".into(), command.into()]))
    } else {
        Ok((shell_path().into(), vec!["-c".into(), command.into()]))
    }
}

fn shell_path() -> &'static str {
    #[cfg(target_os = "android")]
    {
        "/system/bin/sh"
    }
    #[cfg(not(target_os = "android"))]
    {
        "/bin/sh"
    }
}
