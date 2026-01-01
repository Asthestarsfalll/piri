# 开发指南

## 扩展性

### 添加新的插件

1. 在 `src/plugins/` 目录下创建新的插件文件（例如 `myplugin.rs`）
2. 实现 `Plugin` trait：
   ```rust
   use async_trait::async_trait;
   use crate::plugins::Plugin;
   use crate::config::Config;
   use crate::niri::NiriIpc;
   use crate::ipc::IpcRequest;
   use niri_ipc::Event;
   use anyhow::Result;
   
   pub struct MyPlugin {
       niri: NiriIpc,
       // 插件状态
   }
   
   impl MyPlugin {
       pub fn new() -> Self {
           Self {
               niri: NiriIpc::new(None),
           }
       }
   }
   
   #[async_trait]
   impl Plugin for MyPlugin {
       fn name(&self) -> &str { "myplugin" }
       
       async fn init(&mut self, niri: NiriIpc, config: &Config) -> Result<()> {
           self.niri = niri;
           // 初始化插件，读取配置等
           Ok(())
       }
       
       // 处理 IPC 请求（可选，如果插件需要响应客户端命令）
       async fn handle_ipc_request(&mut self, request: &IpcRequest) -> Result<Option<Result<()>>> {
           // 如果处理了请求，返回 Some(Ok(()))
           // 如果未处理，返回 Ok(None)
           Ok(None)
       }
       
       // 处理 niri 事件（可选，仅事件驱动插件需要）
       async fn handle_event(&mut self, event: &Event, niri: &NiriIpc) -> Result<()> {
           match event {
               Event::WindowOpenedOrChanged { window } => {
                   // 处理窗口打开或改变事件
               }
               _ => {
                   // 忽略其他事件
               }
           }
           Ok(())
       }
       
       // 声明插件感兴趣的事件类型（用于事件过滤）
       fn is_interested_in_event(&self, event: &Event) -> bool {
           matches!(event, Event::WindowOpenedOrChanged { .. })
       }
       
       // 更新配置（可选，支持热重载）
       async fn update_config(&mut self, niri: NiriIpc, config: &Config) -> Result<()> {
           // 更新插件配置
           Ok(())
       }

       // 关闭插件（可选，用于清理资源）
       async fn shutdown(&mut self) -> Result<()> {
           // 清理资源
           Ok(())
       }
   }
   ```
3. 在 `src/plugins/mod.rs` 中注册插件：
   - 在文件顶部添加 `pub mod myplugin;`
   - 在 `PluginManager::init` 方法中添加插件初始化逻辑
4. 在 `src/config.rs` 中添加插件配置结构：
   - 在 `PluginsConfig` 中添加插件启用/禁用选项
   - 添加插件特定的配置结构（如果需要）
5. 在 `src/ipc.rs` 中添加 IPC 请求类型（如果插件需要响应客户端命令）
6. 在 `src/main.rs` 中添加 CLI 命令（如果插件需要命令行接口）
7. 更新配置文件示例 `config.example.toml`

#### 事件驱动插件

如果你需要创建一个基于事件的插件（例如监听窗口事件、工作区切换等），只需实现 `handle_event` 方法。**无需创建自己的事件监听循环**，因为 Piri 使用统一的事件分发机制：

- 所有事件由 `PluginManager` 统一监听
- 事件通过 `handle_event` 方法分发给各个插件
- 插件只需关注自己感兴趣的事件类型

这大大简化了插件开发，并确保了高效的资源使用。

### 添加新的子命令

1. 在 `src/main.rs` 的 `Commands` 枚举中添加新的命令
2. 在 `src/main.rs` 的 `async_main` 函数中添加命令处理逻辑
3. 如果命令需要与守护进程通信：
   - 在 `src/ipc.rs` 的 `IpcRequest` 枚举中添加请求类型
   - 在 `src/ipc.rs` 的 `handle_request` 函数中添加请求处理逻辑
   - 或者通过插件系统处理（如果命令属于某个插件）
4. 如果命令需要直接访问 niri：
   - 创建 `NiriIpc` 实例
   - 调用相应的 niri IPC 方法

### 添加新的配置项

1. 在 `src/config.rs` 的 `Config` 结构体中添加字段
2. 更新 TOML 配置文件示例

## 代码格式化

项目使用 `rustfmt` 进行代码格式化。配置文件为 `rustfmt.toml`。

### 安装 rustfmt

```bash
rustup component add rustfmt
```

### 格式化代码

```bash
# 格式化所有代码
cargo fmt

# 检查代码格式（不修改文件）
cargo fmt -- --check
```

## 依赖

- `clap`: 命令行参数解析
- `serde` / `toml`: 配置序列化/反序列化
- `tokio`: 异步运行时
- `anyhow`: 错误处理
- `log` / `env_logger`: 日志系统
- `niri-ipc`: Niri IPC 客户端库

