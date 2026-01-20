# Scratchpads 插件

Scratchpads 允许你快速显示和隐藏常用应用程序的窗口，支持跨 workspace 和 monitor。

## 演示视频

![Scratchpads 演示视频](../assets/scratchpads.mp4)

## 配置

使用 `[scratchpads.{name}]` 格式配置 scratchpad：

```toml
[piri.plugins]
scratchpads = true

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

[scratchpads.preview]
direction = "fromRight"
command = "imv"
app_id = "imv"
size = "60% 80%"
margin = 50
swallow_to_focus = true  # 显示时自动吞入当前聚焦的窗口
```

### 配置参数

- `direction` (必需): 窗口出现的方向
  - `fromTop`: 从顶部滑入
  - `fromBottom`: 从底部滑入
  - `fromLeft`: 从左侧滑入
  - `fromRight`: 从右侧滑入
- `command` (必需): 启动应用程序的完整命令，可包含环境变量和参数
- `app_id` (必需): 用于匹配窗口的应用 ID（支持正则表达式，详见下方说明）
- `size` (必需): 窗口大小，格式为 `"width% height%"`
- `margin` (必需): 距离屏幕边缘的边距（像素）
- `swallow_to_focus` (可选): 如果为 `true`，显示时将 scratchpad 窗口吞入当前聚焦的窗口。隐藏时会先让窗口浮动，再执行正常的隐藏逻辑。默认为 `false`

> **窗口匹配**: `app_id` 使用正则表达式匹配。关于窗口匹配机制的详细说明（包括特殊字符转义），请参阅 [窗口匹配机制文档](../window_matching.md) 和 [插件系统通用配置说明](plugins.md#通用配置说明)

## 使用方法

### 切换显示/隐藏

```bash
piri scratchpads {name} toggle

# 示例
piri scratchpads term toggle
piri scratchpads calc toggle
```

### 动态添加当前窗口

将当前聚焦的窗口快速添加为 scratchpad：

```bash
piri scratchpads {name} add {direction} [--swallow-to-focus]

# 示例
piri scratchpads mypad add fromRight
piri scratchpads mypad add fromRight --swallow-to-focus  # 启用 swallow 功能
```

动态添加的 scratchpad 会使用 `[piri.scratchpad]` 节中设置的默认大小和边距。

> **提示**:
> - 动态添加的窗口仅在第一次注册时调整大小和位置。之后你可以手动调整该窗口的大小和位置（边距），插件在后续切换显示/隐藏时会保持你手动调整后的状态，不再强制重置。
> - 如果 scratchpad 已存在，`add` 命令会自动执行 toggle 操作（显示/隐藏切换），而不是报错。

### 全局配置说明

在 `[piri.scratchpad]` 节下可以设置一些全局默认值：

| 参数 | 说明 | 默认值 |
| :--- | :--- | :--- |
| `default_size` | 动态添加时的默认大小 | `"75% 60%"` |
| `default_margin` | 动态添加时的默认边距 | `50` |
| `move_to_workspace` | (可选) 窗口隐藏后移动到的指定工作区 | `无` |

> **move_to_workspace**: 如果设置了此参数，当 scratchpad 窗口被隐藏时，它会被自动移动到该工作区。这可以防止隐藏的窗口留在当前工作区的堆栈中（虽然它是不可见的）。显示时，窗口依然会自动移动到当前活跃的工作区。

## 工作原理

1. **首次启动**: 如果窗口不存在，启动配置中指定的应用程序
2. **窗口注册**: 找到窗口后，设置为浮动模式并移动到屏幕外
3. **显示**: 将窗口移动到当前聚焦的输出和工作区，按配置的方向和大小定位，并聚焦窗口
4. **隐藏**: 将窗口移动到屏幕外，智能恢复之前的焦点

**跨 workspace 和 monitor**: 无论 scratchpad 窗口原本在哪个工作区或显示器上，都会自动移动到当前聚焦的位置。

## 特性

- ✅ **跨 workspace**: 从任何工作区快速访问
- ✅ **跨 monitor**: 自动出现在当前聚焦的显示器上
- ✅ **智能焦点管理**: 显示时自动聚焦，隐藏时恢复之前的焦点
- ✅ **灵活配置**: 自定义窗口大小、位置和动画方向
- ✅ **动态添加**: 快速添加当前窗口为 scratchpad
- ✅ **Swallow 集成**: 支持将 scratchpad 窗口吞入当前聚焦的窗口（`swallow_to_focus` 选项）
