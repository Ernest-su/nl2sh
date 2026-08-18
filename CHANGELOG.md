# Changelog

格式基于 Keep a Changelog。

## [Unreleased]

### Added

- Mouse-wheel conversation scrolling together with native Shift+drag highlighting and right-click context-menu copy.
- Initial Rust project structure and Android cross-build script.
- Configuration loading, validation, secure initialization wizard and environment API-key override.
- CLI endpoint, model, and API-type overrides applied before final validation.
- Persistent ratatui/crossterm conversation screen, ASCII fallback, scrolling, and terminal restoration guard.
- Chat Completions and Responses protocol adapters behind a unified LLM trait.
- Agent tool loop with ordered call/result rounds, complete-turn context bounding, editing, and execution feedback.
- Fail-closed built-in/configurable security rules, non-TTY refusal, confirmation and strong double confirmation.
- Real openpty executor, interactive bridge/resize, ANSI filtering, pipeline fallback, cancellation, timeout escalation and root/su policy.
- Incremental output sinks for console streaming and TUI history replay.
- Configuration, security, root, HTTP mock, Agent loop/history/error and PTY tests.
- Isolated-process SIGINT regression test proving Agent cancellation and PTY child reaping.
- Live single-frame Agent TUI with in-frame confirmations, execution-mode overrides, cancellation, and pseudo-terminal lifecycle coverage.
- Android NDK build-script support for cc-rs native dependencies and a verified r28c/API 26 AArch64 release build.
- Selectable ARMv7 cross-build and API 34 device smoke coverage for Agent networking, PTY execution, result feedback, and TUI restoration.
- First-run Base-URL-first configuration that continues into the app, secure `0600` config writes, and TUI `/config` provider hot reload.
- Separate TUI rows for user input and runtime status/context information.
- Semantic conversation colors for user input, tool calls, Agent responses, commands, successes, and errors.
- Append-only `0600` JSON Lines history logging for user requests, commands, outputs, results, and errors.
- Simplified Chinese and English TUI localization with Chinese as the default, plus localized setup and confirmation prompts.
- Populated startup history with common Android task examples, `/config`, scrolling, cancellation, and exit guidance.
- Collapsed completed tool results with F2 expansion while retaining live output, full diagnostic logs, and complete model feedback.
- Terminal-native Markdown rendering for Agent replies, including styled inline content, code blocks, Unicode-width tables, wrapping cells, and narrow-screen list fallback.
- One-command Android build/deploy/run script with configurable target directory, Rust target, and adb serial.
- Native Windows PowerShell Android build/deploy/run script using the NDK Windows LLVM toolchain without Bash or WSL.
- Linux and Windows PowerShell deploy/run scripts that push a prebuilt adjacent `nl2sh` binary without compiling it.
- Arrow-key provider selection for common OpenAI-compatible API base URLs, custom endpoints, and visible API-key entry in initial setup and `/config`.
- GitHub Actions release workflow that builds `aarch64-linux-android` and `armv7-linux-androideabi` release binaries with NDK r28c on tag push, packages each with `android-run-linux.sh`, `android-run-windows.ps1` and `config.toml.example` as `.tar.gz`/`.zip` with SHA256 checksums, and publishes them to a GitHub Release.
- A plain-language Chinese user guide covering ADB setup, Linux and Windows launch steps, ABI package selection, first-run configuration, and common troubleshooting; every 32-bit and 64-bit release archive includes it.
- Screenshot of the memory-query TUI conversation embedded in the Chinese user guide and README, and included in release archives so the packaged guide keeps its image.

### Changed

- Unified project documentation around nl2sh's positioning as an Android shell-focused Hermes-like AI agent delivered as a single executable with a rich TUI; this describes product shape, not Hermes API or plugin compatibility.
- Restyled the input row as a Codex-like muted-gray editor strip while keeping the shortcut separator, status row, and bottom separator on the terminal background.
- Agent final-answer guidance now favors user-language summaries, Markdown tables for structured comparisons, and concise readable text over raw tool output.
- Documented Android's misleading `No such file or directory` error for ABI/ELF-interpreter mismatches, including ARMv7 rebuild and verification commands.

### Fixed

- Read-only Android package-version queries using command substitution no longer trigger repeated mutation confirmations; mutating substitutions remain protected.
- Conversation history now accepts mouse-wheel and PageUp/PageDown scrolling instead of being forced to the bottom every frame.
- Android deployment now runs `adb root`, waits for the restarted daemon, verifies UID 0, and only falls back to `su -c`; unreadable private configuration fails early without weakening permissions.
- Fragmented SGR mouse reports from adb terminals are filtered at the input boundary instead of appearing as `[<...M` text or clearing existing input.
- Redirection to `/dev/null` and file-descriptor duplication no longer misclassify read-only diagnostics as mutations or spuriously require root; real writes remain protected.
- Multiline Agent Markdown is rendered as real terminal rows, preserving headings, lists, blank lines, and table rows instead of concatenating them into one line.
- Expanded tool results now calculate wrapped screen rows for bottom alignment and are no longer limited to the number of logical history entries when scrolling.
- Returning from an interactive PTY command now restores mouse capture and forces a full ratatui repaint instead of leaving only command output on a blank screen.

## [0.1.0] - 2026-08-04

### Added

- Initial development baseline.
