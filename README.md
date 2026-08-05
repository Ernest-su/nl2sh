# nl2sh

Natural Language to Shell 是一个面向 Android 原生 `adb shell` 的单二进制 AI Shell Agent。它把自然语言交给 OpenAI 兼容模型，通过 Tool Calling 生成命令，在本地安全分类和确认之后执行，并把真实结果返回模型。

## 特性与安全边界

- 默认使用多轮 Agent Tool Calling；也支持只生成单条命令的 Command 模式。
- 支持 Chat Completions 与 Responses API、自定义 OpenAI 兼容 endpoint。
- `balanced` 默认策略自动执行只读查询、确认修改操作、二次确认危险操作。
- LLM 不能决定确认、风险等级、root 提升或超时；用户编辑后的命令必须重新分类。
- 支持当前用户、自动提升和强制 root 模式。非 root 提升使用参数化的 `su -c <command>`，不拼接 shell 字符串。
- crossterm + ratatui 终端界面通过 RAII 和 panic hook 恢复 raw mode、alternate screen、鼠标捕获和光标。
- 主要目标为 Android API 26+、`aarch64-linux-android` 和 `/data/local/tmp`。不依赖或专门支持 Termux。

默认执行路径使用 Unix `openpty`：slave 成为子进程 controlling terminal，master 非阻塞读取，stdout/stderr 合并，超时会清理整个进程组。识别到全屏/交互命令时，nl2sh 临时使用本地 raw mode，桥接 stdin/master 输出并同步窗口尺寸，结束后恢复终端。不同 Android 终端和全屏应用仍可能存在兼容差异，详见 `PROJECT_STATUS.md`。

## 构建

需要 stable Rust（edition 2021）。桌面 Unix 环境用于开发验证：

```bash
cargo build
cargo test
cargo build --release
```

HTTP 使用 rustls，未启用 native-tls。

## Android 交叉编译和部署

安装目标并配置 Android NDK：

```bash
rustup target add aarch64-linux-android
export ANDROID_NDK_HOME=/path/to/android-ndk
./cross-compile.sh
adb push target/aarch64-linux-android/release/nl2sh /data/local/tmp/
adb shell chmod +x /data/local/tmp/nl2sh
adb shell -t /data/local/tmp/nl2sh
```

脚本默认 API 26 和 `aarch64-linux-android`，可通过 `ANDROID_API_LEVEL` 与 `RUST_TARGET=armv7-linux-androideabi` 覆盖。也接受 `ANDROID_NDK_ROOT`。它不要求 Termux、bash 位于 Android 设备上或 GNU coreutils。

也可以一键完成 release 交叉编译、推送、授权并进入带 TTY 的 adb shell 启动应用：

```bash
export ANDROID_NDK_HOME=/path/to/android-ndk
./android-run.sh
```

Windows PowerShell 可使用原生 NDK Windows 工具链执行同一流程，不需要 Bash 或 WSL：

```powershell
$env:ANDROID_NDK_HOME = "C:\Android\Sdk\ndk\28.2.13676358"
.\android-run.ps1
```

默认推送到 `/data/local/tmp/nl2sh`。可用 `ANDROID_DIR=/data/local/tmp/tools` 修改设备目录，多设备时用 `ADB_SERIAL=<serial>` 指定设备，ARMv7 设备可同时设置 `RUST_TARGET=armv7-linux-androideabi`。脚本要求主机 `PATH` 中可找到 `adb`，设备端仅使用 Android 自带的 `mkdir`、`chmod` 和 shell。连接后会先执行 `adb root`、等待 adbd 重启并验证 `id -u`；root adbd 成功时，后续推送和启动均以 root 进行。设备不支持 `adb root` 时才尝试 `su -c`，两者都不可用且已有 `0600 config.toml` 不可读时会提前报错，不会放宽 API Key 配置文件权限。

已有预编译的 Android `nl2sh` 时，可把它与对应脚本放在同一目录，直接从推送步骤开始，无需 Rust 或 NDK。Linux 使用 `android-run-linux.sh`，Windows PowerShell 使用 `android-run-windows.ps1`；两者同样支持 `ANDROID_DIR` 和 `ADB_SERIAL`：

```bash
chmod +x android-run-linux.sh nl2sh
./android-run-linux.sh
```

```powershell
$env:ADB_SERIAL = "device-serial"
.\android-run-windows.ps1
```

### 通过 GitHub Actions 自动发布

仓库内置 `.github/workflows/release.yml`。推送 `v*` tag（如 `git tag v0.1.0 && git push --tags`）会触发 GitHub Actions：并行交叉编译 `aarch64-linux-android`（arm64-v8a）与 `armv7-linux-androideabi`（armeabi-v7a）两个 release 产物，每个产物连同 `android-run-linux.sh`、`android-run-windows.ps1` 和 `config.toml.example` 打包成 `.tar.gz` 与 `.zip`，并附带 `SHA256SUMS` 发布到对应 tag 的 GitHub Release。Actions 页的 `workflow_dispatch` 可手动触发并生成草稿 Release（tag 通过输入指定）。

下载适合设备 ABI 的包解压后，Linux/macOS 直接运行 `./android-run-linux.sh`，Windows PowerShell 运行 `.\android-run-windows.ps1`；脚本会查找同目录的预编译 `nl2sh` 并完成 root adbd/`su` 回退部署，无需 Rust 或 NDK。

### Android 提示 `No such file or directory`

如果 `/data/local/tmp/nl2sh` 明明存在且已有执行权限，但运行时仍提示：

```text
/system/bin/sh: ./nl2sh: No such file or directory
```

这通常不是文件路径不存在，而是二进制 ABI 与设备不匹配，导致 Android 找不到 ELF 指定的动态加载器。例如，只支持 `armeabi-v7a` 的 32 位设备不能运行默认生成的 `aarch64-linux-android` 64 位程序；该程序请求 `/system/bin/linker64`，而 32 位设备只有 `/system/bin/linker`。

先检查设备 ABI 和远端二进制：

```powershell
adb shell getprop ro.product.cpu.abi
adb shell getprop ro.product.cpu.abilist
adb shell file /data/local/tmp/nl2sh
```

- `arm64-v8a`：使用默认的 `aarch64-linux-android`。
- `armeabi-v7a` 且 ABI 列表中没有 `arm64-v8a`：必须构建 `armv7-linux-androideabi`。

Windows PowerShell 构建并部署 ARMv7 版本：

```powershell
rustup target add armv7-linux-androideabi
$env:RUST_TARGET = "armv7-linux-androideabi"
.\android-run.ps1
```

Linux/macOS 使用相同目标：

```bash
rustup target add armv7-linux-androideabi
RUST_TARGET=armv7-linux-androideabi ./android-run.sh
```

重新推送后，`adb shell file /data/local/tmp/nl2sh` 在 ARMv7 设备上应显示 `ELF 32-bit`、`ARM` 和 `/system/bin/linker`，不应显示 `64-bit arm64` 或 `/system/bin/linker64`。如果 ABI 已匹配，再检查文件权限、ELF interpreter 是否存在，以及二进制是否确实由 Android NDK 而非桌面工具链构建。

## 配置

默认配置位于解析符号链接后的可执行文件目录，名称为 `config.toml`。在 TTY 中启动且配置不存在时，可用 ↑/↓（或 j/k）从 OpenAI、DeepSeek、Moonshot/Kimi、SiliconFlow、Ollama 和自定义 Base URL 中选择，再以普通可见输入填写 API Key，保存后直接继续启动；`nl2sh --init` 也可显式创建配置且不会覆盖已有文件。配置仍以 `0600` 权限创建，请注意终端屏幕和录屏中可能保留输入的 Key。也可传入 `--config /path/config.toml`。

```bash
cp config.toml.example config.toml
```

配置优先级为 CLI 参数、`NL2SH_API_KEY`、`config.toml`、字段默认值。CLI 可用 `--endpoint`、`--model`、`--api-type` 覆盖 provider 设置；覆盖后统一校验，因此可以修正文件中的对应无效值。空 API Key 适用于本地服务，此时不会发送空 Authorization header。不要提交真实 key。

`api_type` 可选 `responses` 或 `chat_completions`。兼容服务对协议的支持并不一致，nl2sh 不会因一次业务错误擅自切换协议。

`execute_user_mode`：

- `auto`：UID 0 直接运行；普通命令保持当前用户；明确需要 root 时才尝试 `su -c`。
- `normal`：永不自动调用 `su`。
- `root`：UID 非 0 时必须通过 `su`，失败时不静默降级。

`history_log_file` 默认为 `nl2sh.log`，相对路径按 `config.toml` 所在目录解析。日志采用逐行 JSON，记录用户输入、命令、输出、结果和错误并在每条记录后刷新；新文件权限为 `0600`。日志可能包含命令输出中的设备信息，排查完成后应按实际保密要求保管或清理，但不会写入 API Key。

`ui_language` 控制终端界面语言，可选 `zh_cn` 或 `en`，默认 `zh_cn`。首次初始化及 `/config` 重配置会先询问界面语言，之后的向导、状态栏、确认弹窗和快捷键说明使用所选语言。`/config` 会优先选中当前 URL 对应的内置服务商，未知 URL 则进入自定义项；API Key 留空会保留当前配置。每次启动时，对话历史区会展示常用任务示例以及 `/config`、Enter、滚轮/PageUp/PageDown、Ctrl+C、Ctrl+Q 的操作说明；这些提示不会发送给模型。

## 使用

```bash
nl2sh                         # TUI Agent 模式
nl2sh "列出 /data 最大的十个文件" # 单次 Agent 请求
nl2sh --mode command "查看内存"   # 生成、分类并执行单条命令
nl2sh --mode command --dry-run "查看内存"
nl2sh --endpoint http://127.0.0.1:11434/v1 --model local --api-type chat_completions "查看系统"
nl2sh --no-pty --ascii
```

Command 模式生成、分类后执行单条命令；`--dry-run` 只展示。修改或危险命令在无 TTY 时会拒绝，不能用管道伪造确认。需要多轮执行和结果回传时使用默认 Agent 模式。Agent TUI 在请求和捕获式执行期间保持活跃，并在界面内显示输出与确认弹窗；命令运行时实时展示输出，完成后工具结果默认折叠，按 F2 可统一展开或收起，完整结果仍写入日志并回传模型。Agent 总结支持终端 Markdown 渲染：标题、粗体、斜体、行内代码、列表、引用、代码块、链接、分隔线和表格；表格按中英文显示宽度对齐、过长单元格自动换行，极窄屏幕降级为键值列表。底部仅输入文字所在行使用低亮度淡灰背景，上下分割线和状态行保持终端默认背景。历史窗口支持鼠标滚轮和 PageUp/PageDown，另支持 Enter 提交、`/config` 重新配置 provider、Ctrl+C 取消当前任务（空闲时清空输入）、Ctrl+Q 安全退出。`/config` 会保留安全和执行设置，依次更新 Base URL、Key、模型和 API 类型，然后重建客户端返回 TUI。

风险等级为 `ReadOnly`、`Mutating`、`Dangerous`、`Critical`。内置检测覆盖危险删除、格式化、块设备写入、递归根权限修改、重启/关机、分区擦除和读写 remount；自定义规则使用 `[[security_rules]]` 添加，不能替换内置规则。

包版本查询（如 `pm list packages --show-versioncode`、`dumpsys package … | grep versionName`，以及只读的命令替换循环）按只读操作处理，不会因为使用 `$()` 本身反复弹出安全确认；替换内部的修改或危险命令仍会重新分类并确认。

## 测试和真机 smoke test

```bash
cargo fmt --all -- --check
cargo check
cargo test
cargo build --release
cargo clippy --all-targets -- -D warnings
```

Android 真机建议依次验证：启动/退出后终端恢复；`id` 和 `getprop` 只读执行；`touch` 确认；`rm -rf /` 二次确认并拒绝；normal/auto/root 三种模式；命令超时和 Ctrl+C；Chat Completions 与 Responses 各一个 endpoint。

## 已知限制

- 已通过 Android NDK r28c、API 26 的 AArch64/ARMv7 release 交叉编译，并在 API 34 ARMv7 设备完成 Agent、PTY 和 TUI 基础 smoke；root 提权及全屏程序仍待扩展验证。
- Agent TUI 在 LLM 和捕获式命令期间保持同一 frame；全屏交互程序需要临时挂起 TUI，结束后自动恢复。
- 交互 PTY 已支持双向桥接和 resize，但尚未在 Android 真机的各类全屏应用上验证。
- Responses 对话适配覆盖常见 function call 结构，不保证所有兼容厂商的扩展字段。

## 文档

- `ARCHITECTURE.md`：模块、数据流、安全、执行与扩展架构。
- `AGENTS.md`：后续 AI 维护约束。
- `PROJECT_PLAN.md`：阶段计划与实际状态。
- `PROJECT_STATUS.md`：当前验证和限制。
- `CHANGELOG.md`：用户可见变更。
