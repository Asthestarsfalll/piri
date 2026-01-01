# Window Rule 插件

Window Rule 插件根据窗口的 `app_id` 或 `title` 使用正则表达式匹配，自动将窗口移动到指定的 workspace。

> **参考**: 此功能类似于 [Hyprland 的 window rules](https://wiki.hypr.land/Configuring/Window-Rules/)。

## 配置

使用 `[[window_rule]]` 格式配置窗口规则：

```toml
[piri.plugins]
window_rule = true

# 根据 app_id 匹配
[[window_rule]]
app_id = "ghostty"
open_on_workspace = "1"

# 根据 title 匹配
[[window_rule]]
title = ".*Chrome.*"
open_on_workspace = "browser"

# 同时指定 app_id 和 title（任一匹配即可）
[[window_rule]]
app_id = "code"
title = ".*VS Code.*"
open_on_workspace = "dev"

# 正则表达式示例：匹配以 "firefox" 开头的 app_id
[[window_rule]]
app_id = "^firefox"
open_on_workspace = "2"

# 匹配精确的 app_id
[[window_rule]]
app_id = "^code$"
open_on_workspace = "dev"
```

## 配置字段

- **`app_id`** (可选): 用于匹配窗口 `app_id` 的正则表达式
- **`title`** (可选): 用于匹配窗口标题的正则表达式
- **`open_on_workspace`** (必需): 目标 workspace 标识符（名称或索引）

**注意**: 至少需要指定 `app_id` 或 `title` 中的一个。如果两者都指定，则任一匹配即可（OR 逻辑）。

## Workspace 标识符

支持两种类型：

- **name**: Workspace 名称，如 `"main"`, `"browser"`
- **idx**: Workspace 索引（1-based），如 `"1"`, `"2"`

**匹配顺序**：name 优先，然后 idx。

## 工作原理

插件监听 `WindowOpenedOrChanged` 事件：

1. 使用配置的正则表达式匹配窗口的 `app_id` 或 `title`
2. 如果匹配，自动将窗口移动到指定的 workspace
3. 规则按配置顺序检查，**第一个匹配的规则会被应用**

## 特性

- ✅ **正则表达式**: 支持完整的正则表达式语法
- ✅ **灵活匹配**: 支持 `app_id` 或 `title`，或两者组合（OR 逻辑）
- ✅ **正则缓存**: 编译后的正则表达式会被缓存，提高性能
- ✅ **配置热更新**: 支持配置更新而不重启守护进程

## 注意事项

1. **规则顺序很重要**: 第一个匹配的规则会被应用，后续规则不会检查
2. **Workspace 不存在**: 如果指定的 workspace 不存在，会记录警告但不会报错
3. **正则表达式性能**: 建议使用简单明确的模式以获得更好性能
