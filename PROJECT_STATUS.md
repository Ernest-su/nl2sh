# Project Status

Last Updated: 2026-08-23

## Recent Changes

- 新增独立 `ProviderMetadataClient`，分别适配 OpenAI、DeepSeek、SiliconFlow 的模型列表与 Ollama 原生模型详情；配置支持上下文窗口/最大输出 Token 覆盖，已知窗口用于在状态栏估算最后一次请求的上下文占用率。
- Agent 任务会累计所有模型步骤返回的输入/输出 Token，并在 TUI 状态栏展示本次任务合计；新增 `/models` 在线模型选择，网络或协议失败时回退手工输入，凭据和原始 Provider 响应不写入审计日志。
- 启动小火车由约 10 FPS 提升到约 30 FPS，并将每帧位移由两列改为一列，以减少跳格卡顿；TUI 事件轮询与异步刷新周期同步缩短，动画约 4.1 秒后结束。
- 修复启动小火车车头与向右行驶方向相反的问题；车体和烟雾现朝向右侧，`NL2SH` 字样保持正向，奇偶宽度下仍会让车头贴到内容区右边缘后再驶出。
- 修复 ADB TTY 在 `/` 命令菜单首尾继续按方向键时可能把拆分的 CSI/SS3 尾字符写入输入框、导致菜单消失的问题；菜单打开时会将 `Esc [ A/B/C/D` 与 `Esc O ...` 重新组合为方向键，首尾循环选择保持不变。
- 修复部分 ADB 宿主终端在 LLM 流式临时文本切换为最终 Markdown 时留下旧渐变字符的问题；流结束后由 TUI 主线程执行一次完整重绘，不改变终端模式、PTY 或安全流程。
- Chat Completions 与 Responses 的模型文本现在通过 SSE 增量显示到 Agent TUI；生成中的尾部使用 TrueColor/ANSI 256 语义渐变动画，完成后立即恢复普通 Markdown 样式。流式工具参数仍完整聚合后才进入安全与确认链。
- 每个 Android Agent 任务会向 system prompt 动态附加一次低敏感环境摘要（API level、ABI、shell、UID、root/su 能力）；失败时安全降级，不采集设备标识或网络信息，也不影响安全与确认链。
- 新增本地 `/exit` 命令，可从命令候选菜单或直接输入安全退出 TUI，行为与 Ctrl+Q 一致且不会进入模型上下文。
- 修复启动小火车以每帧两列移动时可能跨过右边缘贴边帧的问题；奇数和偶数宽度下车头都会抵达内容区最右列后再完整驶出。
- 首次启动欢迎页会在佛祖图下方播放一次带动态蒸汽和 `NL2SH` 车身字样的纯 ASCII 小火车；`android-build-run.sh` 会先把宿主终端行列数同步到 Android PTY，动画再按真实内容宽度移动；窄屏按视口裁剪，且不进入会话、审计或模型历史。
- 佛祖终端图的 `\\`、`/`、`|`、`=`、`^` 光芒与轮廓字符使用独立的装饰金色 token 加粗显示，文字和面部细节保持正文色；该 token 不复用安全警告色，历史内容仍为无 ANSI 的纯文本。
- README 在已知限制之后新增“支持项目”区块，包含 Star/Issue 话术、赞赏说明、可点击的在线微信赞赏码及备用文字链接；TUI 仍保持纯文本方案。
- 修正终端佛祖祝福图的 Unicode 显示宽度：含中文的祝福行与 ASCII 外框统一为 65 列，避免右侧突出。
- 缺失配置时不再自动运行启动向导，而是直接进入不可执行模型任务的 TUI；新增 `/provider` API 配置和 `/model` 模型配置入口，`/config` 保留完整配置能力。
- README 顶部居中展示仓库内的 `assets/logo.png`；启动欢迎页与 `/help` 显示项目 Star/Issue 支持链接、在线微信赞赏链接和纯文本终端祝福图，TUI 仍不内嵌图片或渲染二维码点阵。
- 新增本地 `/help` 帮助与 `/clear` 会话清理命令；清理可见对话、模型上下文和输入历史，但保留审计日志。
- 修复 Android ADB TUI 退出后宿主终端仍处于 SGR 鼠标追踪模式的问题：Rust 正常/异常恢复路径统一关闭鼠标捕获，`android-build-run.sh` 在 ADB 正常、失败或中断退出后额外执行宿主侧兜底恢复。
- 默认 `max_agent_steps` 由 8 提升到 24、`max_context_turns` 由 10 提升到 16，以支持安装后验证、多阶段诊断等更长任务；`config.toml.example` 与默认值测试同步更新。

## Current Phase

0.2.0 发布阶段：本地发布门禁完成，等待 GitHub Actions 构建并发布双 ABI 产物。

## Overall Status

- Product positioning: Android 原生 shell 版的类 Hermes AI Agent；核心程序以单个可执行文件交付，提供多轮 Tool Calling 和丰富 TUI，不声称与 Hermes API 或插件兼容。
- Build status: 0.2.0 的 `cargo check --all-targets` 与 Linux release 构建通过。
- Test status: 0.2.0 的单元、Agent loop 与非 TUI 集成测试通过；两个依赖原始 ANSI 文本连续匹配的伪终端测试受启动动画差分输出影响，发布前需修复。
- Android cross-compile status: 0.2.0 已使用 NDK r28c、API 26 成功构建 `aarch64-linux-android` 与 `armv7-linux-androideabi` release 产物。
- Android device validation: 已在 API 34 `armeabi-v7a` 设备完成部署、Agent/PTY 和 TUI smoke test。
- CI release workflow: 已添加 `.github/workflows/release.yml`，在推送 `v*` tag 时用 GitHub Actions 并行交叉编译 `aarch64-linux-android` 与 `armv7-linux-androideabi`，将两个程序放入统一包的 ABI 子目录，并与自动选择设备/ABI 的 Linux/Windows BAT 启动脚本、`config.toml.example`、`使用说明.md` 打包为单一 `.tar.gz`/`.zip`，附带 SHA256 校验和发布到 GitHub Release；`workflow_dispatch` 可手动触发草稿发布。
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
- 缺失配置时直接进入 TUI，普通任务在 Provider 配置完成前被本地拒绝；`/config` 可完成全部设置，`/provider` 配置 API Endpoint、API Key 和协议，`/model` 单独配置模型，并在保存后热重载客户端。
- TUI 底部输入框独占一行，运行状态、轮数和剩余上下文在下一行显示。
- 对话历史按用户、Tool、Agent、命令、成功和错误等语义类型使用不同颜色。
- 对话历史逐条持久化到默认配置目录下的 `0600` JSON Lines 日志，供异常排查。
- 只读应用版本查询及其命令替换循环不再误判为修改操作；替换内副作用仍需确认。
- TUI 捕获滚轮以浏览历史；按住 Shift 拖选时由宿主终端原生高亮选区并通过右键菜单复制，PageUp/PageDown 仍可浏览历史。
- 输入框使用统一主题的青蓝色闪烁光标和聚焦边框，支持 Left/Right/Home/End/Delete 定位编辑，并可用 Up/Down 调取当前会话已提交的输入历史。
- 输入以 `/` 开头时显示垂直命令候选菜单；Up/Down 选择，Enter 补全，列出 `/help`、`/clear`、`/config`、`/provider` 和 `/model`。
- TUI、初始化向导与安全确认支持中文/英文，默认中文；启动历史区预置常用 Android 任务和操作说明，且不进入模型上下文。
- 输入行和低权重状态行使用统一主题的 `background_alt`，快捷键分隔线使用 `border`/`border_focus`，其余 frame 使用非纯黑 `background`。
- `android-build-run.sh` 会选择或连接 ADB 设备、查询 ABI 并自动选择对应 Rust target，再执行 root adbd、交叉编译、推送和启动；不支持 root adbd 时才回退 `su -c`，私有配置不可读则提前失败。
- `android-build-run.ps1` 使用 NDK `windows-x86_64` LLVM 工具链在 Windows PowerShell 原生完成同等的设备选择、ABI 自动编译、root/su 回退和启动流程，无需 Bash 或 WSL。
- `android-run-linux.sh` 与可双击的 `android-run-windows.bat` 会在无设备时提示连接网络 ADB、单设备自动选择、多设备按编号选择，再查询 ABI 并从 `bin/arm64-v8a` 或 `bin/armeabi-v7a` 推送对应程序；同时保留 root adbd 优先、`su` 回退和私有配置保护。
- 部署文档说明了文件存在却由 ABI/ELF interpreter 不匹配引发 `No such file or directory` 的情况，并给出 AArch64/ARMv7 识别、重建和验证步骤。
- adb TTY 将鼠标 SGR 序列拆成按键字符时，输入边界会过滤 `[<数字;数字;数字M/m`，且空闲 Esc 不再误清已有输入；确认弹窗 Esc 行为不变。
- `/dev/null` 重定向与 fd 复制不再把只读诊断误判为修改或连带要求 root；真实文件写入和命令副作用仍受确认保护，strict 仍按定义确认全部命令。
- 工具执行期间保留有界实时输出，完成后结果默认折叠并可用 F2 展开/收起；模型接收带显式截断标记的有界结果，最终答复被要求使用用户语言及可读表格或文本总结。
- Agent Markdown 原生映射为 ratatui 行与样式，支持标题、行内样式、列表、引用、代码块、链接和分隔线；表格按 Unicode 宽度对齐、换行，并在窄屏降级为键值列表。
- F2 展开工具结果后按 ratatui 实际换行高度定位和滚动，不再用逻辑历史条目数限制大结果，长命令与输出可完整浏览。
- 交互命令退出后恢复备用屏幕与鼠标捕获，并使 ratatui 强制完整重绘，避免第二轮对话只显示结果而框架消失。
- 配置、安全、root、HTTP mock 和 Agent loop 测试源码。
- GitHub Actions release workflow：tag 推送自动构建 AArch64/ARMv7 Android release、打包快速启动脚本并发布 Release。
- `pack-release.sh` 与 `pack-release.ps1` 可在 Linux/Windows 本地构建双 ABI，并输出与 GitHub Release ZIP 相同的 `nl2sh-android/` 目录结构和 SHA256 校验文件。
- 面向普通用户的中文使用说明，覆盖 ADB 连接、Linux/Windows 启动、自动设备/32/64 位选择和常见故障，并纳入统一 release 压缩包。
- `screenshots/nl2sh.gif` 动态操作演示嵌入中文使用说明与 README，release 压缩包同步包含 `screenshots/`，保证打包后的说明动图完整。
- 已建立 `UI_DESIGN.md`，统一定义深色 TUI palette、语义颜色、各界面区域样式、ANSI 256 fallback、实现边界和验收标准；规范明确颜色不得改变或替代安全分类与确认流程。
- 已在 `src/tui/theme.rs` 实现集中式 Theme/Palette 与 TrueColor/ANSI 256 能力选择，并迁移标题栏、对话、Markdown、工具结果、表格、快捷键、输入区、状态栏、命令菜单和确认弹窗；长正文与 stdout 不再继承成功绿色。
- 命令审批改为固定 `1-6` 列表，支持方向键/Enter 与 `y/n/a/e/i/t` 别名；可在当前 Agent 任务内记住完全相同的普通命令，但 Root、Dangerous、Critical 和强确认命令始终禁用该选项，且许可不持久化、不做前缀匹配。
- 审批区域使用完整风险色边框和统一 `background_alt` 面板背景；阶段切换保持稳定最小高度并清空整个面板，避免列表字符残留到强确认或编辑画面。
- 审批面板锚定在输入区正上方的左下角；初始审批忽略孤立 Esc 和大写 CSI 尾字符，避免 adb 将方向键拆分后误触拒绝或 always 导致弹窗消失。
- MIT `LICENSE` 已纳入仓库；Cargo 版本为 0.2.0。
- 实时 TUI、捕获式工具结果、发给模型的 Tool Result、JSONL 单事件和单文件均有可配置上限；截断会插入明确标记。
- TUI 输出与历史生命周期已从 session 控制器拆为独立模块，同时保留新的审批菜单和任务级精确命令许可。

## In Progress

- 扩展真机 smoke test，覆盖 root 提权、修改确认、超时和全屏交互程序。

## Pending / Known Issues

- PTY 已在 API 34 ARMv7 真机执行只读命令；不同 su/fullscreen 程序仍可能需要兼容调整。
- 源码编译启动脚本仅自动映射 `arm64-v8a` 与 `armeabi-v7a`；其他设备 ABI 会明确拒绝，显式 `RUST_TARGET` 与设备不匹配时也会停止。
- Agent TUI 在 LLM 和捕获式命令执行期间保持同一 ratatui frame；全屏交互命令会临时挂起 TUI，退出后恢复并完整重绘。
- 新主题已完成渲染与样式测试，仍需在不同 adb shell 宿主的 TrueColor/ANSI 256、窄屏和实际电视显示效果下做真机可读性验证。

## Technical Decisions

- reqwest 关闭默认 feature，只使用 rustls、JSON 和 stream feature。
- Provider JSON 与 Agent 通过统一类型隔离。
- su 命令作为独立 argv 传递，避免 nl2sh 自己做不安全 shell quoting。
- 安全规则只允许自定义规则提高风险，内置规则不可被清空。
- Android 使用 `/system/bin/sh`，非 Android 开发主机条件使用 `/bin/sh`。

## Verification Performed

- Provider 元数据第二阶段：WSL2 `cargo check`、41 项库测试、10 项 Agent loop 测试和 6 项配置测试通过；Android ARMv7 release 构建、推送后在真实 ADB PTY 使用 `/models` 成功拉取 DeepSeek 模型及 1,000,000 Token 上下文元数据，并完成选择和 TUI 恢复。
- Token 统计与 `/models` 第一阶段：WSL2 `cargo check` 通过；`cargo test` 的 40 项库测试、CLI、Agent loop、取消、配置、日志、LLM mock、PTY、root 与安全测试通过，两个已知启动动画伪终端用例仍因 ANSI 差分文本匹配超时；Android API 设备完成 ARMv7 release 构建、推送及 `--version` 启动验证。
- 本地统一 ZIP 打包：`bash -n pack-release.sh` 与 `pack-release.ps1` PowerShell AST 解析通过；Windows 脚本使用 NDK 双 ABI release 实际构建成功，生成的 `dist/nl2sh-android.zip` 包含与 GitHub workflow 一致的 ABI 子目录、启动脚本、配置示例、说明和截图，`dist/SHA256SUMS` 复算一致。
- 源码编译启动脚本：`bash -n android-build-run.sh` 与 `android-build-run.ps1` PowerShell AST 解析通过；在 ARMv7 ADB 设备上，两者均自动选择唯一设备、识别 `armeabi-v7a` 并把 Rust target 映射为 `armv7-linux-androideabi`，显式指定不匹配的 `aarch64-linux-android` 时在编译和推送前明确拒绝。
- 双 ABI 统一发布包：`bash -n android-run-linux.sh`、`cargo fmt --all -- --check` 与 `cargo check` 通过；模拟组装的 `.tar.gz`/`.zip` 均包含 `bin/arm64-v8a/nl2sh`、`bin/armeabi-v7a/nl2sh`、Linux/BAT 脚本、配置示例、用户说明和截图；Windows BAT 在 ARMv7 ADB 设备自动选择唯一设备、识别 `armeabi-v7a`、推送 32 位程序并正常进入及退出 TUI。`cargo test` 的非 TUI 测试通过，两个已知启动动画伪终端用例仍因 ANSI 差分文本匹配超时而失败。
- ADB 命令菜单方向键：在 API 28 ARMv7 设备向 `/` 菜单分别以超过 500ms 的间隔注入 `Esc`、`[`、`A/B`；第一项向上循环到最后一项、最后一项向下循环回第一项，菜单保持显示且输入始终为 `/`，无 CSI 字母残留。
- LLM 流结束重绘：在 API 28 ARMv7 设备部署 release 二进制，通过 Responses 模型生成不同长度的三行中文；生成期间增量渐变正常，完成时执行一次完整重绘，最终 Markdown 无旧字符残留，Ctrl+Q 后终端正常恢复。
- LLM 流式输出：`cargo fmt --all -- --check`、`cargo check --target aarch64-linux-android` 与 `cargo test --target aarch64-linux-android --no-run` 通过；新增流式文本/工具参数聚合和 TUI 渐变样式测试，其 Android 测试二进制编译通过。
- 0.2.0 发布门禁：`cargo fmt --all -- --check`、`cargo check --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo build --release`、`RUSTDOCFLAGS='-D missing_docs' cargo doc --no-deps` 与 `actionlint .github/workflows/release.yml` 通过。
- 0.2.0 Android 交叉编译：NDK r28c/API 26 的 AArch64 与 ARMv7 release 均通过，分别验证为使用 `/system/bin/linker64` 的 64 位 PIE 和使用 `/system/bin/linker` 的 32 位 PIE。
- 0.2.0 测试：34 项库测试和 10 项 Agent loop 测试通过；`cargo test --all-targets` 中两个旧伪终端测试因启动动画的 ANSI 差分输出不再形成连续原始文本而超时，其余测试通过。
- `cargo check`：通过。
- `cargo test --all-targets`：通过，共 68 项测试；覆盖配置/CLI、安全、历史日志及限额、root、双 LLM 协议、重试/timeout、Agent 历史/失败/取消与模型 Tool Result 截断、真实 SIGINT、PTY、初始化顺序、TUI 重配置、编号审批与任务级精确许可、审批面板定位/跨帧清理/方向键拆分、双行布局、TrueColor/ANSI 256 palette、Markdown/表格/工具结果/确认界面的语义配色。
- `android-build-run.ps1`：PowerShell AST 语法解析与 `git diff --check` 通过。
- `cargo fmt --all -- --check`：通过。
- Release 用户说明打包：`release.yml` 已通过 `actionlint`，AArch64/ARMv7 模拟打包确认 `.tar.gz` 和 `.zip` 均包含 `使用说明.md`。
- `cargo clippy --all-targets -- -D warnings`：通过。
- `cargo build --release`：通过。
- `RUSTDOCFLAGS='-D missing_docs' cargo doc --no-deps`：通过。
- `./cross-compile.sh`：通过；使用 NDK r28c 构建 Android 26 AArch64 PIE，解释器为 `/system/bin/linker64`。
- ARMv7 真机：`--version`、Responses Agent 两轮请求、`getprop` PTY 执行/结果回传和 `adb shell -t` TUI Ctrl+Q 恢复均通过。

## Next Steps

1. 在 root 与非 root 设备补测确认、提权、超时和全屏程序。
2. 根据真机结果优化窄屏布局和全屏交互程序切换。
3. 观察 `v0.2.0` GitHub Actions，确认双 ABI 归档与 Release 发布成功。
