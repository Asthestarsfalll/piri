# 架构设计

## 项目结构

项目采用模块化设计，便于扩展：

### 核心模块

- `main.rs`: 主入口，处理 CLI 命令解析和路由
- `lib.rs`: 库入口，导出所有公共模块
- `config.rs`: 配置管理模块，负责加载和解析 TOML 配置文件
- `niri.rs`: Niri IPC 封装模块，提供与 niri 合成器通信的接口
- `commands.rs`: 命令处理系统，包含 `CommandHandler` 用于处理 IPC 请求
- `daemon.rs`: 守护进程管理，负责守护进程的启动、事件循环和生命周期管理
- `ipc.rs`: 进程间通信模块，实现客户端与守护进程之间的 Unix socket 通信
- `utils.rs`: 工具函数模块

### 插件系统

- `plugins/mod.rs`: 插件系统核心，包含 `Plugin` trait 和 `PluginManager`
- `plugins/scratchpads.rs`: Scratchpads 插件实现
- `plugins/empty.rs`: Empty 插件实现
- `plugins/window_rule.rs`: Window Rule 插件实现
- `plugins/autofill.rs`: Autofill 插件实现
- `plugins/singleton.rs`: Singleton 插件实现
- `plugins/window_order.rs`: Window Order 插件实现
- `plugins/window_utils.rs`: 窗口匹配和工具函数

## 工作原理

### 守护进程架构

Piri 使用守护进程架构来提供持续的服务。守护进程在后台运行，监听 niri 事件并执行相应的操作。

**守护进程启动流程**：
1. `main.rs` 解析 CLI 命令，如果是 `daemon` 命令则启动守护进程
2. `daemon.rs` 创建 `CommandHandler` 和 `PluginManager`
3. 初始化所有启用的插件
4. 启动统一的事件监听器
5. 启动 IPC 服务器，监听客户端连接
6. 进入主事件循环，处理 IPC 请求和 niri 事件

### IPC 通信

客户端通过 Unix socket 与守护进程通信，发送命令和接收响应。这允许在不重启守护进程的情况下执行操作。

**IPC 通信流程**：
1. 客户端通过 `IpcClient` 连接到守护进程的 Unix socket
2. 发送序列化的 `IpcRequest` 请求
3. 守护进程的 `IpcServer` 接收请求
4. `CommandHandler` 处理请求，可能通过插件系统路由
5. 返回 `IpcResponse` 响应给客户端

### 插件系统

插件系统允许扩展功能。每个插件实现 `Plugin` trait，并在守护进程启动时自动加载。

**插件生命周期**：
1. **初始化**：守护进程启动时，`PluginManager` 根据配置初始化所有启用的插件
2. **事件处理**：插件通过 `handle_event` 方法接收 niri 事件
3. **IPC 请求处理**：插件可以通过 `handle_ipc_request` 方法处理客户端请求
4. **配置更新**：支持热重载配置，插件通过 `update_config` 方法更新配置
5. **关闭**：守护进程关闭时，调用插件的 `shutdown` 方法进行清理

#### 统一事件分发机制

Piri 使用统一的事件分发机制来优化性能和资源使用：

- **单一 Socket 连接**：所有基于事件的插件共享一个 niri 事件流 socket 连接，而不是每个插件单独连接
- **高效事件分发**：事件只读取一次，然后通过 `mpsc::UnboundedChannel` 分发给所有需要处理事件的插件
- **智能事件过滤**：插件可以声明它们感兴趣的事件类型，只有相关的插件才会收到事件，避免不必要的处理
- **性能优化**：减少了 socket 连接数量，降低了系统资源消耗
- **易于扩展**：新的事件驱动插件只需实现 `handle_event` 和 `is_interested_in_event` 方法，无需管理自己的事件监听循环

**事件分发流程**：
1. `PluginManager` 启动统一的事件监听器任务
2. 事件监听器通过 `NiriIpc::create_event_stream_socket` 创建事件流连接
3. 事件通过 channel 发送到守护进程主循环
4. 主循环调用 `PluginManager::distribute_event` 分发事件
5. 只有感兴趣的插件会收到事件通知

这种设计确保了：
- 更好的资源利用（只有一个 socket 连接）
- 更高的事件处理效率（事件只读取一次，只分发给感兴趣的插件）
- 更简洁的插件代码（插件只需关注事件处理逻辑）
- 更好的性能（避免不关心事件的插件被调用）

### 配置热重载

Piri 支持配置文件热重载，无需重启守护进程：

- **文件监听**：使用 `notify` crate 监听配置文件变化
- **自动重载**：配置文件修改后自动触发重载
- **插件更新**：重载配置后，所有插件通过 `update_config` 方法更新配置
- **错误处理**：重载失败时发送通知，但不影响守护进程运行

