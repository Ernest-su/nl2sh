use nix::{
    sys::signal::{killpg, Signal},
    unistd::Pid,
};
pub fn signal_group(pid: u32, signal: Signal) {
    let _ = killpg(Pid::from_raw(pid as i32), signal);
}
