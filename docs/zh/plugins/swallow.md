# Swallow 插件

Swallow 插件会在子窗口打开时自动隐藏父窗口。这对于终端启动图片查看器或媒体播放器等场景非常有用，可以让子窗口在布局中替换父窗口的位置。

## 工作原理

当子窗口打开时：

1. **子窗口匹配**：插件检查新窗口是否匹配任何规则的子窗口条件
2. **父窗口发现**：使用两种方法查找父窗口（按优先级）：
   - **基于 PID 的匹配**（默认）：追踪进程树，检查子进程是否由父进程启动
   - **基于规则的匹配**：通过 `app_id`、`title` 或 `pid` 模式匹配父窗口
3. **吞噬操作**：如果找到匹配的父窗口，子窗口会被"吞噬"到父窗口的列位置，有效地替换它

## 配置

使用 `[[swallow]]` 格式配置规则（每个规则一个配置块），使用 `[piri.swallow]` 配置插件全局设置：

```toml
[piri.plugins]
swallow = true

# 插件全局配置
[piri.swallow]
# 启用基于 PID 的父子进程匹配（默认：true）
use_pid_matching = true

# 全局排除规则：匹配这些条件的窗口永远不会被吞噬
[piri.swallow.exclude]
app_id = [".*dialog.*"]
title = [".*error.*"]

# 规则列表（每个规则一个配置块）
# 示例 1: 终端吞噬媒体播放器
[[swallow]]
parent_app_id = [".*terminal.*", ".*alacritty.*", ".*foot.*", ".*ghostty.*"]
child_app_id = [".*mpv.*", ".*imv.*", ".*feh.*"]

# 示例 2: 编辑器吞噬预览窗口
[[swallow]]
parent_app_id = ["code", "nvim-qt"]
child_app_id = [".*preview.*", ".*markdown.*"]
```

### 全局配置参数

在 `[piri.swallow]` 中可以配置以下全局参数：

| 参数 | 类型 | 说明 |
| :--- | :--- | :--- |
| `use_pid_matching` | `bool` | 启用基于 PID 的父子进程匹配（默认：`true`） |
| `exclude` | `SwallowExclude` | 全局排除规则，匹配这些条件的窗口永远不会被吞噬（可选） |

### 规则配置参数

每个规则支持以下可选参数：

| 参数 | 类型 | 说明 |
| :--- | :--- | :--- |
| `parent_app_id` | `Vec<String>` | 匹配父窗口 `app_id` 的正则表达式模式 |
| `parent_title` | `Vec<String>` | 匹配父窗口 `title` 的正则表达式模式 |
| `child_app_id` | `Vec<String>` | 匹配子窗口 `app_id` 的正则表达式模式 |
| `child_title` | `Vec<String>` | 匹配子窗口 `title` 的正则表达式模式 |

### 匹配逻辑

1. **全局排除检查**：首先检查子窗口是否匹配全局 `exclude` 规则，如果匹配则直接跳过，不进行任何吞噬操作

2. **PID 匹配**（当 `use_pid_matching = true`，默认，优先级最高）：
   - 追踪子进程的进程树，查找祖先进程
   - 匹配 PID 是子进程祖先的父窗口
   - 如果指定了父窗口条件（`parent_app_id`、`parent_title`），也会进行检查
   - 如果没有指定父窗口条件，任何祖先窗口都会匹配

3. **基于规则的匹配**（当 PID 匹配失败或禁用时的后备方案）：
   - 使用 `app_id`、`title` 或 `pid` 模式匹配父窗口
   - 仅在 PID 匹配失败或 `use_pid_matching = false` 时使用
   - **父窗口查找机制**：
     - 如果当前聚焦的窗口不是子窗口，则使用当前聚焦的窗口作为候选父窗口
     - 如果当前聚焦的窗口是子窗口本身，则从聚焦窗口队列（维护最近 5 个聚焦的窗口）中查找匹配的父窗口
     - 聚焦窗口队列会在窗口获得焦点时自动更新

4. **排除规则**：排除模式优先 - 如果窗口匹配排除模式，即使匹配包含模式也不会被匹配

5. **模式列表**：当提供多个模式时（例如 `parent_app_id = ["pattern1", "pattern2"]`），如果任一模式匹配，规则就会匹配（OR 逻辑）

### Niri 配置要求

为了确保 Swallow 插件正常工作，需要在 niri 的配置文件中设置以下内容：

```kdl
layout {
    default-column-display "tabbed"
}
```

#### 子窗口处理建议

为了获得更好的体验，建议对可能被子窗口替换的应用程序（如 `mpv`、`imv`、`feh` 等）进行以下配置之一：

> 请参考 [GitHub Issue #2](https://github.com/Asthestarsfalll/piri/issues/2)。

**方法 1：使用 window-rule 设置浮动**

在 niri 配置中为子窗口应用程序设置 `open-floating=true`：

```kdl
window-rule {
    app-id = "mpv"
    open-floating = true
}

window-rule {
    app-id = "imv"
    open-floating = true
}

window-rule {
    app-id = "feh"
    open-floating = true
}
```

**方法 2：使用 autofill 功能**

启用 piri 的 autofill 插件来自动处理这些窗口的布局。

## 示例

### 基于 PID 的匹配示例

![Swallow - 基于 PID 的匹配](./assets/swallow_pid.mp4)

使用默认的 PID 匹配（`use_pid_matching = true`），插件会自动追踪进程树来查找父子关系。

```toml
[piri.swallow]
use_pid_matching = true

[[swallow]]
parent_app_id = [".*ghostty.*"]
child_app_id = [".*mpv.*"]
```

### 基于规则的匹配示例

![Swallow - 基于规则的匹配](./assets/swallow_rule.mp4)

使用 `app_id` 和 `title` 模式来匹配父窗口。

```toml
[piri.swallow]
use_pid_matching = true

[[swallow]]
child_app_id = '.*google-chrome.*'
parent_app_id = '.*ghostty.*'

[[swallow]]
child_app_id = '.*firefox*.'
parent_app_id = '.*ghostty.*'
```

### 基础示例：终端吞噬媒体播放器

```toml
[[swallow]]
parent_app_id = ["ghostty", "alacritty", "foot"]
child_app_id = ["mpv", "imv", "feh"]
```

当你从终端启动 `mpv` 或 `imv` 时，终端窗口将被隐藏，并被媒体播放器替换。


### 全局排除示例

```toml
[piri.swallow]
# 全局排除所有对话框窗口
[piri.swallow.exclude]
app_id = [".*dialog.*", ".*error.*"]

[[swallow]]
parent_app_id = [".*terminal.*"]
child_app_id = [".*mpv.*"]
```

这样所有对话框窗口都不会被吞噬，即使规则匹配也不会执行。

### 禁用 PID 匹配

```toml
[piri.swallow]
use_pid_matching = false

[[swallow]]
parent_app_id = [".*terminal.*"]
child_app_id = [".*mpv.*"]
```

这仅使用基于规则的匹配，不检查进程关系。

### 按标题匹配

```toml
[[swallow]]
parent_title = [".*Terminal.*"]
child_title = [".*Video Player.*"]
```

### 复杂示例：多个模式

```toml
[[swallow]]
parent_app_id = ["ghostty", "alacritty", "foot", "kitty"]
child_app_id = ["mpv", "imv", "feh", "sxiv"]
```

## 默认行为

- 如果未指定规则，插件会启用但不会匹配任何窗口
- 如果未指定，`use_pid_matching` 默认为 `true`
- 如果未指定 `exclude`，则不会进行全局排除
- 如果未指定子窗口条件，规则将匹配任何子窗口并查找父窗口
- 如果未指定父窗口条件（启用 PID 匹配时），任何祖先窗口都会匹配
- 聚焦窗口队列最多维护最近 5 个聚焦的窗口，用于在子窗口聚焦时查找父窗口

## 技术细节

### 进程树追踪

启用 PID 匹配时，插件会：
1. 查找子窗口进程的 PID
2. 向上追踪进程树（最多到 PID 1）以查找祖先 PID
3. 匹配进程 PID 在祖先链中的窗口

### 聚焦窗口队列

插件维护一个最多包含 5 个窗口的聚焦队列，用于跟踪最近聚焦的窗口：
- 当窗口获得焦点时（`WindowFocusTimestampChanged` 事件），窗口 ID 会被添加到队列末尾
- 当新窗口打开时（`WindowOpenedOrChanged` 事件），窗口 ID 也会被添加到队列
- 当子窗口打开且当前聚焦的窗口是子窗口本身时，插件会从队列中从新到旧查找匹配的父窗口
- 队列大小限制为 5，超过时会移除最旧的窗口 ID

### 窗口匹配

插件使用与其他插件相同的窗口匹配机制。详细信息请参阅 [窗口匹配机制文档](../window_matching.md)。

### IPC 调用

插件在执行吞噬操作时会执行以下步骤：
1. 聚焦父窗口
2. 确保子窗口不是浮动窗口（如有需要，转换为平铺窗口）
3. 将子窗口移动到父窗口的工作空间（如果不同）
4. 执行 `ConsumeOrExpelWindowLeft` 操作，将子窗口吞噬到父窗口的列中
5. 聚焦子窗口

所有操作都在一个批处理中执行，以提高性能和原子性。

## 使用场景

- **终端启动媒体播放器**：启动 `mpv`、`imv` 或 `feh` 时隐藏终端
- **编辑器启动预览**：打开预览窗口时隐藏编辑器窗口
- **带启动器窗口的应用程序**：主应用程序启动时隐藏启动器
- **嵌套应用程序工作流**：自动管理父子窗口关系

## 限制

- 浮动窗口无法被吞噬（会先转换为平铺窗口）
- 父窗口和子窗口必须在同一工作空间（插件会自动处理）
- 进程树追踪会一直向上追踪到 PID 1，如果进程树很深可能会影响性能
- PID 匹配要求进程具有父子关系
- 聚焦窗口队列最多维护 5 个窗口，如果父窗口不在最近 5 个聚焦的窗口中，基于规则的匹配可能无法找到父窗口

