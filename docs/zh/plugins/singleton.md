# Singleton 插件

Singleton 插件管理单例窗口——只应该有一个实例的窗口。切换时如果窗口已存在则聚焦，否则启动应用程序。

## 配置

使用 `[singleton.{name}]` 格式配置单例：

```toml
[piri.plugins]
singleton = true

[singleton.browser]
command = "google-chrome-stable"

[singleton.term]
command = "GTK_IM_MODULE=wayland ghostty --class=singleton.term"
app_id = "singleton.term"
```

### 配置参数

- `command` (必需): 启动应用程序的完整命令，可包含环境变量和参数
- `app_id` (可选): 用于匹配窗口的应用 ID（支持正则表达式，详见下方说明）。如不指定，插件会自动从命令中提取（取可执行文件名）

> **窗口匹配**: `app_id` 使用正则表达式匹配。关于窗口匹配机制的详细说明（包括特殊字符转义），请参阅 [窗口匹配机制文档](../window_matching.md) 和 [插件系统通用配置说明](plugins.md#通用配置说明)

## 使用方法

```bash
# 切换单例（如果存在则聚焦，否则启动）
piri singleton {name} toggle

# 示例
piri singleton browser toggle
piri singleton term toggle
```

## 工作原理

1. **首次切换**: 检查是否存在匹配的窗口，如果找到则聚焦并注册，否则启动应用程序并等待窗口出现
2. **后续切换**: 如果注册的窗口仍存在则聚焦，否则搜索匹配的窗口或重新启动
3. **窗口匹配**: 使用配置的 `app_id` 或从命令中提取的 `app_id` 进行匹配

## 特性

- ✅ **智能检测**: 自动检测现有窗口，避免重复启动
- ✅ **自动提取**: 未指定 `app_id` 时自动从命令提取
- ✅ **窗口注册**: 通过 ID 跟踪单例窗口，快速查找
- ✅ **健壮匹配**: 即使窗口不是由插件启动的也能匹配

## 使用场景

- 浏览器、终端等通常只需要一个实例的应用
- 快速访问常用应用并保证单实例
- 防止资源密集型应用的多个实例

## 注意事项

1. **窗口匹配**: 确保应用程序设置了正确的 `app_id` 属性，或在配置中明确指定
2. **app_id 提取**: 从命令的第一个单词提取可执行文件名（去除路径），如 `/usr/bin/google-chrome-stable` → `google-chrome-stable`
3. **超时**: 启动应用后最多等待 5 秒让窗口出现，超时后不会报错但不会聚焦窗口
