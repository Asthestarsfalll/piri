# 插件系统

Piri 支持插件系统，允许你扩展功能。插件在守护进程模式下自动运行。

## 插件控制

你可以在配置文件中控制哪些插件启用或禁用：

```toml
[piri.plugins]
scratchpads = true
empty = true
window_rule = true
```

**默认行为**：
- 如果未明确指定，插件默认**禁用**（`false`）
- 必须显式设置 `scratchpads = true`、`empty = true` 或 `window_rule = true` 来启用插件
- `window_rule` 插件例外：如果配置了窗口规则，默认启用（除非显式设置为 `false`）

## Empty 插件

Empty 插件用于在切换到特定的空 workspace 时执行相应的命令。这对于自动化工作流程非常有用，例如在空 workspace 中自动启动应用程序。

### 配置

在配置文件中使用 `[empty.{workspace}]` 格式配置 workspace 规则：

```toml
# 当切换到 workspace 1 且为空时，执行命令
[empty.1]
command = "notify-send 'Workspace 1 is empty'"

# 当切换到 workspace 2 且为空时，执行命令
[empty.2]
command = "echo 'Workspace 2 is empty' > /tmp/ws2.log"

# 使用 workspace 名称
[empty.main]
command = "firefox"
```

### Workspace 标识符

Empty 插件支持两种 workspace 标识符类型：

1. **name (名称)**: Workspace 的名称（字符串），例如 `"main"`, `"work"`
2. **idx (索引)**: Workspace 的索引号，通常是 1-based，例如 `"1"`, `"2"`, `"3"`

**匹配顺序**：插件会按照 **name 优先，然后 idx** 的顺序进行匹配。不支持 id (u64) 匹配。

插件会自动识别标识符类型，并支持跨类型匹配（例如，如果当前 workspace 是 idx，配置中使用 name，插件会先尝试 name 匹配，再尝试 idx 匹配）。

### 工作原理

Empty 插件使用**纯事件驱动**的方式实时监听 niri 合成器的事件：

1. **事件流监听**: 插件通过 niri IPC 的 `EventStream` 实时监听 workspace 激活事件
2. **实时响应**: 当收到 `WorkspaceActivated` 事件时（表示 workspace 已切换），立即查询当前 workspace 状态
3. **检测 workspace 状态**: 查询 workspace 是否为空（通过 `active_window_id` 字段）
4. **执行命令**: 如果 workspace 为空且匹配配置规则，立即执行命令

### 特性

- ✅ **纯事件驱动**: 使用 niri 事件流实时监听，`read_event()` 阻塞等待事件，事件到达时自动唤醒
- ✅ **自动检测**: 自动检测 workspace 切换，无需手动触发
- ✅ **灵活匹配**: 支持 name 和 idx 两种标识符类型，匹配顺序为 name 优先，然后 idx
- ✅ **自动重连**: 连接断开时自动重连，确保服务持续运行

### 使用场景示例

```toml
# 在空 workspace 1 中自动启动终端
[empty.1]
command = "alacritty"

# 在空 workspace 2 中自动启动浏览器
[empty.2]
command = "firefox"

# 在空 workspace 3 中自动启动编辑器
[empty.3]
command = "code"

# 使用名称标识符
[empty.work]
command = "emacs"
```

## Scratchpads 插件

Scratchpads 功能通过插件系统实现。详细说明请参考 [Scratchpads 文档](scratchpads.md)。

## Window Rule 插件

Window Rule 插件用于根据窗口的 `app_id` 或 `title` 自动将窗口移动到指定的 workspace。这对于自动化窗口管理非常有用。

### 配置

在配置文件中使用 `[[window_rule]]` 格式配置窗口规则：

```toml
# 根据 app_id 匹配
[[window_rule]]
app_id = "ghostty"
open_on_workspace = "1"

# 根据 title 匹配
[[window_rule]]
title = "^kitty"
open_on_workspace = "browser"

# 同时指定 app_id 和 title（任一匹配即可）
[[window_rule]]
app_id = "code"
title = ".*VS Code.*"
open_on_workspace = "dev"
```

### 特性

- ✅ **纯事件驱动**: 使用 niri 事件流实时监听，窗口创建时自动处理
- ✅ **正则表达式支持**: 支持强大的正则表达式模式匹配
- ✅ **灵活匹配**: 支持按 `app_id` 或 `title` 匹配，或两者组合（OR 逻辑）
- ✅ **Workspace 匹配**: 支持 name 和 idx 两种标识符类型，匹配顺序为 name 优先，然后 idx

详细说明请参考 [Window Rule 文档](window_rule.md)。

