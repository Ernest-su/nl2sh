# nl2sh 项目完整生成 Prompt

## Role

你是一名资深 Rust 系统程序员，同时熟悉 Android Native 开发、Linux/Android 终端环境、PTY、TUI、异步 Rust、OpenAI 兼容 API、Agent Tool Calling、CLI 安全策略、Android root / `su -c` 以及 Android NDK 交叉编译。

请创建一个完整、可编译、可运行的 Rust 项目。

项目名称：

```text
nl2sh
```

全称：

```text
Natural Language to Shell
```

项目目标：

创建一个运行在 Android 原生 shell 环境中的单二进制 AI Shell Agent，功能类似简化版 OpenAI Codex CLI。

核心链路：

```text
自然语言输入
      ↓
LLM Agent
      ↓
Tool Calling / Shell Command
      ↓
安全检查
      ↓
用户确认
      ↓
PTY 执行
      ↓
实时结果展示
      ↓
执行结果回传 Agent
      ↓
根据需要继续多轮执行
```

---

# 一、目标运行环境

## 1.1 优先支持

必须优先支持：

- Android `aarch64-linux-android`
- `adb shell`
- Android 原生终端环境
- root Android 设备
- 非 root 但存在 `su` 的 Android 设备
- `/data/local/tmp` 部署方式

典型部署方式：

```bash
adb push target/aarch64-linux-android/release/nl2sh /data/local/tmp/
adb shell chmod +x /data/local/tmp/nl2sh
adb shell -t /data/local/tmp/nl2sh
```

## 1.2 不需要支持

不需要专门支持：

- Termux
- Linux 桌面发行版优化
- Windows
- macOS

可以保证代码在常规 Unix 环境下易于调试，但所有设计、依赖和实现必须优先服从 Android NDK 与 `adb shell` 环境。

## 1.3 Android 最低版本

建议 Android 最低 API Level 为 API 26 或更高，并在文档及交叉编译脚本中明确说明。

---

# 二、技术栈要求

## 2.1 Rust

必须使用：

```text
Rust stable
edition = 2021
```

代码必须兼容 stable Rust，不得依赖 nightly 特性。

## 2.2 TUI

必须使用：

```text
ratatui
crossterm
```

禁止使用：

```text
termion
```

## 2.3 异步运行时

使用：

```text
tokio
```

根据实际使用启用必要 feature，避免无意义启用 `full`。若为保证实现完整性确有必要，可以说明原因。

## 2.4 HTTP 客户端

使用：

```text
reqwest
```

必须：

- 使用 rustls
- 禁止 native-tls
- 关闭 reqwest 默认 TLS feature
- 只启用必要的 JSON、stream、rustls 等功能

## 2.5 序列化

使用：

```text
serde
serde_json
```

## 2.6 命令行参数

使用：

```text
clap
```

建议使用 derive API。

## 2.7 错误处理

使用：

```text
anyhow
thiserror（仅在需要定义稳定错误类型时可选）
```

业务逻辑对外统一返回：

```rust
anyhow::Result<T>
```

禁止在正常业务代码中使用：

```rust
unwrap()
expect()
panic!()
todo!()
unimplemented!()
```

测试代码中可在非常明确的前置条件下有限使用 `expect`，但应尽量避免。

## 2.8 配置格式

使用：

```text
toml
```

配置文件名称：

```text
config.toml
```

默认放置位置：

```text
nl2sh 可执行文件所在目录
```

要求正确处理：

- 通过相对路径启动
- 通过绝对路径启动
- 符号链接
- 无法获取当前可执行文件路径
- 配置文件不存在
- 配置格式错误
- 配置字段缺失

---

# 三、项目目录与模块架构

必须采用清晰、低耦合、可扩展的模块化设计。

建议目录结构如下，可根据实现需要增加合理文件，但不得将大量逻辑堆积到 `main.rs`：

```text
nl2sh/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── ARCHITECTURE.md
├── AGENTS.md
├── PROJECT_PLAN.md
├── PROJECT_STATUS.md
├── CHANGELOG.md
├── config.toml.example
├── cross-compile.sh
├── .gitignore
├── src/
│   ├── main.rs
│   ├── cli.rs
│   ├── tui/
│   │   ├── mod.rs
│   │   ├── app.rs
│   │   ├── ui.rs
│   │   ├── events.rs
│   │   ├── terminal.rs
│   │   └── input.rs
│   ├── llm/
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   ├── chat_completions.rs
│   │   ├── responses.rs
│   │   ├── retry.rs
│   │   └── types.rs
│   ├── agent/
│   │   ├── mod.rs
│   │   ├── runner.rs
│   │   ├── context.rs
│   │   ├── policy.rs
│   │   └── tools.rs
│   ├── shell/
│   │   ├── mod.rs
│   │   ├── executor.rs
│   │   ├── pty.rs
│   │   ├── pipeline.rs
│   │   ├── root.rs
│   │   ├── interactive.rs
│   │   └── process.rs
│   ├── security/
│   │   ├── mod.rs
│   │   ├── detector.rs
│   │   ├── classifier.rs
│   │   ├── rules.rs
│   │   └── types.rs
│   └── config/
│       ├── mod.rs
│       ├── loader.rs
│       ├── model.rs
│       └── wizard.rs
└── tests/
    ├── config_tests.rs
    ├── security_tests.rs
    ├── root_tests.rs
    ├── llm_mock_tests.rs
    └── agent_loop_tests.rs
```

要求：

- 每个模块必须有明确的 `pub` API
- 模块边界清晰
- 避免循环依赖
- 公共类型有文档注释
- 关键状态机有注释
- 业务代码与具体 OpenAI JSON 协议解耦
- Shell 执行层与 TUI 层解耦
- 安全检测层不得依赖具体 UI
- Agent 不得绕过安全和确认流程

---

# 四、TUI 设计

界面风格类似 Codex CLI，适合 Android `adb shell -t`。

## 4.1 页面布局

使用三区域布局：

```text
+------------------------------------------------------+
| nl2sh v0.x | Agent | root: yes/no | endpoint/model  |
+------------------------------------------------------+
|                                                      |
| > 用户输入                                           |
|                                                      |
| 🤖 Agent                                             |
|                                                      |
| 💻 command                                           |
|                                                      |
| ✅ stdout                                            |
| ❌ stderr / error                                    |
|                                                      |
+------------------------------------------------------+
| Enter send | Y run | N cancel | E edit | Ctrl+C stop |
| Ctrl+Q quit                                          |
+------------------------------------------------------+
```

## 4.2 顶部标题栏

显示：

- 程序名称
- 程序版本
- 当前运行模式
  - Agent
  - Command
- root 状态
  - root
  - su available
  - normal shell
- 当前模型
- 当前 API 类型
  - Chat Completions
  - Responses

## 4.3 中间主区域

显示完整对话和执行历史：

- 用户输入
- Agent 状态
- Tool Call
- LLM 生成命令
- 风险判断结果
- 用户确认状态
- Shell 执行输出
- 超时信息
- 中断信息
- 错误信息
- Agent 最终回答

前缀标识：

```text
>  用户输入
🤖 Agent 状态或回答
🔧 Tool Call
💻 Shell 命令
⚠️ 风险警告
✅ 成功输出
❌ 错误输出
⏱ 超时
⛔ 用户拒绝或中断
```

需要考虑终端不支持 Emoji 的情况：

- 可以保留 Emoji 默认显示
- 同时提供 ASCII 降级配置或内部 fallback
- 不得因为字符宽度计算错误导致 TUI 崩溃

## 4.4 底部输入和状态栏

显示：

- 输入框
- 当前状态
- 快捷键
- 是否正在请求 LLM
- 是否正在执行命令
- 当前 Agent 轮数
- 剩余上下文轮数

快捷键：

```text
Enter   提交输入或确认编辑
Y       执行
N       取消
E       编辑命令
Ctrl+C  中断当前 LLM 请求或命令
Ctrl+Q  安全退出程序
Esc     关闭弹窗或取消当前编辑
```

## 4.5 终端状态恢复

无论以下任何情况发生，都必须正确恢复终端：

- 正常退出
- Ctrl+Q
- Ctrl+C
- LLM 请求错误
- Shell 执行错误
- PTY 初始化失败
- 子进程异常结束
- panic hook 被触发
- 全屏交互命令退出

必须恢复：

- raw mode
- alternate screen
- 光标可见性
- 鼠标捕获状态
- 终端尺寸状态

不得将 `enable_mouse_capture` 作为必需条件，Android `adb shell` 下默认可以关闭鼠标事件。

---

# 五、交互流程

## 5.1 TUI 模式

启动：

```bash
nl2sh
```

流程：

```text
用户输入自然语言
    ↓
加入对话历史
    ↓
发送给 Agent
    ↓
LLM 返回文本或 Tool Call
    ↓
提取 shell 命令
    ↓
命令分类
    ↓
安全规则检测
    ↓
根据确认策略决定是否提示
    ↓
执行或取消
    ↓
PTY 实时输出
    ↓
结果返回 Agent
    ↓
Agent 判断是否继续调用工具
    ↓
完成并显示最终结果
```

## 5.2 非交互模式

示例：

```bash
nl2sh "查看 /data 目录下最大的十个文件"
```

行为：

- 调用 LLM
- 展示生成命令
- 按相同安全策略判断
- 需要确认时读取终端输入
- 用户确认后执行
- 实时输出结果
- 默认不绕过确认流程
- 无 TTY 时不得静默执行修改或高风险命令

如果 stdin/stdout 不是 TTY：

- 查询类只读命令可以按配置执行
- 修改类、高风险命令必须拒绝自动执行，除非用户显式使用专门的危险覆盖参数
- 默认不提供危险覆盖参数，或必须使用非常明确的参数名并显示警告

## 5.3 命令编辑

用户选择 `E` 时：

- 将生成命令复制到可编辑输入框
- 用户修改后重新执行安全分类和风险检查
- 不允许编辑后绕过检测
- 修改后的命令必须重新确认

---

# 六、Agent 设计

## 6.1 默认模式

必须支持两种模式：

```text
1. Tool Calling Agent Mode
2. Command Generation Mode
```

默认：

```text
Tool Calling Agent Mode
```

Command Generation Mode 用于兼容：

- 不支持 Tool Calling 的模型
- 简单 OpenAI 兼容服务
- 部分本地模型

## 6.2 Agent Loop

支持多轮 Tool Loop。

每轮包括：

```text
LLM Request
    ↓
LLM Response
    ↓
Tool Call / Final Text
    ↓
Tool Policy
    ↓
Security Evaluation
    ↓
Confirmation
    ↓
Execution
    ↓
Tool Result
    ↓
Append Context
    ↓
Next LLM Request
```

配置中必须提供最大 Agent 轮数：

```toml
max_agent_steps = 8
```

达到上限时：

- 停止继续执行
- 显示原因
- 保留完整历史
- 不得无限循环

## 6.3 内置 Tool

至少实现：

```json
{
  "name": "execute_shell_command",
  "description": "Execute a shell command in the Android shell environment after security evaluation and required user confirmation."
}
```

参数：

```json
{
  "command": "ls -la",
  "reason": "List files in the current directory.",
  "interactive": false,
  "requires_root": false
}
```

其中：

- `command` 必填
- `reason` 可选但建议要求模型提供
- `interactive` 可选
- `requires_root` 仅作为模型建议，实际仍由程序判断

LLM 无权直接决定：

- 是否跳过确认
- 是否属于安全命令
- 是否使用 root
- 是否延长超时
- 是否绕过执行限制

## 6.4 Tool 调用策略

### 查询类操作

允许 Agent 自动连续调用，无需用户逐次确认。

典型只读操作：

```text
ls
pwd
find
du
df
ps
cat
grep
sed -n
head
tail
stat
file
getprop
id
whoami
mount（仅查看）
pm list
dumpsys（只读调用）
```

注意：

- 不得只通过命令首词判断
- 必须检测重定向、管道、副作用参数、命令替换和复合命令
- `cat file > other` 不是只读
- `find ... -delete` 不是只读
- `sed -i` 不是只读
- `mount -o remount,rw` 不是只读

### 修改类操作

必须经过用户确认，即使默认安全等级为 balanced。

典型修改操作：

```text
rm
mv
cp
chmod
chown
mkdir
rmdir
touch
ln
truncate
tee
echo >
printf >
sed -i
find -delete
package install
pm install
pm uninstall
settings put
setprop
mount -o remount
文件写入
系统配置修改
进程终止
```

### 高风险操作

必须二次确认，并显示明确风险说明。

典型高风险操作：

```text
rm -rf
mkfs
dd
reboot
shutdown
halt
poweroff
wipe
fastboot
分区修改
块设备写入
递归修改根目录权限
fork bomb
```

## 6.5 Agent 最终回答

完成 Tool Loop 后，Agent 可以输出简短总结，但不得伪造执行结果。

最终回答必须基于真实 Tool Result。

如果 Tool 被拒绝或失败，Agent 应明确说明：

- 未执行
- 执行失败
- 超时
- 用户取消
- 权限不足

---

# 七、LLM 支持

## 7.1 API 类型

必须同时支持：

### OpenAI Chat Completions API

```text
/v1/chat/completions
```

### OpenAI Responses API

```text
/v1/responses
```

## 7.2 OpenAI 兼容 endpoint

支持配置自定义 endpoint，以兼容：

- OpenAI
- Ollama 的 OpenAI 兼容接口
- vLLM
- LM Studio
- 其他实现 Chat Completions 或 Responses 协议的服务

不要假设所有兼容服务都支持 Responses API。

## 7.3 API 类型配置

配置示例：

```toml
api_type = "responses"
```

支持：

```text
chat_completions
responses
```

可以增加：

```text
auto
```

若实现 `auto`，必须采用明确、可预测的探测或回退策略，不得因为一次业务错误就错误切换 API。

## 7.4 LLM 抽象层

业务代码禁止直接依赖 OpenAI 请求/响应格式。

必须定义抽象接口，例如：

```rust
#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(
        &self,
        request: LlmRequest,
    ) -> anyhow::Result<LlmResponse>;
}
```

统一业务类型：

```text
LlmRequest
LlmResponse
ConversationMessage
ToolDefinition
ToolCall
ToolResult
Usage
FinishReason
```

具体 API 实现：

```text
ChatCompletionsClient
ResponsesClient
```

## 7.5 Tool Calling

支持：

- Chat Completions `tools` / `tool_calls`
- Responses API function tools / function call outputs

两种 API 最终都转换为统一的内部 `ToolCall` 类型。

## 7.6 Command Generation Mode System Prompt

命令生成模式下，System Prompt 必须要求：

```text
你是 Android shell 命令生成器。

根据用户自然语言生成一条适用于 Android adb shell 的可执行 shell 命令。

只输出命令本身。
不要解释。
不要输出 Markdown 代码块。
不要输出前缀。
不要输出多条候选命令。
不要假设 Termux 可用。
优先使用 Android toybox 或系统常见命令。
如果无法安全或可靠生成命令，返回固定文本：
NL2SH_UNABLE_TO_GENERATE
```

程序必须清理可能出现的：

- Markdown code fence
- 多余空白
- `Command:` 前缀

但不得过度修复或拼接模型输出。

## 7.7 上下文管理

配置：

```toml
max_context_turns = 10
```

要求：

- 控制用户/Assistant 对话历史
- Tool Call 和 Tool Result 成对保留
- 不得截断出非法工具调用上下文
- 超出限制时从最旧的完整交互单元开始删除
- System Prompt 始终保留

## 7.8 重试

配置：

```toml
llm_retry_count = 3
llm_retry_base_delay_ms = 500
```

要求：

- 对网络错误、超时、429、部分 5xx 自动重试
- 使用有上限的指数退避
- 不重试明显的配置错误和认证错误
- 重试结束后向用户显示错误
- Ctrl+C 可以取消等待中的重试
- 不得阻塞 TUI 主循环

---

# 八、配置管理

## 8.1 默认配置

生成 `config.toml.example`：

```toml
api_key = "sk-xxx"
model = "gpt-4o-mini"
endpoint = "https://api.openai.com/v1"
api_type = "responses"

max_context_turns = 10
max_agent_steps = 8

llm_retry_count = 3
llm_retry_base_delay_ms = 500
llm_request_timeout_secs = 60

execute_timeout_secs = 30
interactive_execute_timeout_secs = 0
execute_confirm_policy = "risk_only"
security_level = "balanced"
execute_user_mode = "auto"

enable_pty = true
ascii_symbols = false
```

## 8.2 API Key

要求：

- 只从配置文件读取
- 禁止硬编码真实 API Key
- 示例 key 只能出现在 example 文件和 README 示例
- 日志和错误信息不得输出完整 API Key
- Debug 输出不得泄露 Authorization header

可以额外支持环境变量覆盖：

```text
NL2SH_API_KEY
```

优先级必须清晰记录：

```text
CLI 参数 > 环境变量 > config.toml > 默认值
```

## 8.3 配置向导

如果 `config.toml` 不存在：

- TUI 模式启动时提示创建
- 支持交互式向导
- 支持 `nl2sh --init`
- API Key 输入时尽量不回显
- 写入前显示目标路径
- 不得覆盖已有配置，除非用户明确确认

向导至少询问：

- API Key
- endpoint
- model
- API 类型
- 确认策略
- root 执行模式

## 8.4 配置校验

启动时校验：

- endpoint URL
- model 非空
- timeout 大于 0
- max_context_turns 合法
- max_agent_steps 合法
- 枚举值合法
- API Key 是否必需

对于不需要 API Key 的本地服务，允许：

```toml
api_key = ""
```

此时不要发送空的 Bearer header，除非服务明确要求。

---

# 九、Shell 执行系统

## 9.1 PTY 为主要执行路径

默认使用 PTY。

不得只实现：

```text
tokio::process::Command + stdout/stderr pipe
```

但为无 TTY 环境和测试，可以同时提供 pipeline fallback。

## 9.2 普通命令

普通命令在 TUI 中执行：

- 创建 PTY
- 启动 shell
- 实时读取输出
- 将输出发送到 TUI
- 支持终端 resize
- 支持超时
- 支持 Ctrl+C
- 记录退出码

## 9.3 PTY 实现要求

Android 优先使用：

- `nix`
- `libc`
- Android Bionic libc 可用接口

不要强依赖 `portable-pty`。

必须封装为独立模块，避免 PTY 实现污染 Agent 和 TUI 代码。

实现时注意：

- PTY master/slave
- `fork` / `forkpty` / `openpty`
- `setsid`
- controlling terminal
- `dup2`
- signal
- waitpid
- 文件描述符关闭
- 非阻塞读取
- 子进程退出状态
- zombie 进程清理

如果采用 `forkpty`，必须解释 Android 兼容性和 crate feature。

## 9.4 stdout 和 stderr

PTY 下 stdout/stderr 通常混合。

设计中必须接受这一事实，并在架构文档中说明。

pipeline fallback 可以分别处理 stdout/stderr。

## 9.5 ANSI 控制序列

普通命令在 TUI 内显示时：

- 保留普通颜色可以作为可选能力
- 必须过滤会破坏 TUI 的光标移动、清屏、alternate screen 等控制序列
- 不得将任意 PTY 字节直接写回 TUI 终端
- 无法完整实现 ANSI 终端仿真时，应提供保守过滤器

## 9.6 全屏或强交互命令

例如：

```text
vim
vi
top
less
more
ssh
passwd
su
sh
bash
logcat（持续模式）
```

处理方案：

```text
暂停 ratatui
    ↓
退出 alternate screen
    ↓
关闭 raw mode 或切换到合适模式
    ↓
将本地终端与子进程 PTY 双向桥接
    ↓
等待子进程结束
    ↓
恢复 raw mode
    ↓
重新进入 alternate screen
    ↓
重绘 TUI
```

应支持：

- 终端尺寸同步
- 输入双向传输
- Ctrl+C 转发
- 子进程退出后恢复 TUI
- 异常时仍恢复终端

## 9.7 交互命令检测

使用：

- 已知命令列表
- 命令结构分析
- LLM Tool 参数中的 `interactive` 提示

但 LLM 提示不能作为唯一依据。

允许用户在确认弹窗中选择：

```text
在 TUI 内执行
切换到交互终端执行
取消
编辑
```

## 9.8 执行超时

默认：

```toml
execute_timeout_secs = 30
```

超时后：

- 先发送 SIGTERM
- 等待短暂 grace period
- 再发送 SIGKILL
- 清理子进程和 PTY
- 显示超时
- 将超时结果返回 Agent

交互式命令可使用：

```toml
interactive_execute_timeout_secs = 0
```

`0` 表示不自动超时。

## 9.9 Ctrl+C

Ctrl+C 行为：

- 空闲状态：清空当前输入或提示再次按下退出，不应直接破坏终端
- LLM 请求中：取消请求
- Agent Loop 中：停止后续 Tool Call
- 命令执行中：向子进程组发送 SIGINT
- 再次 Ctrl+C：可升级为 SIGTERM
- Ctrl+Q：退出整个程序

---

# 十、Android root 与 su 支持

## 10.1 不支持 Termux

不得使用 Termux 特有路径、包管理器或环境假设。

## 10.2 root 检测

启动时检测：

```text
geteuid()
id -u
su 可用性
```

优先使用系统调用获取 UID，命令仅作为辅助。

状态分类：

```text
Root
SuAvailable
Normal
```

## 10.3 执行用户模式

配置：

```toml
execute_user_mode = "auto"
```

支持：

```text
auto
normal
root
```

### auto

- 当前 UID 为 0：直接通过 `sh -c` 执行
- 当前 UID 非 0：默认当前用户执行
- 当命令明确需要 root 时，检测并使用 `su -c`
- 不应因为检测到 su 就把所有命令自动提升为 root

### normal

始终使用当前用户：

```text
sh -c
```

不得自动调用 su。

### root

- 当前 UID 为 0：直接执行
- 非 root：尝试 `su -c`
- su 不可用或授权失败：显示错误，不得静默降级执行需要 root 的命令

## 10.4 su -c

必须支持：

```bash
su -c '<command>'
```

需要正确处理：

- shell quoting
- 单引号
- 双引号
- 管道
- 重定向
- 多命令
- 换行
- 命令替换

不要通过简单字符串拼接构造不安全的 `su -c` 参数。

可以通过：

- 参数化调用
- 安全 shell quoting 函数
- 将命令通过 stdin 传递给 root shell

选择一种可在 Android 常见 su 实现上工作的方案，并在架构文档中说明取舍。

## 10.5 root 确认

使用 root 执行的修改操作：

- 至少需要普通修改确认
- 高风险命令必须二次确认
- 确认弹窗必须明确显示 `ROOT`

例如：

```text
⚠️ 将以 ROOT 权限执行：
rm /data/system/example

继续？ [y/N]
```

---

# 十一、安全系统

## 11.1 基本原则

任何命令都必须遵循：

```text
LLM Output
    ↓
Parse
    ↓
Classify
    ↓
Security Detect
    ↓
Confirm Policy
    ↓
Execute
```

禁止：

- LLM 直接调用执行器
- Tool Call 绕过安全层
- 用户编辑后跳过重新检测
- root 模式绕过安全层
- Agent 连续执行修改操作而不确认

## 11.2 安全等级

配置：

```toml
security_level = "balanced"
```

支持：

```text
strict
balanced
unsafe
```

### strict

- 所有命令执行前确认
- 修改命令明确警告
- 高风险命令二次确认

### balanced

默认：

- 查询类命令自动执行
- 修改类命令确认
- 高风险命令二次确认

### unsafe

- 查询类和普通修改命令可以自动执行
- 高风险命令仍建议保留至少一次强确认
- 若实现完全无确认，必须要求额外 CLI 显式开关，不得仅通过配置文件静默启用

## 11.3 确认策略

配置：

```toml
execute_confirm_policy = "risk_only"
```

支持：

```text
always
risk_only
never
```

但 Tool Policy 优先级高于通用确认策略：

- 查询类是否确认由策略决定
- 修改类默认必须确认
- 高风险命令必须二次确认
- `never` 不得默认绕过高风险二次确认，除非同时启用明确危险模式

## 11.4 风险等级

定义：

```rust
pub enum RiskLevel {
    ReadOnly,
    Mutating,
    Dangerous,
    Critical,
}
```

安全检查结果至少包含：

```rust
pub struct SecurityAssessment {
    pub risk_level: RiskLevel,
    pub matched_rules: Vec<MatchedRule>,
    pub requires_confirmation: bool,
    pub requires_double_confirmation: bool,
    pub requires_root: bool,
    pub explanation: String,
}
```

## 11.5 必须检测的危险模式

至少包含：

```text
rm -rf /
rm -rf /*
rm -rf /system
rm -rf /data
mkfs
dd if=
dd of=/dev/
chmod -R 777 /
chmod -R 777 /*
:(){ :|:& };:
> /dev/sd*
> /dev/block/*
shutdown
reboot
halt
poweroff
wipe
fastboot erase
mount -o remount,rw
```

必须考虑：

- 多余空格
- 引号
- 绝对路径
- 命令链
- `&&`
- `||`
- `;`
- 管道
- 重定向
- `$()`
- 反引号
- `sh -c`
- `su -c`
- 转义字符
- 大小写差异（适用时）

不要只使用简单 `contains()`。

可以采用：

- 规范化
- Token 化
- 正则规则
- 命令段拆分
- 启发式检测

无需实现完整 POSIX Shell Parser，但必须比纯字符串包含更可靠。

## 11.6 可配置安全规则

支持在配置文件中增加自定义规则：

```toml
[[security_rules]]
id = "delete-root"
pattern = "rm\\s+-[^\\n]*r[^\\n]*f[^\\n]*\\s+/"
risk = "critical"
message = "This command may recursively delete the root filesystem."
```

内置规则不得因为用户规则为空而失效。

---

# 十二、命令行参数

必须支持：

```bash
nl2sh
```

启动 TUI Agent 模式。

```bash
nl2sh "自然语言指令"
```

非交互单次请求模式，展示命令并按策略确认后执行。

```bash
nl2sh --init
```

初始化配置。

```bash
nl2sh --version
```

显示版本。

```bash
nl2sh --help
```

显示帮助。

建议额外支持：

```bash
nl2sh --mode agent
nl2sh --mode command
nl2sh --config /path/to/config.toml
nl2sh --api-type responses
nl2sh --no-pty
nl2sh --ascii
nl2sh --dry-run
```

`--dry-run` 只生成、分类和展示命令，不执行。

---

# 十三、编译配置

必须保证：

```bash
cargo build
cargo build --release
cargo test
```

能够通过。

`Cargo.toml` release profile：

```toml
[profile.release]
lto = true
codegen-units = 1
strip = true
panic = "abort"
opt-level = "z"
```

若 `panic = "abort"` 会影响终端恢复设计，需要通过 panic hook、RAII 和正常错误流程减少风险，并在文档中说明。

不要宣称生成后已经实际编译通过，除非确实执行了编译验证。

---

# 十四、Android 交叉编译

提供完整：

```text
cross-compile.sh
```

目标：

```text
aarch64-linux-android
```

脚本要求：

- `set -euo pipefail`
- 检测 `ANDROID_NDK_HOME`
- 兼容 `ANDROID_NDK_ROOT`
- 检测 NDK LLVM toolchain
- 支持 Linux/macOS host tag
- 配置 clang linker
- 配置 ar
- 指定 API Level
- 安装 Rust target 的提示
- 执行 release build
- 输出最终二进制路径
- 错误信息清晰

需要说明：

```bash
rustup target add aarch64-linux-android
```

不要要求依赖 Termux。

---

# 十五、测试要求

必须提供可运行的测试代码。

## 15.1 配置测试

覆盖：

- 正常 TOML
- 缺失字段默认值
- 非法枚举
- 非法 timeout
- 配置文件不存在
- API Key 为空的本地 endpoint

## 15.2 安全测试

覆盖至少：

```text
ls -la
cat /proc/cpuinfo
cat file > other
find /data -type f
find /data -type f -delete
rm -rf /
rm -rf /*
mkfs.ext4 /dev/block/x
dd if=/dev/zero of=/dev/block/x
chmod -R 777 /
reboot
su -c 'rm -rf /'
sh -c "reboot"
```

验证风险分类和确认要求。

## 15.3 root 测试

不得依赖测试机真实 root。

通过抽象或 mock 测试：

- UID 0
- UID 非 0
- su 存在
- su 不存在
- su 执行失败
- normal 模式
- auto 模式
- root 模式

## 15.4 LLM Mock 测试

使用 mock server 或 mock trait，覆盖：

- Chat Completions 文本响应
- Chat Completions Tool Call
- Responses API 文本响应
- Responses API Tool Call
- 429 重试
- 401 不重试
- 超时取消
- 非法 JSON
- 空响应

## 15.5 Agent Loop 测试

覆盖：

- 只读 Tool 自动执行
- 修改 Tool 请求确认
- 用户拒绝
- 高风险二次确认
- Tool Result 返回 LLM
- 多轮完成
- 达到 max_agent_steps
- Ctrl+C 取消
- Tool 执行失败
- 超时

## 15.6 PTY 测试

普通 `cargo test` 中避免依赖 Android 真机。

可以：

- 为 PTY 接口提供 mock
- 在 Unix 条件编译下提供基础 smoke test
- 在 README 中提供 Android 真机手工测试步骤

---

# 十六、ARCHITECTURE.md

必须生成：

```text
ARCHITECTURE.md
```

内容至少包括：

## 16.1 系统整体架构

```text
User
 |
TUI / CLI
 |
Agent Runner
 |
LLM Provider
 |
Tool System
 |
Security Engine
 |
Confirmation Policy
 |
PTY / Pipeline Executor
 |
Root / su Layer
 |
Android Runtime
```

详细说明：

- 模块职责
- 数据流
- 控制流
- 异常流
- 取消流程
- 超时流程
- 终端恢复流程

## 16.2 Rust 模块架构

逐一说明：

```text
src/tui
src/llm
src/agent
src/shell
src/security
src/config
```

每个模块包括：

- 职责
- 主要类型
- 输入
- 输出
- 对外接口
- 依赖关系
- 不允许承担的职责

## 16.3 Agent 执行流程

说明：

- 查询操作
- 修改操作
- 高风险操作
- 多轮 Tool Loop
- Tool Result 回传
- 最大轮数限制
- 用户取消

## 16.4 PTY 架构

说明：

- PTY 创建
- 子进程组
- stdin/stdout/stderr
- 信号
- resize
- Ctrl+C
- 超时
- TUI 暂停与恢复
- Android 兼容取舍

## 16.5 Android root 架构

说明：

```text
nl2sh
 |
 +-- uid == 0
 |      |
 |      +-- sh -c
 |
 +-- uid != 0
        |
        +-- normal execution
        |
        +-- su available
               |
               +-- su -c
```

说明：

- auto
- normal
- root
- 权限不足
- su 授权失败
- root 风险提示

## 16.6 LLM Provider 架构

说明：

- 统一 trait
- Chat Completions
- Responses
- Tool Call 类型转换
- retry
- cancellation
- context truncation

## 16.7 安全架构

说明：

```text
Raw Command
   ↓
Normalize
   ↓
Split Compound Commands
   ↓
Classify Side Effects
   ↓
Match Dangerous Rules
   ↓
Build SecurityAssessment
   ↓
Apply Confirmation Policy
```

## 16.8 扩展设计

说明未来如何扩展：

- 新 LLM Provider
- 新 Tool
- 新安全规则
- 新执行环境
- 新配置来源
- 新 UI
- 新 Shell Parser

---

# 十七、AGENTS.md

必须生成：

```text
AGENTS.md
```

用途：

供 Codex、Claude Code、Cursor、Gemini CLI 等 AI 编程工具后续维护项目。

内容必须明确：

## 17.1 项目简介

包括：

- nl2sh 目标
- 技术栈
- Android 优先环境
- 默认 Agent 模式
- 安全边界

## 17.2 AI 开始工作前

任何 AI 在修改代码前必须先阅读：

```text
AGENTS.md
ARCHITECTURE.md
PROJECT_PLAN.md
PROJECT_STATUS.md
```

必要时还要阅读：

```text
CHANGELOG.md
README.md
```

## 17.3 修改前要求

AI 修改前必须明确：

- 修改目标
- 涉及模块
- 是否改变公共接口
- 是否影响 Android
- 是否影响安全模型
- 是否影响 PTY/终端恢复
- 需要增加或更新哪些测试

## 17.4 修改后要求

修改后必须：

- 运行或说明应运行的格式化
- 运行或说明应运行的测试
- 更新 `PROJECT_STATUS.md`
- 必要时更新 `PROJECT_PLAN.md`
- 架构变化时更新 `ARCHITECTURE.md`
- 用户可见变化时更新 `CHANGELOG.md`
- 配置变化时更新 `config.toml.example` 和 README

## 17.5 Rust 代码规范

要求：

- stable Rust
- 禁止业务代码 `unwrap`
- 禁止业务代码 `panic`
- 使用 `anyhow::Context`
- 公共接口写文档注释
- 保持模块边界
- 避免无必要 clone
- 避免阻塞 tokio runtime
- 不把长耗时同步 IO 放在 async 主线程
- 对文件描述符和终端状态使用 RAII

## 17.6 Android 兼容规则

禁止：

- 引入仅 glibc 可用的实现而无 Android 条件处理
- 依赖 systemd
- 依赖 dbus
- 依赖 Termux
- 使用 native-tls
- 假设 `/bin/bash` 存在
- 假设 GNU coreutils 存在

必须：

- 优先兼容 Android toybox
- 使用 `/system/bin/sh` 或可配置 shell
- 保持 aarch64-linux-android 可编译
- 考虑 Bionic libc
- 不使用桌面 Linux 专属路径

## 17.7 安全规则

任何 AI 不得破坏：

```text
LLM
 ↓
Security
 ↓
Confirmation
 ↓
Execution
```

禁止：

- 直接执行 LLM 输出
- 删除确认流程
- 将所有命令归为只读
- root 模式跳过风险判断
- 将 `unsafe` 设为默认
- 在测试中削弱生产安全逻辑

## 17.8 PTY 规则

修改 PTY 时必须关注：

- fd 泄漏
- zombie
- process group
- signal
- timeout
- terminal restore
- resize
- Android NDK
- 全屏程序

## 17.9 Git 提交规范

建议使用：

```text
feat:
fix:
refactor:
docs:
test:
chore:
```

提交应单一职责，不把无关格式化混入功能修改。

---

# 十八、PROJECT_PLAN.md

必须生成：

```text
PROJECT_PLAN.md
```

项目计划至少包含以下阶段：

```text
Phase 0  项目初始化与工程基线
Phase 1  CLI 与配置系统
Phase 2  TUI 基础框架
Phase 3  LLM Provider 抽象
Phase 4  Chat Completions 与 Responses API
Phase 5  Agent 与 Tool Calling
Phase 6  安全分类与确认策略
Phase 7  PTY 执行器
Phase 8  Android root 与 su
Phase 9  交互式命令终端切换
Phase 10 测试、文档和 Android 验证
Phase 11 发布准备
```

每个阶段必须包含：

- 目标
- 工作项
- 涉及文件或模块
- 依赖项
- 输出物
- 验收标准
- 风险
- 状态

使用 Markdown checkbox：

```markdown
- [x] 已完成
- [ ] 未完成
```

由于这次要求生成完整初始实现，计划文件应根据实际生成内容标记完成状态，不得机械地把所有阶段标记完成。

无法通过真实 Android 环境验证的事项应保留为待验证状态。

---

# 十九、PROJECT_STATUS.md

必须生成：

```text
PROJECT_STATUS.md
```

建议格式：

```markdown
# Project Status

Last Updated: YYYY-MM-DD

## Current Phase

Phase X

## Overall Status

- Build status:
- Test status:
- Android cross-compile status:
- Android device validation:
- Known blockers:

## Completed

- ...

## In Progress

- ...

## Pending

- ...

## Known Issues

- ...

## Technical Decisions

- ...

## Verification Performed

- ...

## Next Steps

1. ...
2. ...
```

要求：

- 日期使用生成时的实际日期
- 真实记录已完成内容
- 不得伪造编译、测试或 Android 真机验证结果
- 如果只提供代码但未实际执行编译，必须明确写“未执行”
- 记录当前已知限制
- 记录下一步可执行事项

---

# 二十、CHANGELOG.md

必须生成：

```text
CHANGELOG.md
```

采用 Keep a Changelog 风格的简化格式：

```markdown
# Changelog

## [Unreleased]

### Added

- ...

### Changed

- ...

### Fixed

- ...

## [0.1.0] - YYYY-MM-DD

### Added

- Initial project structure
- TUI
- LLM integration
- Agent Tool Calling
- Security evaluation
- PTY execution
- Android root support
```

不得记录未实现的能力。

---

# 二十一、README.md

必须生成完整 README，至少包含：

- 项目简介
- 功能特性
- 安全说明
- 支持环境
- 不支持 Termux 的说明
- 依赖
- 本地编译
- Android 交叉编译
- NDK 配置
- adb 部署
- 配置文件
- API 类型
- root / su 模式
- TUI 快捷键
- 非交互模式
- Agent 模式
- Command 模式
- 风险等级
- 测试
- Android 真机 smoke test
- 已知限制
- 项目文档索引

README 中不得声称：

- 已在所有 Android 设备验证
- 所有 OpenAI 兼容服务都完全支持
- PTY 对所有全屏应用均无兼容问题

---

# 二十二、输出要求

请严格按照以下顺序输出。

## 22.1 第一部分：实现说明

简要说明：

- 核心架构
- 关键技术选择
- Android PTY 方案
- Agent Tool Policy
- 安全边界
- 已知限制

## 22.2 第二部分：完整目录结构

输出完整目录树。

## 22.3 第三部分：所有文件完整内容

必须输出：

- `Cargo.toml`
- `.gitignore`
- `config.toml.example`
- `cross-compile.sh`
- `README.md`
- `ARCHITECTURE.md`
- `AGENTS.md`
- `PROJECT_PLAN.md`
- `PROJECT_STATUS.md`
- `CHANGELOG.md`
- 所有 `src/` 文件
- 所有 `tests/` 文件

要求：

- 每个文件完整输出
- 每个代码块前标明文件路径
- 不允许省略
- 不允许使用“此处省略”
- 不允许 TODO 占位符
- 不允许伪代码
- 不允许仅给接口不实现
- 不允许引用“上一段相同”
- 不允许用补丁代替完整文件

## 22.4 内容过长时

允许自动分多次输出。

必须遵守：

- 每次只在完整文件边界停止
- 不得在一个文件中间截断
- 每次开头说明本次文件范围
- 每次结尾列出已完成文件和剩余文件
- 后续消息必须自动继续，不要求用户重复粘贴 Prompt
- 不得因为长度原因降低代码完整性
- README 和文档可以后置，但不得省略
- 最后一轮必须明确说明全部文件已输出完成

## 22.5 编译和测试验证

生成所有文件后，必须提供：

```bash
cargo fmt --all -- --check
cargo check
cargo test
cargo build --release
```

以及 Android 交叉编译验证命令：

```bash
./cross-compile.sh
```

如果你拥有可执行环境，应实际运行这些命令并根据错误持续修复，直到通过或遇到无法解决的环境限制。

如果无法实际执行：

- 必须明确说明“未实际执行”
- 不得声称“已编译通过”
- 必须进行静态一致性检查
- 检查 Cargo 依赖与 feature
- 检查模块路径
- 检查公开接口
- 检查 async trait
- 检查条件编译
- 检查测试依赖

## 22.6 生成过程的自检清单

在输出结束前逐项自检：

- [ ] `Cargo.toml` 与源码依赖一致
- [ ] 所有模块都在 `mod.rs` 或父模块声明
- [ ] 没有缺失的 `use`
- [ ] 没有未实现函数
- [ ] 没有 TODO
- [ ] 没有硬编码真实 API Key
- [ ] Chat Completions 与 Responses API 均有实现
- [ ] Agent 默认模式为 Tool Calling
- [ ] Command Mode 可用
- [ ] 只读 Tool 可自动连续执行
- [ ] 修改 Tool 必须确认
- [ ] 高风险 Tool 必须二次确认
- [ ] root / su 不绕过安全检查
- [ ] PTY 和 pipeline fallback 均有清晰边界
- [ ] 全屏交互命令会暂停并恢复 TUI
- [ ] Ctrl+C 可取消 LLM 和命令
- [ ] 超时会清理子进程
- [ ] 配置向导可用
- [ ] Android 不依赖 Termux
- [ ] README、ARCHITECTURE、AGENTS、PLAN、STATUS、CHANGELOG 均完整
- [ ] 测试文件完整
- [ ] PROJECT_STATUS 未伪造验证结果

---

# 二十三、代码质量要求

生成项目必须满足：

- 可维护
- 模块化
- Android 优先
- 单二进制
- Rust stable
- 无 TODO
- 无伪代码
- 无省略
- 无硬编码真实 API Key
- 关键公共接口有文档注释
- 不使用 native-tls
- 不依赖 Termux
- 不依赖 systemd
- 不假设 bash 或 GNU coreutils
- 不直接执行未经安全检查的 LLM 输出
- 不允许 Agent 绕过用户确认
- 所有终端状态通过 RAII 或等价机制恢复
- 所有子进程能够被回收
- 所有异步任务能够取消或结束
- 所有配置枚举有默认值和反序列化校验
- 所有用户可见错误具有上下文

---

# 二十四、实现优先级

当“功能丰富”与“可编译、可维护、Android 兼容”冲突时，优先级必须是：

```text
1. 可编译
2. 安全边界正确
3. Android 兼容
4. 终端状态可恢复
5. 核心功能可用
6. 测试可执行
7. 架构可扩展
8. UI 美观
```

不得为了展示更多功能而输出无法编译的复杂实现。

---

# 二十五、最终执行指令

现在开始生成完整项目。

先输出：

1. 实现说明
2. 完整目录结构
3. `Cargo.toml`
4. 其余文件

当输出过长时，自动分批继续，并严格保持文件完整。

不要询问是否继续。

不要省略任何文件。

不要输出 TODO。

不要声称执行过未实际执行的编译或测试。
