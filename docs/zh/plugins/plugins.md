# 插件系统

Piri 支持插件系统，允许你扩展功能。插件在守护进程模式下自动运行。

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