# 窗口匹配机制

Piri 使用统一的窗口匹配机制，支持通过正则表达式匹配窗口的 `app_id` 和 `title`。多个插件（如 `window_rule`、`singleton`、`scratchpads`）都使用此机制来查找和匹配窗口。

## 支持的匹配方式

### 1. 正则表达式匹配

所有窗口匹配都基于 **正则表达式（Regex）**，支持完整的正则表达式语法。

### 2. 匹配字段

- **`app_id`**: 窗口的应用 ID（可选）
- **`title`**: 窗口标题（可选）

**注意**: 至少需要指定 `app_id` 或 `title` 中的一个。如果两者都指定，则任一匹配即可（OR 逻辑）。

## 匹配逻辑

1. **单一字段匹配**: 如果只指定了 `app_id` 或 `title`，则必须匹配该字段
2. **多字段匹配**: 如果同时指定了 `app_id` 和 `title`，则任一匹配即可（OR 逻辑）
3. **正则表达式**: 使用 Rust 的 `regex` crate，支持完整的正则表达式语法

## 使用示例

### 基本匹配

```toml
# 精确匹配 app_id
app_id = "code"

# 匹配包含特定字符串的 app_id
app_id = ".*chrome.*"

# 匹配以特定字符串开头的 app_id
app_id = "^firefox"

# 匹配精确的 app_id（使用锚点）
app_id = "^code$"
```

### 标题匹配

```toml
# 匹配包含 "Chrome" 的标题
title = ".*Chrome.*"

# 匹配以 "VS Code" 开头的标题
title = "^VS Code"

# 匹配包含数字的标题
title = ".*\\d+.*"
```

### 组合匹配

```toml
# app_id 或 title 任一匹配即可
app_id = "code"
title = ".*VS Code.*"
```

## 在插件中的使用

### Window Rule 插件

```toml
[[window_rule]]
app_id = ".*firefox.*"
open_on_workspace = "2"

[[window_rule]]
title = ".*Chrome.*"
open_on_workspace = "3"
focus_command = "notify-send 'Focusing on Chrome'"
```

### Singleton 插件

```toml
[singleton.browser]
command = "google-chrome-stable"
app_id = "google-chrome"  # 使用正则表达式匹配
```

### Scratchpads 插件

```toml
[scratchpads.term]
direction = "fromRight"
command = "ghostty"
app_id = "float\\.dropterm"  # 使用正则表达式匹配，注意转义点号
```

## 正则表达式语法参考

### 常用模式

| 模式 | 说明 | 示例 |
|------|------|------|
| `.` | 匹配任意字符 | `"c.ode"` 匹配 `"code"`, `"cade"` |
| `.*` | 匹配任意字符（零个或多个） | `".*chrome.*"` 匹配包含 `chrome` 的字符串 |
| `^` | 匹配字符串开头 | `"^firefox"` 匹配以 `firefox` 开头的字符串 |
| `$` | 匹配字符串结尾 | `"code$"` 匹配以 `code` 结尾的字符串 |
| `[abc]` | 匹配字符集中的任意字符 | `"[abc]ode"` 匹配 `"aode"`, `"bode"`, `"code"` |
| `[0-9]` | 匹配数字 | `"[0-9]+"` 匹配一个或多个数字 |
| `\d` | 匹配数字（等价于 `[0-9]`） | `"\d+"` 匹配一个或多个数字 |
| `\w` | 匹配单词字符（字母、数字、下划线） | `"\w+"` 匹配单词 |
| `+` | 一个或多个 | `"[0-9]+"` 匹配一个或多个数字 |
| `*` | 零个或多个 | `".*"` 匹配任意字符串 |
| `?` | 零个或一个 | `"colou?r"` 匹配 `"color"` 或 `"colour"` |
| `\|` | 或 | `"firefox\|chrome"` 匹配 `"firefox"` 或 `"chrome"` |

### 转义特殊字符

如果需要在模式中匹配特殊字符（如 `.`, `*`, `+`, `?`, `[`, `]`, `(`, `)`, `{`, `}`, `^`, `$`, `|`, `\`），需要使用反斜杠转义：

```toml
# 匹配包含点号的 app_id
app_id = "float\\.dropterm"

# 匹配包含括号的标题
title = ".*\\(.*\\).*"
```

## 性能优化

1. **正则表达式缓存**: 编译后的正则表达式会被缓存，避免重复编译
2. **简单模式优先**: 使用简单明确的模式可以获得更好的性能
3. **避免过度复杂**: 过于复杂的正则表达式可能影响性能

## 最佳实践

1. **精确匹配**: 如果知道确切的 `app_id`，使用 `^app_id$` 进行精确匹配
2. **部分匹配**: 使用 `.*pattern.*` 进行部分匹配
3. **转义特殊字符**: 如果 `app_id` 或 `title` 包含正则表达式特殊字符，记得转义
4. **测试模式**: 在配置前，可以使用在线正则表达式测试工具验证模式是否正确

## 调试技巧

如果窗口匹配不工作，可以：

1. **检查日志**: 查看 piri 的日志输出，了解匹配过程
2. **验证 app_id/title**: 使用 `niri-ipc` 工具查看窗口的实际 `app_id` 和 `title`
3. **测试正则表达式**: 使用在线工具测试正则表达式是否正确
4. **简化模式**: 先使用简单的模式（如精确匹配）验证基本功能，再逐步复杂化

## 示例配置

### 匹配多个浏览器

```toml
[[window_rule]]
app_id = ".*(firefox|chrome|chromium).*"
open_on_workspace = "browser"
```

### 匹配开发工具

```toml
[[window_rule]]
app_id = ".*(code|vscode|idea).*"
open_on_workspace = "dev"
```

### 匹配终端

```toml
[[window_rule]]
app_id = ".*(term|terminal|ghostty|alacritty).*"
open_on_workspace = "1"
```

### 使用标题匹配特定窗口

```toml
[[window_rule]]
title = ".*GitHub.*"
open_on_workspace = "dev"
```

