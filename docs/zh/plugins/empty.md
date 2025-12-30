# Empty 插件

Empty 插件用于在切换到特定的空 workspace 时执行相应的命令。这对于自动化工作流程非常有用，例如在空 workspace 中自动启动应用程序。

> **参考**: 此功能类似于 [Hyprland 的 `on-created-empty` workspace rule](https://wiki.hypr.land/Configuring/Workspace-Rules/#rules)。

## 配置

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

## Workspace 标识符

Empty 插件支持两种 workspace 标识符类型：

1. **name (名称)**: Workspace 的名称（字符串），例如 `"main"`, `"work"`
2. **idx (索引)**: Workspace 的索引号，通常是 1-based，例如 `"1"`, `"2"`, `"3"`

**匹配顺序**：插件会按照 **name 优先，然后 idx** 的顺序进行匹配。不支持 id (u64) 匹配。

插件会自动识别标识符类型，并支持跨类型匹配（例如，如果当前 workspace 是 idx，配置中使用 name，插件会先尝试 name 匹配，再尝试 idx 匹配）。

## 工作原理

Empty 插件使用**纯事件驱动**的方式实时监听 niri 合成器的事件：

1. **事件流监听**: 插件通过 niri IPC 的 `EventStream` 实时监听 workspace 激活事件
2. **实时响应**: 当收到 `WorkspaceActivated` 事件时（表示 workspace 已切换），立即查询当前 workspace 状态
3. **检测 workspace 状态**: 查询 workspace 是否为空（通过 `active_window_id` 字段）
4. **执行命令**: 如果 workspace 为空且匹配配置规则，立即执行命令

## 特性

- ✅ **纯事件驱动**: 使用 niri 事件流实时监听，`read_event()` 阻塞等待事件，事件到达时自动唤醒
- ✅ **自动检测**: 自动检测 workspace 切换，无需手动触发
- ✅ **灵活匹配**: 支持 name 和 idx 两种标识符类型，匹配顺序为 name 优先，然后 idx
- ✅ **自动重连**: 连接断开时自动重连，确保服务持续运行

## 使用场景示例

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
