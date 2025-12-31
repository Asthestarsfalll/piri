# 插件系统

Piri 支持插件系统，允许你扩展功能。插件在守护进程模式下自动运行。

## 可用插件

### [Scratchpads](scratchpads.md)

强大的窗口管理功能，允许你快速显示和隐藏常用应用程序的窗口。支持跨工作区和跨显示器功能。

**主要特性**：
- 快速显示/隐藏常用应用程序
- 跨工作区和跨显示器支持
- 可自定义出现方向和大小

### [Empty 插件](empty.md)

在切换到特定空工作区时执行命令。用于自动化工作流程，例如在空工作区中自动启动应用程序。

**主要特性**：
- 在空工作区上自动执行命令
- 基于工作区的配置
- 类似于 Hyprland 的 `on-created-empty` 工作区规则

### [Window Rule 插件](window_rule.md)

根据窗口的 `app_id` 或 `title` 自动将窗口移动到指定的工作区。用于自动化窗口管理和组织应用程序。

**主要特性**：
- 自动将窗口分配到工作区
- 通过 `app_id` 或 `title` 匹配（支持正则表达式）
- 类似于 Hyprland 的窗口规则

### [Autofill 插件](autofill.md)

在窗口关闭或布局改变时，自动将最后一列窗口对齐到最右侧位置。有助于保持整洁有序的窗口布局。

**主要特性**：
- 纯事件驱动，实时对齐
- 零配置，开箱即用
- 聚焦保持 - 保持用户聚焦的窗口
- 工作区感知操作

## 插件控制

你可以在配置文件中控制哪些插件启用或禁用：

```toml
[piri.plugins]
scratchpads = true
empty = true
window_rule = true
autofill = true
```

**默认行为**：
- 如果未明确指定，插件默认**禁用**（`false`）
- 必须显式设置 `scratchpads = true`、`empty = true`、`window_rule = true` 或 `autofill = true` 来启用插件
- `window_rule` 插件例外：如果配置了窗口规则，默认启用（除非显式设置为 `false`）