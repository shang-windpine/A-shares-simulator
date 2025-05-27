# Connection Handler

A股模拟交易系统的核心连接处理库，作为服务器的中枢组件，负责管理所有客户端连接、消息路由分发以及与业务逻辑层的协调。

## 功能特性

### 🚀 核心功能
- **高并发连接管理**：支持多达 10,000 个并发客户端连接
- **消息路由分发**：根据消息类型将请求分发到相应的业务处理器
- **心跳处理**：在连接层面直接处理心跳消息，维持会话状态
- **优雅关闭**：支持服务器的优雅关闭流程
- **背压控制**：实现合理的背压机制，防止系统过载

### 📡 协议支持
- 基于 TCP 的自定义应用层协议
- 消息格式：`消息头 + Protobuf消息体`
- 与 `trade_protocal_lite` 深度集成
- 支持多种业务消息类型（登录、交易、查询、市场数据等）

### 🛡️ 可靠性保证
- 完善的错误处理机制
- 连接超时和心跳检测
- 消息大小限制防止内存攻击
- 全面的日志记录（使用 tracing）

## 架构设计

```
┌─────────────────────────────────────────────────────────────┐
│                        Server                               │
│  ┌─────────────────┐    ┌─────────────────┐                │
│  │   TcpListener   │    │  Connection     │                │
│  │      主监听     │    │   Manager       │                │
│  └─────────────────┘    └─────────────────┘                │
│            │                      │                        │
│            ▼                      ▼                        │
│  ┌─────────────────────────────────────────────────────────┐│
│  │              ClientConnection (每个客户端)               ││
│  │  ┌─────────────┐  ┌──────────────┐  ┌────────────────┐ ││
│  │  │FrameDecoder │  │ MessageDispat │  │ Heartbeat      │ ││
│  │  │             │  │ cher         │  │ Handler        │ ││
│  │  └─────────────┘  └──────────────┘  └────────────────┘ ││
│  └─────────────────────────────────────────────────────────┘│
│                           │                                 │
│                           ▼                                 │
│  ┌─────────────────────────────────────────────────────────┐│
│  │                 业务逻辑处理器                          ││
│  │   Login │ Trade │ Query │ Market │ ... (todo!())      ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

## 核心组件

### Server
服务器主组件，管理整个服务器的生命周期
- TCP 监听器管理
- 连接数量控制
- 优雅关闭协调

### ConnectionManager
连接管理器，负责管理所有活跃的客户端连接
- 连接 ID 分配和跟踪
- 连接状态维护
- 批量连接管理操作

### ClientConnection
客户端连接处理器，管理单个客户端连接的完整生命周期
- 消息接收和解析
- 心跳处理和超时检测
- 响应发送

### MessageDispatcher
消息分发器，根据消息类型将请求路由到相应的业务处理器
- 支持多种消息类型的路由
- 业务逻辑处理协调
- 模拟响应生成（当前阶段）

### FrameDecoder
帧解码器，处理 TCP 字节流并提取完整的消息帧
- 处理粘包和半包问题
- 消息完整性验证
- 与 `trade_protocal_lite` 深度集成

## 快速开始

### 添加依赖

在您的 `Cargo.toml` 中添加：

```toml
[dependencies]
connection_handler = { path = "path/to/connection_handler" }
tokio = { version = "1.0", features = ["full"] }
tracing = "0.1"
```

### 基本使用

```rust
use connection_handler::{Server, ServerConfig, init_tracing};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    init_tracing();
    
    // 创建服务器配置
    let config = ServerConfig {
        listen_addr: "127.0.0.1:8080".to_string(),
        max_connections: 10_000,
    };

    // 创建并运行服务器
    let mut server = Server::new(config).await?;
    
    println!("服务器启动，监听地址: {}", server.listen_addr());
    
    // 运行服务器主循环
    server.run().await?;
    
    Ok(())
}
```

### 运行示例

```bash
cargo run --example simple_server
```

## 支持的消息类型

当前支持以下消息类型的处理：

### 请求消息
- **LoginRequest**: 用户登录请求
- **NewOrderRequest**: 新订单请求  
- **CancelOrderRequest**: 撤单请求
- **MarketDataRequest**: 市场数据订阅请求
- **AccountQueryRequest**: 账户查询请求
- **Heartbeat**: 心跳消息

### 响应消息
- **LoginResponse**: 登录响应
- **OrderUpdateResponse**: 订单状态更新
- **MarketDataSnapshot**: 市场数据快照
- **AccountInfoResponse**: 账户信息响应
- **HeartbeatResponse**: 心跳响应
- **ErrorResponse**: 错误响应

## 配置参数

### ServerConfig
- `listen_addr`: 监听地址 (默认: "127.0.0.1:8080")
- `max_connections`: 最大连接数 (默认: 10,000)

### 连接参数
- `heartbeat_timeout`: 心跳超时时间 (默认: 30 秒)
- `heartbeat_check_interval`: 心跳检查间隔 (默认: 10 秒)
- `max_message_size`: 最大消息大小 (默认: 1MB)

## 性能特性

- **高并发**: 使用 Tokio 的 M:N 线程模型支持高并发
- **低延迟**: 异步 I/O 和零拷贝设计
- **内存安全**: Rust 的内存安全保证
- **可扩展**: 模块化设计支持功能扩展

## 开发状态

当前实现状态：

✅ **已完成**
- 基础架构和核心组件
- TCP 连接管理和消息处理
- 心跳机制和优雅关闭
- 消息路由和分发框架
- 全面的日志记录

🚧 **进行中**
- 业务逻辑处理器实现（当前为模拟响应）
- 性能优化和压力测试
- 更丰富的配置选项

📋 **计划中**
- 连接池和负载均衡
- 监控和指标收集
- 更完善的错误恢复机制

## 依赖关系

- `tokio`: 异步运行时
- `bytes`: 高效的字节缓冲区操作
- `tracing`: 结构化日志记录
- `trade_protocal_lite`: 协议定义和消息处理
- `thiserror`: 错误处理
- `chrono`: 时间处理

## 许可证

本项目采用 MIT 许可证。

## 贡献

欢迎提交问题报告和拉取请求！

---

*这是 A股模拟交易系统的核心组件，为高并发的交易场景设计。* 