# Piri

[English](README.en.md) | **中文**

---

欢迎来到 Piri，您扩展 niri 合成器功能的门户。Piri 提供了可扩展的命令系统，旨在简化和高效，允许你提升你的生产力并定制你的用户体验。

您可以将其视为类似工具但针对 niri 用户（涉及编辑文本文件）。通过基于命令的架构，Piri 被设计为轻量级且易于使用。

请注意，使用 Rust 和守护进程架构鼓励使用多个功能而对内存占用和性能影响不大。

欢迎贡献、建议、bug 报告和评论。

> **注意**: 本项目完全由 [Cursor](https://cursor.sh/) AI 代码编辑器配合完成开发。

## 功能特性

- 🚀 **守护进程模式**: 可以作为守护进程运行，提供持续的服务
- 🔧 **Niri IPC 封装**: 提供了完整的 niri IPC 命令封装，方便与 niri 交互
- 📝 **TOML 配置**: 使用 TOML 格式的配置文件，易于阅读和编辑
- 🎯 **可扩展命令系统**: 支持 `piri sub_command` 格式的子命令系统，便于添加新功能
- 📦 **Scratchpads**: 强大的窗口管理功能，支持快速访问常用应用程序（详见 [Scratchpads](#scratchpads) 章节）

## 安装

### 使用安装脚本（推荐）

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

### 使用 Cargo 安装

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

## 配置

将示例配置文件复制到配置目录：

```bash
mkdir -p ~/.config/niri
cp config.example.toml ~/.config/niri/piri.toml
```

然后编辑 `~/.config/niri/piri.toml` 来配置你的功能。

### 基础配置示例

```toml
[niri]
# socket_path = "/tmp/niri"  # 可选，默认使用 $XDG_RUNTIME_DIR/niri

[piri.scratchpad]
# 动态添加 scratchpad 时的默认大小和边距
default_size = "40% 60%"  # 默认大小，格式为 "width% height%"
default_margin = 50        # 默认边距（像素）
```

更多配置选项请参考各个功能模块的详细说明。

## 使用方法

### 启动守护进程

```bash
# 启动守护进程（前台运行）
piri daemon
```

### 重新加载配置

```bash
# 重新加载配置文件（无需重启守护进程）
piri reload
```

注意：重新加载配置后，新配置会立即生效。已存在的 scratchpad 窗口将继续使用旧配置，新启动的 scratchpad 将使用新配置。

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

## Scratchpads

Scratchpads 是一个强大的窗口管理功能，允许你快速显示和隐藏常用应用程序的窗口。它支持跨 workspace 和 monitor，无论你在哪个工作区或显示器上，都能快速访问你的 scratchpad 窗口。

### 演示视频

<video src="assets/scratchpads.mp4" controls width="100%"></video>

### 配置

在配置文件中添加 `[scratchpads.{name}]` 节来配置 scratchpad。每个 scratchpad 需要一个唯一的名称。

#### 配置示例

```toml
[scratchpads.term]
direction = "fromRight"
command = "GTK_IM_MODULE=wayland ghostty --class=float.dropterm"
app_id = "float.dropterm"
size = "40% 60%"
margin = 50

[scratchpads.calc]
direction = "fromBottom"
command = "gnome-calculator"
app_id = "gnome-calculator"
size = "50% 40%"
margin = 100
```

#### 配置参数说明

- `direction` (必需): 窗口出现的方向
  - `fromTop`: 从顶部滑入
  - `fromBottom`: 从底部滑入
  - `fromLeft`: 从左侧滑入
  - `fromRight`: 从右侧滑入

- `command` (必需): 启动应用程序的完整命令字符串，可以包含环境变量和参数

- `app_id` (必需): 用于匹配窗口的应用 ID。这是 niri 识别窗口的关键标识符

- `size` (必需): 窗口大小，格式为 `"width% height%"`，例如 `"40% 60%"` 表示宽度为屏幕的 40%，高度为屏幕的 60%

- `margin` (必需): 距离屏幕边缘的边距（像素）

### 使用方法

#### 切换 Scratchpad 显示/隐藏

```bash
piri scratchpads {name} toggle
```

例如：

```bash
# 切换终端 scratchpad
piri scratchpads term toggle

# 切换计算器 scratchpad
piri scratchpads calc toggle
```

#### 动态添加当前窗口为 Scratchpad

你可以将当前聚焦的窗口快速添加为 scratchpad，无需编辑配置文件：

```bash
piri scratchpads {name} add {direction}
```

参数说明：
- `{name}`: Scratchpad 的名称（唯一标识符）
- `{direction}`: 窗口出现的方向，可选值：
  - `fromTop`: 从顶部滑入
  - `fromBottom`: 从底部滑入
  - `fromLeft`: 从左侧滑入
  - `fromRight`: 从右侧滑入

例如：

```bash
# 将当前窗口添加为名为 "mypad" 的 scratchpad，从右侧滑入
piri scratchpads mypad add fromRight
```

动态添加的 scratchpad 会使用配置文件 `[piri.scratchpad]` 节中设置的默认大小和边距。添加后，你可以使用 `piri scratchpads {name} toggle` 来切换显示/隐藏。

#### 工作原理

1. **首次启动**: 当执行 `piri scratchpads {name} toggle` 时，如果窗口不存在，会启动配置中指定的应用程序

2. **窗口注册**: 找到窗口后，将其设置为浮动模式，并移动到屏幕外

3. **显示/隐藏**: 
   - **显示**: 
     - 记录当前聚焦的窗口
     - 将窗口移动到当前聚焦的输出和工作区，并根据配置的方向和大小定位
     - 将焦点转移到 scratchpad 窗口
     - **支持跨 workspace 和 monitor**: 无论 scratchpad 窗口原本在哪个工作区或显示器上，都会自动移动到当前聚焦的位置
   - **隐藏**: 
     - 将窗口移动到屏幕外
     - 恢复焦点：
       - 如果之前聚焦的窗口在当前工作区，则聚焦到它
       - 如果不在当前工作区，且当前工作区存在其他窗口，则聚焦到最中间的窗口

### 特性

- ✅ **跨 workspace 支持**: 无论你在哪个工作区，都能快速访问 scratchpad
- ✅ **跨 monitor 支持**: 支持多显示器环境，scratchpad 会自动出现在当前聚焦的显示器上
- ✅ **智能焦点管理**: 显示时自动聚焦，隐藏时智能恢复之前的焦点
- ✅ **灵活配置**: 支持自定义窗口大小、位置和动画方向
- ✅ **动态添加**: 可以将当前聚焦的窗口快速添加为 scratchpad，无需编辑配置文件

## 工作原理

### 架构设计

项目采用模块化设计，便于扩展：

- `config.rs`: 配置管理模块
- `niri.rs`: Niri IPC 封装模块
- `commands.rs`: 命令处理系统
- `scratchpads.rs`: Scratchpads 功能实现
- `daemon.rs`: 守护进程管理
- `ipc.rs`: 进程间通信（用于客户端与守护进程通信）

## 扩展性

### 添加新的子命令

1. 在 `src/main.rs` 的 `Commands` 枚举中添加新的命令
2. 在 `src/commands.rs` 的 `CommandHandler` 中添加处理方法
3. 实现相应的功能模块
4. 在 `src/ipc.rs` 中添加相应的 IPC 消息类型（如果需要）

### 添加新的配置项

1. 在 `src/config.rs` 的 `Config` 结构体中添加字段
2. 更新 TOML 配置文件示例

## 开发

### 代码格式化

项目使用 `rustfmt` 进行代码格式化。配置文件为 `rustfmt.toml`。

#### 安装 rustfmt

```bash
rustup component add rustfmt
```

#### 格式化代码

```bash
# 格式化所有代码
cargo fmt

# 检查代码格式（不修改文件）
cargo fmt -- --check
```

## 依赖

- `clap`: 命令行参数解析
- `serde` / `toml`: 配置序列化/反序列化
- `tokio`: 异步运行时
- `anyhow`: 错误处理
- `log` / `env_logger`: 日志系统
- `niri-ipc`: Niri IPC 客户端库

## 许可证

MIT License

## 参考项目

本项目受到 [Pyprland](https://github.com/hyprland-community/pyprland) 的启发。Pyprland 是一个为 Hyprland 合成器提供扩展功能的优秀项目，提供了大量插件来增强用户体验。
