# Piri

[English](README.en.md) | **中文**

---

Piri 是基于 Rust 的 [Niri](https://github.com/YaLTeR/niri) 高性能功能扩展工具。它通过高效的 Niri IPC 交互和统一的事件分发机制，为 Niri 提供稳健的状态驱动插件系统。

## 核心插件

- 📦 **Scratchpads**: 智能隐藏/显示窗口。支持自动捕获已有窗口或按需启动应用，跨工作区与显示器无缝跟随（详见 [Scratchpads 文档](docs/zh/plugins/scratchpads.md)）
- 🔌 **Empty**: 空白工作区自动化。在切换到空工作区时自动触发预设命令，助您快速进入工作状态（详见 [Empty 文档](docs/zh/plugins/empty.md)）
- 🎯 **Window Rule**: 强大规则引擎。基于正则匹配实现窗口自动归位，并提供带去重机制的焦点触发命令执行（详见 [Window Rule 文档](docs/zh/plugins/window_rule.md)）
- 📐 **Workspace Rule**: 工作区窗口布局管理。提供自动宽度调整、自动平铺、自动对齐和自动最大化等功能，整合了原 Autofill 功能（详见 [Workspace Rule 文档](docs/zh/plugins/workspace_rule.md)）
- 🔒 **Singleton**: 单实例保障。确保特定应用全局唯一，支持快速聚焦现有实例或自动拉起新进程（详见 [Singleton 文档](docs/zh/plugins/singleton.md)）
- 📋 **Window Order**: 智能窗口排序。根据配置权重自动重排平铺窗口，相同权重窗口保持相对位置以最小化移动损耗（详见 [Window Order 文档](docs/zh/plugins/window_order.md)）
- 🍽️ **Swallow**: 窗口吞噬机制。当子窗口打开时自动隐藏父窗口，让子窗口在布局中替换父窗口的位置（详见 [Swallow 文档](docs/zh/plugins/swallow.md)）

## 窗口匹配机制

Piri 使用统一的窗口匹配机制，支持通过正则表达式匹配窗口的 `app_id` 和 `title`。多个插件（如 `window_rule`、`singleton`、`scratchpads`）都使用此机制来查找和匹配窗口。

**支持的匹配方式**：

- ✅ **正则表达式匹配**: 支持完整的正则表达式语法
- ✅ **灵活匹配**: 支持 `app_id` 和/或 `title` 匹配
- ✅ **OR 逻辑**: 如果同时指定 `app_id` 和 `title`，任一匹配即可

> **注意**: Window Rule 插件额外支持列表匹配（`app_id` 和 `title` 可以是列表），详见 [Window Rule 文档](docs/zh/plugins/window_rule.md)。

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

#### 手动启动

```bash
# 启动守护进程（前台运行）
piri daemon
```

```bash
# 更多调试日志
piri --debug daemon
```

#### 自动启动（推荐）

在 niri 配置文件中添加以下配置，让 piri daemon 在 niri 启动时自动运行：

编辑 `~/.config/niri/config.kdl`，在 `spawn-at-startup` 部分添加：

```kdl
spawn-at-startup "bash" "-c" "/path/to/piri daemon > /dev/null 2>&1 &"
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

快速显示和隐藏常用应用程序的窗口。支持跨 workspace 和 monitor，无论你在哪个工作区或显示器上，都能快速访问你的 scratchpad 窗口。支持**动态添加窗口**、**自动保留手动调整的大小与边距**、**隐藏后自动移动到指定工作区**，以及**将窗口吞入当前聚焦的窗口**（`swallow_to_focus` 选项）。

**配置示例**：

```toml
[piri.plugins]
scratchpads = true

[piri.scratchpad]
default_size = "40% 60%"
default_margin = 50
move_to_workspace = "tmp" # 窗口隐藏后自动移动到工作区 tmp

[scratchpads.term]
direction = "fromRight"
command = "GTK_IM_MODULE=wayland ghostty --class=float.dropterm"
app_id = "float.dropterm"
size = "40% 60%"
margin = 50

[scratchpads.preview]
direction = "fromRight"
command = "imv"
app_id = "imv"
size = "60% 80%"
margin = 50
swallow_to_focus = true  # 显示时自动吞入当前聚焦的窗口
```

**快速使用**：

```bash
# 切换 scratchpad 显示/隐藏
piri scratchpads {name} toggle

# 动态添加当前窗口为 scratchpad
piri scratchpads {name} add {direction} [--swallow-to-focus]

# 示例
piri scratchpads mypad add fromRight
piri scratchpads mypad add fromRight --swallow-to-focus  # 启用 swallow 功能
```

> **提示**:
>
> - 动态添加的窗口仅在第一次注册时使用默认大小和边距。之后你可以手动调整窗口的大小和位置（边距），插件会自动保留这些调整。
> - 如果 scratchpad 已存在，`add` 命令会自动执行 toggle 操作（显示/隐藏切换）。

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

# focus_command 仅对规则全局执行一次（规则级别，非窗口级别）
[[window_rule]]
app_id = "firefox"
focus_command = "notify-send 'Firefox focused'"
focus_command_once = true

# app_id 作为列表（任意一个匹配即可）
[[window_rule]]
app_id = ["code", "code-oss", "codium"]
open_on_workspace = "dev"

# title 作为列表（任意一个匹配即可）
[[window_rule]]
title = [".*Chrome.*", ".*Chromium.*", ".*Google Chrome.*"]
open_on_workspace = "browser"
```

**特性**：

- 正则表达式模式匹配支持
- 根据 `app_id` 或 `title` 匹配，或两者组合（OR 逻辑）
- 支持模式列表：`app_id` 和 `title` 可以是列表，任意一个匹配即可触发规则
- 支持 workspace 名称或索引匹配
- 焦点触发的命令执行，内置去重机制
- `focus_command_once` 选项：对每个规则全局仅执行一次 `focus_command`（参见 [issue #1](https://github.com/Asthestarsfalll/piri/issues/1)）
- 纯事件驱动，实时响应窗口创建

详细说明请参考 [Window Rule 文档](docs/zh/plugins/window_rule.md) 和 [窗口匹配机制文档](docs/zh/window_matching.md)。

### Workspace Rule

![Workspace Rule - Autofill](./assets/autofill.mp4)

![Workspace Rule - Auto Tile](./assets/auto_tile.mp4)

工作区窗口布局管理插件，提供自动宽度调整、自动平铺、自动对齐和自动最大化等功能。整合了原 Autofill 插件的功能，提供更全面的工作区窗口管理能力。

**配置示例**：

```toml
[piri.plugins]
workspace_rule = true

# 默认配置（应用到所有工作区）
[piri.workspace_rule]
auto_width = ["100%", "50%", "33.33%", "25%", "20%"]
auto_fill = true  # 自动对齐（原 Autofill 功能）
auto_maximize = true  # 自动最大化

# 工作区特定配置
[workspace_rule.main]
auto_maximize = true

[workspace_rule.dev]
auto_width = ["100%", ["45%", "55%"], ["30%", "35%", "35%"]]
auto_tile = true  # 自动平铺
```

**特性**：

- 自动宽度调整：根据窗口数量自动调整窗口宽度
- 自动平铺：自动将新窗口合并到现有列中
- 自动对齐：窗口关闭后自动对齐到最右侧（原 Autofill 功能）
- 自动最大化：只有一个窗口时自动最大化，多个窗口时自动取消最大化
- 工作区感知：每个工作区可以独立配置
- 灵活配置：支持默认配置和工作区特定配置

**从 Autofill 迁移**：

```toml
# 旧配置
[piri.plugins]
autofill = true

# 新配置
[piri.plugins]
workspace_rule = true

[piri.workspace_rule]
auto_fill = true  # 启用原 Autofill 功能
```

详细说明请参考 [Workspace Rule 文档](docs/zh/plugins/workspace_rule.md)。

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

[singleton.editor]
command = "code"
app_id = "code"
on_created_command = "notify-send '编辑器已打开'"
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
- 支持窗口创建后执行自定义命令（`on_created_command`）

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

### Swallow

![Swallow](./assets/autofill_1.mp4)

当子窗口打开时自动隐藏父窗口，让子窗口在布局中替换父窗口的位置。这对于终端启动图片查看器或媒体播放器等场景非常有用。

**配置示例**：

```toml
[piri.plugins]
swallow = true

[piri.swallow]
use_pid_matching = true  # 启用基于 PID 的父子进程匹配（默认：true）

# 全局排除规则（可选）
[piri.swallow.exclude]
app_id = [".*dialog.*"]

# 规则列表
[[swallow]]
parent_app_id = [".*terminal.*", ".*alacritty.*", ".*foot.*", ".*ghostty.*"]
child_app_id = [".*mpv.*", ".*imv.*", ".*feh.*"]

[[swallow]]
parent_app_id = ["code", "nvim-qt"]
child_app_id = [".*preview.*", ".*markdown.*"]
```

**特性**：

- 支持基于 PID 的父子进程匹配（默认启用）
- 支持基于规则的匹配（通过 `app_id`、`title` 或 `pid` 模式）
- 支持全局和规则级别的排除规则
- 智能聚焦窗口队列，自动查找父窗口
- 自动处理工作空间移动和浮动窗口转换
- 智能浮动窗口处理：浮动窗口不会被吞噬，从浮动转为平铺时会重新尝试吞噬

详细说明请参考 [Swallow 文档](docs/zh/plugins/swallow.md)。

## 文档

- [架构设计](docs/zh/architecture.md) - 项目架构和工作原理
- [插件系统](docs/zh/plugins/plugins.md) - 插件系统详细说明
- [开发指南](docs/zh/development.md) - 开发、扩展和贡献指南

## 许可证

MIT License

## 参考项目

本项目受到 [Pyprland](https://github.com/hyprland-community/pyprland) 的启发。Pyprland 是一个为 Hyprland 合成器提供扩展功能的优秀项目，提供了大量插件来增强用户体验。
