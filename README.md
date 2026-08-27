# Piri

[English](README.en.md) | **中文**

---

Piri 是基于 Rust 开发的 [Niri](https://github.com/YaLTeR/niri) 合成器扩展工具，通过高效的 IPC 交互与统一的事件分发机制，提供稳健的状态驱动插件系统。

## 核心插件

- 📦 **Scratchpads**: 智能隐藏/显示窗口，支持自动捕获或按需启动，跨工作区与显示器跟随（详见 [文档](docs/zh/plugins/scratchpads.md)）
- 🔌 **Empty**: 空工作区自动化，切换时自动执行命令（详见 [文档](docs/zh/plugins/empty.md)）
- 🎯 **Window Rule**: 规则引擎，基于正则匹配实现窗口自动归位与焦点触发命令（详见 [文档](docs/zh/plugins/window_rule.md)）
- 📐 **Workspace Rule**: 工作区布局管理，提供自动宽度、平铺、对齐、最大化及 EdgePulse 边缘提示（详见 [文档](docs/zh/plugins/workspace_rule.md), [EdgePulse](docs/zh/plugins/edge_pulse.md)）
- 🔒 **Singleton**: 单实例保障，确保应用全局唯一（详见 [文档](docs/zh/plugins/singleton.md)）
- 📌 **Mark**: 窗口标记与快速聚焦，绑定存于内存（详见 [文档](docs/zh/plugins/mark.md)）
- 📍 **Sticky**: 浮动窗口跟随，支持跨显示器（详见 [文档](docs/zh/plugins/sticky.md)）
- 📋 **Window Order**: 按权重自动重排窗口，最小化移动（详见 [文档](docs/zh/plugins/window_order.md)）
- 🍽️ **Swallow**: 窗口吞噬，子窗口替换父窗口位置（详见 [文档](docs/zh/plugins/swallow.md)）

## 核心特性

- **Rust 高性能**：基于 Rust 构建，低内存占用，异步事件处理
- **高效 IPC 交互**：通过 Unix Socket 与 Niri 合成器通信，自动重连，批量指令执行
- **统一事件分发**：单一事件监听器统一接收 Niri 事件，按插件兴趣分发
- **细粒度事件拆分**：自动将 Niri 粗粒度事件拆分为 `WindowOpened`、`WindowChanged`、`WindowToggleFloating` 等子事件，插件无需自行追踪窗口状态（详见 [文档](docs/zh/event_normalization.md)）
- **状态驱动插件系统**：每个插件实现统一的 `Plugin` trait，支持配置热更新、事件过滤与 IPC 命令处理
- **配置热重载**：修改 `piri.toml` 后自动生效，无需重启 daemon

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

### 启动 daemon

#### 手动启动

```bash
# 启动 daemon（前台运行）
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

https://github.com/user-attachments/assets/1c40d75b-310a-4503-9d59-92f0479a54d5

快速显示/隐藏常用窗口，支持跨工作区与显示器。特性：**动态添加**、**保留手动调整**、**隐藏后移至指定工作区**、**吞入聚焦窗口**（`swallow_to_focus`）、**Sticky 跟随**（`sticky`，委托 sticky 插件）、**失焦自动隐藏**（`auto_hide_on_focus_loss`）及**非浮动窗口直接聚焦**。

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

[scratchpads.note]
direction = "fromTop"
command = "gnome-text-editor"
app_id = "org.gnome.TextEditor"
size = "50% 40%"
margin = 100
sticky = true  # 跟随焦点工作区（由 sticky 插件处理）

[scratchpads.calc]
direction = "fromBottom"
command = "gnome-calculator"
app_id = "org.gnome.Calculator"
size = "30% 40%"
margin = 50
auto_hide_on_focus_loss = true  # 失去焦点时自动隐藏
```

> **注意**: 同一个 scratchpad 不能同时启用 `sticky` 和 `auto_hide_on_focus_loss`。

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

基于正则匹配自动归位窗口至指定 workspace，支持焦点触发命令执行。

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

# 锁定窗口到指定 workspace（末尾 ! 表示锁定，窗口无法被移走）
[[window_rule]]
app_id = "firefox"
open_on_workspace = "1@eDP!"

# title 作为列表（任意一个匹配即可）
[[window_rule]]
title = [".*Chrome.*", ".*Chromium.*", ".*Google Chrome.*"]
open_on_workspace = "browser"
```

**特性**：
- 正则匹配（`app_id`/`title`，支持列表与 OR 逻辑）
- 支持 workspace 名称/索引
- Workspace 锁定（`!` 后缀），防止窗口被移走
- 焦点触发命令，内置去重机制
- `focus_command_once`：规则级单次执行（[issue #1](https://github.com/Asthestarsfalll/piri/issues/1)）
- 纯事件驱动

详细说明请参考 [Window Rule 文档](docs/zh/plugins/window_rule.md) 和 [窗口匹配机制文档](docs/zh/window_matching.md)。

### Workspace Rule

https://github.com/user-attachments/assets/092a383c-993b-42b5-9b89-b78b0807b436

https://github.com/user-attachments/assets/b48b0465-0101-4298-9935-22f46d1a2658

https://github.com/user-attachments/assets/3b8dc835-473c-4b6e-b684-02a6951b63f9

https://github.com/user-attachments/assets/2e9e2c86-b9ef-44f1-a896-46473b93b417

工作区布局管理，提供自动宽度、平铺、对齐与最大化。内置 EdgePulse 边缘提示（支持动画），聚焦列到达边缘时渲染视觉提示。

**配置示例**：
```toml
[piri.plugins]
workspace_rule = true

# 默认配置（应用到所有工作区）
[piri.workspace_rule]
auto_width = ["100%", "50%", "33.33%", "25%", "20%"]
auto_fill = true  # 启用自动对齐
auto_maximize = true  # 自动最大化

# EdgePulse 边缘提示（支持动画）
[piri.workspace_rule.edge_pulse]
enabled = true
animation_enabled = true  # 启用动画
animation_style = "pulse"  # pulse 呼吸效果，fade 淡入效果
animation_duration = 600  # 动画周期（毫秒）
animation_amplitude = 0.8  # 动画强度
animation_repeat = 3  # 每次触发播放次数（0=无限）

# 工作区特定配置
[workspace_rule.main]
auto_maximize = true

[workspace_rule.dev]
auto_width = ["100%", ["45%", "55%"], ["30%", "35%", "35%"]]
auto_tile = true  # auto_tile
```

**特性**：
- 自动宽度调整：根据窗口数量自动调整窗口宽度
- auto_tile：自动将新窗口合并到现有列中
- 自动对齐：窗口关闭后自动对齐到最右侧
- 自动最大化：单窗口时自动最大化，多窗口时取消最大化
- EdgePulse 边缘提示：聚焦列到达边缘时渲染视觉提示
- 工作区感知：每个工作区可独立配置

详细说明请参考 [Workspace Rule 文档](docs/zh/plugins/workspace_rule.md)。

### Singleton

管理单例窗口，确保全局唯一。切换时聚焦已有实例或启动新进程，适用于浏览器、终端等。

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
- 智能检测现有窗口，自动提取 App ID
- 窗口注册表快速查找
- 支持创建后执行命令（`on_created_command`）

详细说明请参考 [Singleton 文档](docs/zh/plugins/singleton.md)。

### Mark

为窗口设置命名标记（如 `a`、`b`），用于快速聚焦。标记存于内存，重启 daemon 后清空。无需配置文件，启用插件后通过 Niri `spawn` 绑定即可。

**配置示例**：

```toml
[piri.plugins]
mark = true
```

**快速使用**：

```bash
# 无有效绑定时：把当前焦点窗口绑定到名称；已有且窗口仍在：聚焦该窗口
piri mark {name} toggle

# 强制把当前窗口绑定到名称（覆盖原绑定）
piri mark {name} add

# 删除该标记
piri mark {name} delete
```

**说明**：Piri 无法全局捕获「下一个按键」；若希望少占快捷键，可配合启动器（如 `fuzzel`）选字母后再调用上述命令。Niri 若支持多键序列或绑定模式，可将多条 `piri mark …` 收拢在同一前缀下。

详细说明请参考 [Mark 文档](docs/zh/plugins/mark.md)。

### Sticky

将当前焦点的**浮动窗口**设为“跟随窗口”，在你切换焦点工作区时自动跟随。适用于词典、翻译、日志、播放器控制等常驻小窗场景。

**配置示例**：

```toml
[piri.plugins]
sticky = true
```

**快速使用**：

```bash
# 设为 sticky（仅同 monitor 跟随）
piri sticky add

# 设为 sticky（允许跨 monitor）
piri sticky add --cross

# 取消 sticky
piri sticky delete
```

详细说明请参考 [Sticky 文档](docs/zh/plugins/sticky.md)。

### Window Order

https://github.com/user-attachments/assets/2c9cbbb4-7001-44ce-acfd-afb51dfbc372

https://github.com/user-attachments/assets/9818d478-3a33-456b-8367-548bb8ab7da7

按权重自动重排窗口，权重越大越靠左。

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
- 智能排序，最小化移动次数
- 支持手动/事件驱动触发
- 支持工作区过滤
- 相同权重保持相对顺序
- 支持 `app_id` 部分匹配

详细说明请参考 [Window Order 文档](docs/zh/plugins/window_order.md)。

### Swallow

https://github.com/user-attachments/assets/9968e97f-fdea-4211-a007-717edf703e93

https://github.com/user-attachments/assets/51567d89-8ca8-4f4a-b2ca-732dfc0741c9

子窗口打开时自动隐藏父窗口，让子窗口替换父窗口位置。适用于终端启动图片查看器或媒体播放器等场景。

**配置示例**：
```toml
[piri.plugins]
swallow = true

[piri.swallow]
use_pid_matching = true  # 启用基于 PID 的父子进程匹配（默认：true）
swallow_on_change = false  # 窗口 title/app_id 变化时重新检查吞噬规则（默认：false）

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
- 支持 PID 父子进程匹配（默认启用）
- 支持规则匹配（`app_id`/`title`/`pid`）
- 支持全局/规则级排除
- 智能聚焦窗口队列
- 自动处理工作区移动与浮动转换

详细说明请参考 [Swallow 文档](docs/zh/plugins/swallow.md)。

## 文档

- [架构设计](docs/zh/architecture.md) - 项目架构和工作原理
- [细粒度事件拆分](docs/zh/event_normalization.md) - 事件拆分机制详解
- [插件系统](docs/zh/plugins/plugins.md) - 插件系统详细说明
- [开发指南](docs/zh/development.md) - 开发、扩展和贡献指南

## 许可证

MIT License

## 参考项目

本项目受到 [Pyprland](https://github.com/hyprland-community/pyprland) 的启发。Pyprland 是一个为 Hyprland 合成器提供扩展功能的优秀项目，提供了大量插件来增强用户体验。
