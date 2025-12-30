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
   
   pub struct MyPlugin {
       // 插件状态
   }
   
   #[async_trait]
   impl Plugin for MyPlugin {
       fn name(&self) -> &str { "myplugin" }
       async fn init(&mut self, niri: NiriIpc, config: &Config) -> Result<()> { /* ... */ }
       async fn run(&mut self) -> Result<()> { /* ... */ }
   }
   ```
3. 在 `src/plugins/mod.rs` 中注册插件
4. 在 `src/config.rs` 中添加插件配置结构
5. 更新配置文件示例

### 添加新的子命令

1. 在 `src/main.rs` 的 `Commands` 枚举中添加新的命令
2. 在 `src/commands.rs` 的 `CommandHandler` 中添加处理方法
3. 实现相应的功能模块
4. 在 `src/ipc.rs` 中添加相应的 IPC 消息类型（如果需要）

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

