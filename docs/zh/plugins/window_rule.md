# Window Rule 插件

Window Rule 插件根据窗口的 `app_id` 或 `title` 使用正则表达式匹配，自动将窗口移动到指定的 workspace。

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
focus_command = "notify-send 'Focusing on Chrome'"

# 同时指定 app_id 和 title（任一匹配即可）
[[window_rule]]
app_id = "code"
title = ".*VS Code.*"
open_on_workspace = "dev"

# 只有 focus_command，不移动窗口
[[window_rule]]
title = ".*Chrome.*"
focus_command = "notify-send 'Chrome focused'"

# focus_command 仅对规则全局执行一次（规则级别，非窗口级别）
[[window_rule]]
app_id = "firefox"
focus_command = "notify-send 'Firefox focused'"
focus_command_once = true

# 正则表达式示例：匹配以 "firefox" 开头的 app_id
[[window_rule]]
app_id = "^firefox"
open_on_workspace = "2"

# 匹配精确的 app_id
[[window_rule]]
app_id = "^code$"
open_on_workspace = "dev"

# app_id 作为列表（任意一个匹配即可）
[[window_rule]]
app_id = ["code", "code-oss", "codium"]
open_on_workspace = "dev"

# title 作为列表（任意一个匹配即可）
[[window_rule]]
title = [".*Chrome.*", ".*Chromium.*", ".*Google Chrome.*"]
open_on_workspace = "browser"
```

## 配置字段

- **`app_id`** (可选): 用于匹配窗口 `app_id` 的正则表达式。可以是单个字符串或字符串列表。如果提供列表，任意一个模式匹配即可触发规则。
- **`title`** (可选): 用于匹配窗口标题的正则表达式。可以是单个字符串或字符串列表。如果提供列表，任意一个模式匹配即可触发规则。
- **`open_on_workspace`** (可选): 目标 workspace 标识符（名称或索引，详见下方说明）
- **`focus_command`** (可选): 当窗口获得焦点时执行的命令
- **`focus_command_once`** (可选，默认: `false`): 如果设置为 `true`，`focus_command` 将仅对该规则全局执行一次，无论有多少窗口匹配该规则。更多详情请参阅 [issue #1](https://github.com/Asthestarsfalll/piri/issues/1)。

**注意**: 
- 至少需要指定 `app_id` 或 `title` 中的一个
- 至少需要指定 `open_on_workspace` 或 `focus_command` 中的一个
- 如果同时指定 `app_id` 和 `title`，则任一匹配即可（OR 逻辑）
- `app_id` 和 `title` 可以是单个字符串或字符串列表。当提供列表时，列表中任意一个模式匹配即可触发规则

> **窗口匹配**: 关于窗口匹配机制的详细说明，请参阅 [窗口匹配机制文档](../window_matching.md) 和 [插件系统通用配置说明](plugins.md#通用配置说明)

> **Workspace 标识符**: 关于 workspace 标识符（name/idx）的详细说明，请参阅 [插件系统通用配置说明](plugins.md#workspace-标识符)

## 工作原理

插件监听 `WindowOpenedOrChanged` 事件：

1. 使用配置的正则表达式匹配窗口的 `app_id` 或 `title`
2. 如果匹配，自动将窗口移动到指定的 workspace
3. 规则按配置顺序检查，**第一个匹配的规则会被应用**

## 特性

- ✅ **正则表达式**: 支持完整的正则表达式语法
- ✅ **灵活匹配**: 支持 `app_id` 或 `title`，或两者组合（OR 逻辑）
- ✅ **列表支持**: `app_id` 和 `title` 可以是模式列表，任意一个匹配即可触发规则
- ✅ **正则缓存**: 编译后的正则表达式会被缓存，提高性能
- ✅ **配置热更新**: 支持配置更新而不重启守护进程

## focus_command_once 功能

`focus_command_once` 选项允许您对每个规则仅执行一次 `focus_command`，而不是对每个窗口都执行。请参阅 [issue #1](https://github.com/Asthestarsfalll/piri/issues/1)。

## 注意事项

1. **规则顺序很重要**: 第一个匹配的规则会被应用，后续规则不会检查
2. **Workspace 不存在**: 如果指定的 workspace 不存在，会记录警告但不会报错
3. **正则表达式性能**: 建议使用简单明确的模式以获得更好性能
4. **focus_command_once 是规则级别的**: 跟踪是按规则进行的，而不是按窗口进行的。一旦规则的 `focus_command` 已执行（当 `focus_command_once = true` 时），它将不会对匹配该规则的任何后续窗口再次执行
