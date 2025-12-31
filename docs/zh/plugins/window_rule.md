# Window Rule 插件

Window Rule 插件用于根据窗口的 `app_id` 或 `title` 自动将窗口移动到指定的 workspace。这对于自动化窗口管理非常有用，例如将特定应用程序自动分配到特定 workspace。

> **参考**: 此功能类似于 [Hyprland 的 window rules](https://wiki.hypr.land/Configuring/Window-Rules/)。

## 配置

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

## 配置字段

每个窗口规则包含以下字段：

- **`app_id`** (可选): 用于匹配窗口 `app_id` 的正则表达式模式
- **`title`** (可选): 用于匹配窗口标题的正则表达式模式
- **`open_on_workspace`** (必需): 目标 workspace 标识符（名称或索引）

**注意**: 至少需要指定 `app_id` 或 `title` 中的一个。如果两者都指定，则任一匹配即可（OR 逻辑）。

## Workspace 标识符

Window Rule 插件支持两种 workspace 标识符类型：

1. **name (名称)**: Workspace 的名称（字符串），例如 `"main"`, `"work"`, `"browser"`
2. **idx (索引)**: Workspace 的索引号，通常是 1-based，例如 `"1"`, `"2"`, `"3"`

**匹配顺序**：插件会按照 **name 优先，然后 idx** 的顺序进行匹配。这与 Empty 插件保持一致。

插件会自动识别标识符类型，并支持跨类型匹配（例如，如果配置中使用 name，插件会先尝试 name 匹配，再尝试 idx 匹配）。

## 工作原理

Window Rule 插件使用**纯事件驱动**的方式实时监听 niri 合成器的事件：

1. **事件流监听**: 插件通过 niri IPC 的 `EventStream` 实时监听窗口事件
2. **实时响应**: 当收到 `WindowOpenedOrChanged` 事件时（表示窗口已创建或变更），立即检查窗口信息
3. **正则匹配**: 使用配置的正则表达式模式匹配窗口的 `app_id` 或 `title`
4. **自动移动**: 如果窗口匹配规则，自动将窗口移动到指定的 workspace

## 特性

- ✅ **纯事件驱动**: 使用 niri 事件流实时监听，窗口创建时自动处理
- ✅ **正则表达式支持**: 支持强大的正则表达式模式匹配
- ✅ **灵活匹配**: 支持按 `app_id` 或 `title` 匹配，或两者组合（OR 逻辑）
- ✅ **正则缓存**: 编译后的正则表达式会被缓存，提高性能
- ✅ **Workspace 匹配**: 支持 name 和 idx 两种标识符类型，匹配顺序为 name 优先，然后 idx
- ✅ **自动重连**: 连接断开时自动重连，确保服务持续运行
- ✅ **配置热更新**: 支持配置更新而不重启守护进程

## 匹配逻辑

1. **规则顺序**: 规则按照配置文件中出现的顺序依次检查，**第一个匹配的规则会被应用**
2. **匹配条件**: 
   - 如果只指定了 `app_id`，则只匹配 `app_id`
   - 如果只指定了 `title`，则只匹配 `title`
   - 如果两者都指定，则任一匹配即可（OR 逻辑）
3. **正则表达式**: 使用 Rust 的 `regex` crate，支持完整的正则表达式语法

## 使用场景示例

```toml
# 将所有 Firefox 窗口移动到 workspace 2
[[window_rule]]
app_id = ".*firefox.*"
open_on_workspace = "2"

# 将所有 Chrome 窗口移动到 workspace 3
[[window_rule]]
title = ".*Chrome.*"
open_on_workspace = "3"

# 将 VS Code 或 Code 窗口移动到开发 workspace
[[window_rule]]
app_id = "code"
title = ".*VS Code.*"
open_on_workspace = "dev"

# 将特定终端移动到 workspace 1
[[window_rule]]
app_id = "ghostty"
open_on_workspace = "1"

# 将标题包含 "kitty" 的窗口移动到 browser workspace
[[window_rule]]
title = "^kitty"
open_on_workspace = "browser"
```

## 正则表达式示例

```toml
# 匹配以 "firefox" 开头的 app_id
[[window_rule]]
app_id = "^firefox"
open_on_workspace = "2"

# 匹配包含 "chrome" 的标题（不区分大小写需要特殊处理）
[[window_rule]]
title = ".*[Cc]hrome.*"
open_on_workspace = "3"

# 匹配精确的 app_id
[[window_rule]]
app_id = "^code$"
open_on_workspace = "dev"

# 匹配多个可能的 app_id（使用 | 操作符）
[[window_rule]]
app_id = "^(code|vscode|Code)$"
open_on_workspace = "dev"
```

## 注意事项

1. **规则顺序很重要**: 第一个匹配的规则会被应用，后续规则不会检查
2. **正则表达式性能**: 复杂的正则表达式可能影响性能，建议使用简单明确的模式
3. **窗口创建时机**: 插件在窗口创建或变更时立即处理，确保窗口被正确移动到目标 workspace
4. **Workspace 不存在**: 如果指定的 workspace 不存在，会记录警告但不会报错
