# Piri

[English](README.en.md) | **中文**

---

欢迎来到 Piri，您扩展 niri 合成器功能的门户。Piri 提供了可扩展的命令系统，旨在简化和高效，允许你提升你的生产力并定制你的用户体验。

您可以将其视为类似工具但针对 niri 用户（涉及编辑文本文件）。通过基于命令的架构，Piri 被设计为轻量级且易于使用。

请注意，使用 Rust 和守护进程架构鼓励使用多个功能而对内存占用和性能影响不大。

欢迎贡献、建议、bug 报告和评论。

> **注意**: 本项目完全由 [Cursor](https://cursor.sh/) AI 代码编辑器配合完成开发。

## 插件

- 📦 **Scratchpads**: 强大的窗口管理功能，支持快速访问常用应用程序（详见 [Scratchpads 文档](docs/zh/plugins/scratchpads.md)）
- 🔌 **Empty**: 在切换到空 workspace 时自动执行命令，用于自动化工作流程（详见 [Empty 文档](docs/zh/plugins/empty.md)）

## 快速开始

### 安装

#### 使用安装脚本（推荐）

最简单的方式是使用提供的安装脚本：

```bash
# 运行安装脚本
./install.sh
```

安装脚本会自动：
- 构建 release 版本
- 安装到 `~/.local/bin/piri`（普通用户）或 `/usr/local/bin/piri`（root）
- 复制配置文件到 `~/.config/niri/piri.toml`

如果 `~/.local/bin` 不在 PATH 中，脚本会提示你添加到 PATH。

#### 使用 Cargo 安装

```bash
# 安装到用户目录（推荐，不需要 root 权限）
cargo install --path .

# 或者安装到系统目录（需要 root 权限）
sudo cargo install --path . --root /usr/local
```

安装完成后，如果安装到用户目录，确保 `~/.cargo/bin` 在你的 `PATH` 环境变量中：

```bash
export PATH="$PATH:$HOME/.cargo/bin"
```

可以将此命令添加到你的 shell 配置文件中（如 `~/.bashrc` 或 `~/.zshrc`）。

### 配置

将示例配置文件复制到配置目录：

```bash
mkdir -p ~/.config/niri
cp config.example.toml ~/.config/niri/piri.toml
```

然后编辑 `~/.config/niri/piri.toml` 来配置你的功能。

## 使用方法

### 启动守护进程

```bash
# 启动守护进程（前台运行）
piri daemon
```

```bash
# 更多调试日志
piri --debug daemon
```

### 重新加载配置

```bash
# 重新加载配置文件（无需重启守护进程）
piri reload
```

### Shell 自动补全

生成 shell 自动补全脚本：

```bash
# Bash
piri completion bash > ~/.bash_completion.d/piri

# Zsh
piri completion zsh > ~/.zsh_completion.d/_piri

# Fish
piri completion fish > ~/.config/fish/completions/piri.fish
```

## 插件

### Scratchpads

![](./assets/scratchpads.mp4)

快速显示和隐藏常用应用程序的窗口。支持跨 workspace 和 monitor。

**配置示例**：
```toml
[piri.plugins]
scratchpads = true

[scratchpads.term]
direction = "fromRight"
command = "GTK_IM_MODULE=wayland ghostty --class=float.dropterm"
app_id = "float.dropterm"
size = "40% 60%"
margin = 50
```

**快速使用**：
```bash
# 切换 scratchpad 显示/隐藏
piri scratchpads {name} toggle

# 动态添加当前窗口为 scratchpad
piri scratchpads {name} add {direction}
```

详细说明请参考 [Scratchpads 文档](docs/zh/plugins/scratchpads.md)。

### Empty

在切换到空 workspace 时自动执行命令，用于自动化工作流程。

> **参考**: 此功能类似于 [Hyprland 的 `on-created-empty` workspace rule](https://wiki.hypr.land/Configuring/Workspace-Rules/#rules)。

**配置示例**：
```toml
[piri.plugins]
empty = true

# 当切换到 workspace 1 且为空时，执行命令
[empty.1]
command = "alacritty"

# 使用 workspace 名称
[empty.main]
command = "firefox"
```

**Workspace 标识符**：支持使用 workspace 名称（如 `"main"`）或索引（如 `"1"`）来匹配。

详细说明请参考 [插件系统文档](docs/zh/plugins/empty.md)。

## 文档

- [架构设计](docs/architecture.md) - 项目架构和工作原理
- [Scratchpads](docs/scratchpads.md) - Scratchpads 功能详细说明
- [插件系统](docs/plugins.md) - 插件系统详细说明
- [开发指南](docs/development.md) - 开发、扩展和贡献指南

## 许可证

MIT License

## 参考项目

本项目受到 [Pyprland](https://github.com/hyprland-community/pyprland) 的启发。Pyprland 是一个为 Hyprland 合成器提供扩展功能的优秀项目，提供了大量插件来增强用户体验。
