# Scratchpads 详细说明

Scratchpads 是一个强大的窗口管理功能（通过插件系统实现），允许你快速显示和隐藏常用应用程序的窗口。它支持跨 workspace 和 monitor，无论你在哪个工作区或显示器上，都能快速访问你的 scratchpad 窗口。

## 演示视频

![Scratchpads 演示视频](../assets/scratchpads.mp4)

## 配置

在配置文件中添加 `[scratchpads.{name}]` 节来配置 scratchpad。每个 scratchpad 需要一个唯一的名称。

### 配置示例

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

### 配置参数说明

- `direction` (必需): 窗口出现的方向
  - `fromTop`: 从顶部滑入
  - `fromBottom`: 从底部滑入
  - `fromLeft`: 从左侧滑入
  - `fromRight`: 从右侧滑入

- `command` (必需): 启动应用程序的完整命令字符串，可以包含环境变量和参数

- `app_id` (必需): 用于匹配窗口的应用 ID。这是 niri 识别窗口的关键标识符

- `size` (必需): 窗口大小，格式为 `"width% height%"`，例如 `"40% 60%"` 表示宽度为屏幕的 40%，高度为屏幕的 60%

- `margin` (必需): 距离屏幕边缘的边距（像素）

## 使用方法

### 切换 Scratchpad 显示/隐藏

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

### 动态添加当前窗口为 Scratchpad

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

## 工作原理

1. **首次启动**: 当执行 `piri scratchpads {name} toggle` 时，如果窗口不存在，会启动配置中指定的应用程序

2. **窗口注册**: 找到窗口后，将其设置为浮动模式，并移动到屏幕外

3. **显示/隐藏**: 
   - **显示**: 
     - 记录当前聚焦的窗口
     - 将窗口移动到当前聚焦的输出和工作区，并根据配置的方向和大小定位
     - 将焦点转移到 scratchpad 窗口
     - **支持跨 workspace 和 monitor**: 无论 scratchpad 窗口原本在哪个工作区或显示器上，都会自动移动到当前聚焦的位置
   - **隐藏**: 
     - 直接将窗口移动到屏幕外（无需先聚焦 scratchpad）
     - 恢复焦点：
       - 如果之前聚焦的窗口在当前工作区，则聚焦到它
       - 如果不在当前工作区，且当前工作区存在其他窗口，则聚焦到最中间的窗口

## 特性

- ✅ **跨 workspace 支持**: 无论你在哪个工作区，都能快速访问 scratchpad
- ✅ **跨 monitor 支持**: 支持多显示器环境，scratchpad 会自动出现在当前聚焦的显示器上
- ✅ **智能焦点管理**: 显示时自动聚焦，隐藏时智能恢复之前的焦点
- ✅ **灵活配置**: 支持自定义窗口大小、位置和动画方向
- ✅ **动态添加**: 可以将当前聚焦的窗口快速添加为 scratchpad，无需编辑配置文件

