# 架构设计

Piri 采用模块化、状态驱动且事件导向的架构，旨在为 Niri 合成器提供高性能、低延迟的扩展能力。

## 核心设计理念

### 1. 状态驱动与 Manager 模式
每个复杂插件（如 Scratchpads, Singleton）都遵循 **State-Manager** 模式：
- **State 结构体**：聚合了静态配置（来自 TOML）和运行时状态（如 `window_id`、可见性、最后聚焦窗口）。
- **Manager 结构体**：负责维护 `HashMap<String, State>`，并提供原子化的操作方法。
- **Plugin 外壳**：实现 `Plugin` trait，负责将 IPC 请求和 Niri 事件映射到 Manager 的具体操作上。

### 2. 资源保障机制 (Resource Assurance)
插件采用“延迟初始化”和“自动恢复”策略。核心方法 `ensure_window_id` 实现了完整的资源保障链路：
1.  **有效性检查**：检查当前绑定的 `window_id` 是否仍然存在。
2.  **自动捕获**：如果 ID 失效，根据 `app_id` 模式搜索当前所有窗口进行捕获。
3.  **自动启动**：如果搜索不到，执行配置的命令并进入“等待-重试”循环直到窗口出现。
4.  **初始化设置**：一旦获取窗口，立即统一执行浮动设置、大小调整和几何定位。

### 3. 智能配置重载 (Smart Config Reload)
Piri 支持无损的热重载：
- **配置合并**：重载时，系统会对比新旧配置。如果某个实例（如 Scratchpad）在旧状态中已有关联的 `window_id`，更新配置时会予以保留。
- **动态保护**：对于通过 IPC 动态添加的资源（标记为 `is_dynamic`），即使它们不在 TOML 配置文件中，重载时也会被智能保留。
- **缓存清理**：重载后自动清理 `WindowMatcher` 的正则表达式缓存，确保新的匹配规则立即生效。

## 核心模块说明

### 插件系统 (`src/plugins/`)
- `mod.rs`: 定义 `Plugin` trait 和统一的事件/IPC 分发总线。
- `scratchpads.rs`: 核心功能，管理隐藏/显示窗口，支持跨工作区和显示器。
- `singleton.rs`: 确保特定应用（如浏览器）全局只有一个实例并支持快速切换。
- `window_rule.rs`: 基于规则的自动化中心，处理窗口自动归位和焦点触发命令。
- `window_utils.rs`: 几何计算中心与窗口匹配引擎，包含正则表达式缓存池。

### 通信与事件中心
- `niri.rs`: 高性能异步 IPC 客户端，封装了所有 Niri 动作。
- `daemon.rs`: 整个系统的神经中枢，协调事件流分发、信号处理和插件生命周期。
- `ipc.rs`: 基于 Unix Socket 的内部命令协议。

## 性能与健壮性设计

### 统一去重机制
针对 Niri 可能发送的密集重复事件（如窗口创建过程中的多次状态变更），Piri 在插件层实现了基于 **窗口 ID + 时间戳** 的全局去重引擎（冷却时间通常为 200ms - 500ms），确保 `focus_command` 等副作用只触发一次。

### 几何计算抽象
所有关于屏幕边缘（FromTop, FromLeft 等）、边距、百分比大小的数学计算都收拢在 `window_utils.rs` 中。通过一次性获取输出（Output）上下文，减少了与合成器之间的往返延迟。

### 正则表达式缓存
`WindowMatcherCache` 使用 `Arc<Mutex<HashMap<String, Regex>>>`。在 `window_rule` 等需要频繁匹配 app_id 的场景下，避免了重复编译正则表达式带来的 CPU 开销。
