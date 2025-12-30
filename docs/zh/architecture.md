# 架构设计

## 项目结构

项目采用模块化设计，便于扩展：

- `config.rs`: 配置管理模块
- `niri.rs`: Niri IPC 封装模块
- `commands.rs`: 命令处理系统
- `scratchpads.rs`: Scratchpads 功能实现
- `plugins/`: 插件系统目录
  - `mod.rs`: 插件系统核心
  - `empty.rs`: Empty 插件实现
  - `scratchpads.rs`: Scratchpads 插件实现
- `daemon.rs`: 守护进程管理
- `ipc.rs`: 进程间通信（用于客户端与守护进程通信）

## 工作原理

### 守护进程架构

Piri 使用守护进程架构来提供持续的服务。守护进程在后台运行，监听 niri 事件并执行相应的操作。

### IPC 通信

客户端通过 IPC 与守护进程通信，发送命令和接收响应。这允许在不重启守护进程的情况下执行操作。

### 插件系统

插件系统允许扩展功能。每个插件实现 `Plugin` trait，并在守护进程启动时自动加载。

