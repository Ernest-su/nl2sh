# nl2sh Termux 安装说明

本项目提供两个由 Android NDK 构建的 Termux `.deb`：

```text
dist/
├── nl2sh_版本_aarch64.deb
└── nl2sh_版本_arm.deb
```

Linux 构建使用 `./pack-termux-release.sh`。Windows 可在 PowerShell 中使用 Windows 版 Android NDK 编译，并让 WSL 只负责 `.deb` 封包：

```powershell
$env:ANDROID_NDK_HOME = "C:\Android\Sdk\ndk\28.2.13676358"
.\pack-termux-release.ps1
```

Windows 需要安装 Rust、两个 Android Rust target、Windows 版 NDK 和 WSL；默认 WSL 发行版中需要存在 `bash` 与 `dpkg-deb`。

## 1. 确认设备架构

在 Termux 中运行：

```bash
dpkg --print-architecture
```

- 输出 `aarch64`：安装文件名以 `_aarch64.deb` 结尾的包。
- 输出 `arm`：安装文件名以 `_arm.deb` 结尾的包。
- 当前发布包不支持 `x86_64` 和 `i686`。

## 2. 本地安装

64 位 ARM 设备：

```bash
apt install ./nl2sh_*_aarch64.deb
```

32 位 ARM 设备：

```bash
apt install ./nl2sh_*_arm.deb
```

当前目录中每种架构只应有一个安装包；也可以输入 `apt install ./nl2sh_` 后按 Tab 补全。

安装完成后验证：

```bash
command -v nl2sh
nl2sh --version
nl2sh
```

首次进入 TUI 后使用 `/config` 配置模型服务。APT 版本默认把配置保存在 `~/.config/nl2sh/config.toml`，把日志和会话保存在 `~/.local/state/nl2sh/`；对应的 `XDG_CONFIG_HOME`、`XDG_STATE_HOME` 环境变量可以改变这些基础目录。

## 3. 更新和卸载

通过本地 `.deb` 更新时，下载新发布包并再次执行：

```bash
apt install ./新的nl2sh包.deb
```

通过 nl2sh APT 软件源安装的用户使用：

```bash
pkg update
pkg upgrade nl2sh
```

APT 构建不会自行替换 `$PREFIX/bin/nl2sh`；`nl2sh update` 和 TUI `/update` 会提示使用包管理器更新。

卸载程序：

```bash
apt remove nl2sh
```

卸载不会自动删除用户配置、日志或会话。如需清理，请先确认内容不再需要，再自行处理 `~/.config/nl2sh` 和 `~/.local/state/nl2sh`。

## 4. 常见问题

### 提示架构不匹配

重新运行 `dpkg --print-architecture`，确认 `.deb` 文件名中的架构完全一致。

### 提示 `No such file or directory`

确认安装的是本发布包中的 Android/Termux `.deb`，而不是桌面 Linux 编译产物；同时确认设备架构匹配。

### 无法联网访问模型

使用 `/config` 检查 Provider、Endpoint、API Key 和代理配置。API Key 不应写入命令历史或提交到代码仓库。
