# Project Status

Last Updated: 2026-08-06

## Current Phase

Phase 10：本地功能与验证基本完成，等待 Android 交叉编译和真机验证。

## Overall Status

- Product positioning: Android 原生 shell 版的类 Hermes AI Agent；核心程序以单个可执行文件交付，提供多轮 Tool Calling 和丰富 TUI，不声称与 Hermes API 或插件兼容。
- Build status: `cargo check` 和 `cargo build --release` 已于 2026-08-05 在当前 Linux 主机通过。
- Test status: `cargo test` 已通过（53 个 unit/integration tests，0 failure）。
- Android cross-compile status: 已使用 NDK r28c、API 26 成功构建 `aarch64-linux-android` release 产物。
- Android device validation: 已在 KONKA Android TV（API 34、`armeabi-v7a`）完成部署、Agent/PTY 和 TUI smoke test。
- CI release workflow: 已添加 `.github/workflows/release.yml`，在推送 `v*` tag 时用 GitHub Actions 并行交叉编译 `aarch64-linux-android` 与 `armv7-linux-androideabi`，把预编译 `nl2sh` 与 Linux/Windows 启动脚本、`config.toml.example`、`使用说明.md` 打包为 `.tar.gz`/`.zip` 并附带 SHA256 校验和发布到 GitHub Release；`workflow_dispatch` 可手动触发草稿发布。
- Known blockers: 尚未验证真实 root 提权、修改确认、超时和全屏交互程序；CI workflow 尚未在真实 GitHub Actions 上运行验证。

## Completed

- Android shell 版类 Hermes AI Agent 的产品定位，以单文件 Android 可执行程序和丰富 TUI 为主要交付形态。
- 模块化 Cargo 工程、CLI、配置加载/校验/向导。
- 两种 OpenAI API adapter 与统一 LLM trait。
- Agent loop、shell tool、真实结果回传和最大轮数。
- 四级安全评估、内置/自定义规则和确认接口。
- root 模式解析、su 参数化执行、pipeline timeout 和进程组清理。
- 可持续多轮输入、完整历史回放、滚动和 terminal guard 的 TUI。
- openpty 主执行器、pipeline fallback、交互双向桥接、resize、ANSI 过滤和实时 output sink。
- Ctrl+C 对 LLM 请求/退避及命令进程组的取消路径；编辑命令重新分类。
- endpoint/model/api-type CLI 覆盖，覆盖后统一配置校验。
- 隔离子进程 SIGINT 回归测试，验证 Agent 退出及 PTY 进程组回收。
- 单 frame Agent TUI、内嵌确认弹窗、实时状态/输出，以及真实伪终端生命周期回归测试。
- 缺失配置时先从 OpenAI、DeepSeek、Moonshot/Kimi、SiliconFlow、Ollama 或自定义 Base URL 中方向键选择，再以可见输入填写 API Key 并继续启动；TUI `/config` 复用该流程，可原子更新 provider 配置并热重载客户端。
- TUI 底部输入框独占一行，运行状态、轮数和剩余上下文在下一行显示。
- 对话历史按用户、Tool、Agent、命令、成功和错误等语义类型使用不同颜色。
- 对话历史逐条持久化到默认配置目录下的 `0600` JSON Lines 日志，供异常排查。
- 只读应用版本查询及其命令替换循环不再误判为修改操作；替换内副作用仍需确认。
- TUI 会捕获并在退出时恢复鼠标事件，历史窗口支持滚轮及 PageUp/PageDown 浏览。
- TUI、初始化向导与安全确认支持中文/英文，默认中文；启动历史区预置常用 Android 任务和操作说明，且不进入模型上下文。
- 仅用户输入文字所在行使用低亮度淡灰背景；快捷键上分割线、状态行和下分割线保持终端默认背景。
- `android-run.sh` 会先执行 `adb root`、等待 adbd 并验证 UID 0，再交叉编译、推送和启动；不支持 root adbd 时才回退 `su -c`，私有配置不可读则提前失败。
- `android-run.ps1` 使用 NDK `windows-x86_64` LLVM 工具链在 Windows PowerShell 原生完成同等的交叉编译、推送、root/su 回退和启动流程，无需 Bash 或 WSL。
- `android-run-linux.sh` 与 `android-run-windows.ps1` 可将脚本同目录中的预编译 `nl2sh` 直接推送并启动，跳过 Rust/NDK 编译，同时保留 root adbd 优先、`su` 回退和私有配置保护。
- 部署文档说明了文件存在却由 ABI/ELF interpreter 不匹配引发 `No such file or directory` 的情况，并给出 AArch64/ARMv7 识别、重建和验证步骤。
- adb TTY 将鼠标 SGR 序列拆成按键字符时，输入边界会过滤 `[<数字;数字;数字M/m`，且空闲 Esc 不再误清已有输入；确认弹窗 Esc 行为不变。
- `/dev/null` 重定向与 fd 复制不再把只读诊断误判为修改或连带要求 root；真实文件写入和命令副作用仍受确认保护，strict 仍按定义确认全部命令。
- 工具执行期间保留实时输出，完成后结果默认折叠并可用 F2 展开/收起；模型仍接收完整结果，最终答复被要求使用用户语言及可读表格或文本总结。
- Agent Markdown 原生映射为 ratatui 行与样式，支持标题、行内样式、列表、引用、代码块、链接和分隔线；表格按 Unicode 宽度对齐、换行，并在窄屏降级为键值列表。
- F2 展开工具结果后按 ratatui 实际换行高度定位和滚动，不再用逻辑历史条目数限制大结果，长命令与输出可完整浏览。
- 交互命令退出后恢复备用屏幕与鼠标捕获，并使 ratatui 强制完整重绘，避免第二轮对话只显示结果而框架消失。
- 配置、安全、root、HTTP mock 和 Agent loop 测试源码。
- GitHub Actions release workflow：tag 推送自动构建 AArch64/ARMv7 Android release、打包快速启动脚本并发布 Release。
- 面向普通用户的中文使用说明，覆盖 ADB 连接、Linux/Windows 启动、32/64 位选择和常见故障，并纳入所有 release 压缩包。
- 内存查询示例截图嵌入中文使用说明与 README，release 压缩包同步包含 `screenshots/`，保证打包后的说明图文完整。

## In Progress

- 扩展真机 smoke test，覆盖 root 提权、修改确认、超时和全屏交互程序。

## Pending / Known Issues

- PTY 已在 API 34 ARMv7 真机执行只读命令；不同 su/fullscreen 程序仍可能需要兼容调整。
- `android-run.sh` 与 `android-run.ps1` 默认构建 AArch64；仅支持 `armeabi-v7a` 的设备必须显式设置 `RUST_TARGET=armv7-linux-androideabi`，否则 Android 会因缺少 `/system/bin/linker64` 报 `No such file or directory`。
- Agent TUI 在 LLM 和捕获式命令执行期间保持同一 ratatui frame；全屏交互命令会临时挂起 TUI，退出后恢复并完整重绘。

## Technical Decisions

- reqwest 关闭默认 feature，只使用 rustls、JSON 和 stream feature。
- Provider JSON 与 Agent 通过统一类型隔离。
- su 命令作为独立 argv 传递，避免 nl2sh 自己做不安全 shell quoting。
- 安全规则只允许自定义规则提高风险，内置规则不可被清空。
- Android 使用 `/system/bin/sh`，非 Android 开发主机条件使用 `/bin/sh`。

## Verification Performed

- `cargo check`：通过。
- `cargo test`：通过，测试覆盖配置/CLI、安全、历史日志、root、双 LLM 协议、重试/timeout、Agent 历史/失败/取消、真实 SIGINT、PTY、初始化顺序、TUI 重配置、双行布局和语义配色。
- `android-run.ps1`：PowerShell AST 语法解析与 `git diff --check` 通过；因当前未授权实际部署，未执行 adb/NDK 真机流程。
- `cargo fmt --all -- --check`：通过。
- Release 用户说明打包：`release.yml` 已通过 `actionlint`，已在本地模拟 AArch64/ARMv7 目录并确认 `.tar.gz` 和 `.zip` 均包含 `使用说明.md`。
- `cargo clippy --all-targets -- -D warnings`：通过。
- `cargo build --release`：通过。
- `RUSTDOCFLAGS='-D missing_docs' cargo doc --no-deps`：通过。
- `./cross-compile.sh`：通过；使用 `/home/ernest/Android/Sdk/ndk/28.2.13676358` 构建 Android 26 AArch64 PIE，解释器为 `/system/bin/linker64`。
- ARMv7 真机：`--version`、Responses Agent 两轮请求、`getprop` PTY 执行/结果回传和 `adb shell -t` TUI Ctrl+Q 恢复均通过。

## Next Steps

1. 在 root 与非 root 设备补测确认、提权、超时和全屏程序。
2. 根据真机结果优化窄屏布局和全屏交互程序切换。
3. 在真实 GitHub Actions 上验证 release workflow（`workflow_dispatch` 草稿）并发布首个版本。
