# Empty 插件

Empty 插件在切换到空 workspace 时自动执行配置的命令，用于自动化工作流程。

> **参考**: 此功能类似于 [Hyprland 的 `on-created-empty` workspace rule](https://wiki.hypr.land/Configuring/Workspace-Rules/#rules)。

## 配置

使用 `[empty.{workspace}]` 格式配置 workspace 规则：

```toml
[piri.plugins]
empty = true

# 当切换到 workspace 1 且为空时，执行命令
[empty.1]
command = "alacritty"

# 使用 workspace 名称
[empty.main]
command = "firefox"

# 在空 workspace 中自动启动编辑器
[empty.dev]
command = "code"
```

## Workspace 标识符

支持两种标识符类型：

- **name**: Workspace 名称，如 `"main"`, `"work"`
- **idx**: Workspace 索引（1-based），如 `"1"`, `"2"`

**匹配顺序**：name 优先，然后 idx。插件自动识别类型并支持跨类型匹配。

## 工作原理

插件监听 `WorkspaceActivated` 事件，当 workspace 切换时：

1. 检查当前 workspace 是否为空（通过 `active_window_id` 字段）
2. 如果为空且匹配配置规则，立即执行命令

## 特性

- ✅ **事件驱动**: 实时监听 workspace 切换
- ✅ **灵活匹配**: 支持 name 和 idx 两种标识符
- ✅ **自动检测**: 无需手动触发

## 使用场景

- 在空 workspace 中自动启动常用应用（终端、浏览器、编辑器等）
- 为不同 workspace 设置不同的默认应用
- 自动化工作流程
