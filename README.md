# Piri

[English](README.en.md) | **中文**

---

欢迎来到 Piri，您扩展 niri 合成器功能的门户。Piri 提供了可扩展的命令系统，旨在简化和高效，允许你提升你的生产力并定制你的用户体验。

您可以将其视为类似工具但针对 niri 用户（涉及编辑文本文件）。通过基于命令的架构，Piri 被设计为轻量级且易于使用。

请注意，使用 Rust 和守护进程架构鼓励使用多个功能而对内存占用和性能影响不大。

欢迎贡献、建议、bug 报告和评论。

> **注意**: 本项目完全由 [Cursor](https://cursor.sh/) AI 代码编辑器配合完成开发。

## 插件

- 📦 **Scratchpads**: 强大的窗口管理功能，支持快速显示和隐藏常用应用程序的窗口，跨 workspace 和 monitor（详见 [Scratchpads 文档](docs/zh/plugins/scratchpads.md)）
- 🔌 **Empty**: 在切换到空 workspace 时自动执行命令，用于自动化工作流程（详见 [Empty 文档](docs/zh/plugins/empty.md)）
- 🎯 **Window Rule**: 根据窗口的 `app_id` 或 `title` 使用正则表达式匹配，自动将窗口移动到指定 workspace，并支持在窗口获得焦点时执行命令（详见 [Window Rule 文档](docs/zh/plugins/window_rule.md)）
- 🔄 **Autofill**: 在窗口关闭或布局改变时自动将最后一列窗口对齐到最右侧位置（详见 [Autofill 文档](docs/zh/plugins/autofill.md)）
- 🔒 **Singleton**: 管理单例窗口，切换时如果窗口已存在则聚焦，否则启动应用程序（详见 [Singleton 文档](docs/zh/plugins/singleton.md)）
- 📋 **Window Order**: 根据配置的权重值自动重排工作区中的窗口顺序，权重值越大窗口越靠左（详见 [Window Order 文档](docs/zh/plugins/window_order.md)）

## 窗口匹配机制

Piri 使用统一的窗口匹配机制，支持通过正则表达式匹配窗口的 `app_id` 和 `title`。多个插件（如 `window_rule`、`singleton`、`scratchpads`）都使用此机制来查找和匹配窗口。

**支持的匹配方式**：
- ✅ **正则表达式匹配**: 支持完整的正则表达式语法
- ✅ **灵活匹配**: 支持 `app_id` 和/或 `title` 匹配
- ✅ **OR 逻辑**: 如果同时指定 `app_id` 和 `title`，任一匹配即可

**详细文档**: [窗口匹配机制文档](docs/zh/window_matching.md)

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

![Scratchpads](./assets/scratchpads.mp4)

快速显示和隐藏常用应用程序的窗口。支持跨 workspace 和 monitor，无论你在哪个工作区或显示器上，都能快速访问你的 scratchpad 窗口。

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

### Window Rule

根据窗口的 `app_id` 或 `title` 使用正则表达式匹配，自动将窗口移动到指定的 workspace，并支持在窗口获得焦点时执行命令。

**配置示例**：
```toml
[piri.plugins]
window_rule = true

# 根据 app_id 匹配，移动到 workspace（精确匹配：先 name，后 idx）
[[window_rule]]
app_id = ".*firefox.*"
open_on_workspace = "2"

# 根据 title 匹配，移动到 workspace，并在获得焦点时执行命令
[[window_rule]]
title = ".*Chrome.*"
open_on_workspace = "3"
focus_command = "[[ $(fcitx5-remote) -eq 2 ]] && fcitx5-remote -c"

# 同时指定 app_id 和 title（任一匹配即可），移动到 workspace（name）
[[window_rule]]
app_id = "code"
title = ".*VS Code.*"
open_on_workspace = "browser"

# 只有 focus_command，不移动窗口
[[window_rule]]
title = ".*Chrome.*"
focus_command = "notify-send 'Chrome focused'"
```

详细说明请参考 [Window Rule 文档](docs/zh/plugins/window_rule.md) 和 [窗口匹配机制文档](docs/zh/window_matching.md)。

### Autofill

![Autofill](./assets/autofill.mp4)

在窗口关闭或布局改变时自动将最后一列窗口对齐到最右侧位置，保持整洁有序的窗口布局。

**配置示例**：
```toml
[piri.plugins]
autofill = true
```

**特性**：
- 无需配置，开箱即用
- 纯事件驱动，实时响应
- 工作区感知，仅影响当前工作区
- 自动保持整洁的窗口布局

详细说明请参考 [Autofill 文档](docs/zh/plugins/autofill.md)。

### Singleton

管理单例窗口——只应该有一个实例的窗口。当你切换一个单例时，如果窗口已经存在，它会聚焦该窗口；否则，它会启动应用程序。这对于浏览器、终端或其他通常只需要一个实例运行的工具非常有用。

**配置示例**：
```toml
[piri.plugins]
singleton = true

[singleton.browser]
command = "google-chrome-stable"

[singleton.term]
command = "GTK_IM_MODULE=wayland ghostty --class=singleton.term"
app_id = "singleton.term"
```

**快速使用**：
```bash
# 切换单例窗口（如果存在则聚焦，不存在则启动）
piri singleton {name} toggle
```

**特性**：
- 智能窗口检测，自动检测现有窗口
- 自动 App ID 提取，无需手动指定
- 窗口注册表，快速查找已存在的窗口
- 自动聚焦现有窗口，避免重复实例

详细说明请参考 [Singleton 文档](docs/zh/plugins/singleton.md)。

### Window Order

![Window Order - 手动触发](./assets/window_order.mp4)

![Window Order - 事件监听自动触发](./assets/window_order_envent.mp4)

根据配置的权重值自动重排工作区中的窗口顺序。权重值越大，窗口越靠左。

**配置示例**：
```toml
[piri.plugins]
window_order = true

[piri.window_order]
enable_event_listener = true  # 启用事件监听，自动重排
default_weight = 0           # 未配置窗口的默认权重
# workspaces = ["1", "2", "dev"]  # 可选：仅在指定工作区应用（空列表 = 所有工作区）

[window_order]
google-chrome = 100
code = 80
ghostty = 70
```

**快速使用**：
```bash
# 手动触发窗口重排（可在任意工作区执行）
piri window_order toggle
```

**特性**：
- 智能排序算法，最小化窗口移动次数
- 支持手动触发和事件驱动自动触发
- 支持工作区过滤（仅自动触发时生效）
- 相同权重窗口保持相对顺序
- 支持 `app_id` 部分匹配

详细说明请参考 [Window Order 文档](docs/zh/plugins/window_order.md)。

## 文档

- [架构设计](docs/architecture.md) - 项目架构和工作原理
- [插件系统](docs/plugins.md) - 插件系统详细说明
- [开发指南](docs/development.md) - 开发、扩展和贡献指南

## 许可证

MIT License

## 参考项目

本项目受到 [Pyprland](https://github.com/hyprland-community/pyprland) 的启发。Pyprland 是一个为 Hyprland 合成器提供扩展功能的优秀项目，提供了大量插件来增强用户体验。
