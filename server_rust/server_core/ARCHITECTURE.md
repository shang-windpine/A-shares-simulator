# Server Core 架构设计

## 概述

本项目实现了一个分层架构的交易服务器，支持依赖注入和服务编排。核心业务逻辑包括订单处理引擎和撮合引擎的完整集成。

## 架构层次

### 1. 应用层 (App)
- **文件**: `src/app.rs`
- **职责**: 服务编排、依赖注入、应用生命周期管理
- **核心组件**:
  - `App`: 应用程序主入口
  - `AppConfig`: 应用程序配置
  - `AppServices`: 业务服务容器，包含 OrderEngine 和 MatchingEngine

### 2. 网络层 (Server)
- **文件**: `src/server.rs`
- **职责**: TCP连接管理、网络服务器
- **核心组件**:
  - `Server`: TCP服务器
  - `ServerConfig`: 服务器配置

### 3. 连接管理层 (ConnectionManager)
- **文件**: `src/connection_manager.rs`
- **职责**: 客户端连接生命周期管理
- **核心组件**:
  - `ConnectionManager`: 连接管理器实现
  - `ConnectionManagement`: 连接管理抽象trait

### 4. 协议适配层 (MessageDispatcher)
- **文件**: `src/message_dispatcher.rs`
- **职责**: 消息路由、协议转换、业务逻辑调用
- **核心组件**:
  - `MessageDispatcher`: 消息分发器

### 5. 业务逻辑层
- **OrderEngine**: `order_engine` crate - 订单处理、风控验证
- **MatchingEngine**: `matching_engine` crate - 订单撮合、交易生成

## 业务服务架构

### 服务通信模型
```
OrderEngine  ──[OrderNotification]──> MatchingEngine
     ↑                                      │
     │                                      ↓
     └──[MatchNotification]──────────────────┘
```

### 核心业务流程
1. **OrderEngine** 接收和验证订单
2. 通过 `OrderNotification` channel 发送订单到 **MatchingEngine**
3. **MatchingEngine** 执行撮合逻辑，生成交易
4. 通过 `MatchNotification` channel 返回撮合结果到 **OrderEngine**
5. **OrderEngine** 更新订单状态和用户资金

## 依赖注入设计

### 服务启动流程
```
App::new() 
  -> App::initialize_services()
    -> OrderEngineFactory::create_with_channels()  // 创建OrderEngine和通信channels
    -> MatchingEngine::new(order_rx, match_tx)     // 创建MatchingEngine
  -> App::start_services()
    -> spawn MatchingEngine::run()                 // 启动撮合引擎任务
  -> Server::new_with_services(order_engine)
    -> ConnectionManager::create_and_manage_connection_with_services()
      -> ClientConnection::new_with_control_and_services(order_engine)
        -> MessageDispatcher::new(connection_id, order_engine)
```

### 依赖关系
- `App` 创建并管理 `OrderEngine` 和 `MatchingEngine` 实例
- `App` 负责启动 `MatchingEngine` 的异步任务
- `Server` 接收 `OrderEngine` 引用并传递给连接管理器
- `ConnectionManager` 将 `OrderEngine` 传递给每个客户端连接
- `ClientConnection` 将 `OrderEngine` 注入到 `MessageDispatcher`
- `MessageDispatcher` 使用 `OrderEngine` 处理订单相关请求
- `MatchingEngine` 作为独立服务运行，通过 channels 与 `OrderEngine` 通信

## 业务逻辑集成

### 订单处理流程
1. 客户端发送 `NewOrderRequest`
2. `MessageDispatcher::handle_new_order_request()` 接收请求
3. 协议类型转换 (protobuf -> core_entities)
4. 调用 `OrderEngine::submit_order()` 提交订单
5. **OrderEngine** 内部将订单发送到 **MatchingEngine**
6. **MatchingEngine** 执行撮合并返回结果
7. 返回 `OrderUpdateResponse` 给客户端

### 撤单处理流程
1. 客户端发送 `CancelOrderRequest`
2. `MessageDispatcher::handle_cancel_order_request()` 接收请求
3. 调用 `OrderEngine::get_order()` 查询订单
4. 调用 `OrderEngine::cancel_order()` 提交撤单请求
5. **OrderEngine** 内部处理撤单逻辑
6. 返回 `OrderUpdateResponse` 给客户端

### 服务生命周期管理
- **初始化阶段**: `App::initialize_services()` 创建所有服务实例
- **启动阶段**: `App::start_services()` 启动需要异步运行的服务
- **运行阶段**: 各服务协同工作处理业务请求
- **关闭阶段**: `App::shutdown()` 优雅关闭所有服务

## 向后兼容性

- 保留了 `MessageDispatcher::new_without_services()` 方法
- 为没有业务服务依赖的场景创建默认的 `OrderEngine` 实例
- 现有的测试和API保持不变

## 使用示例

```rust
use server_core::{App, init_tracing};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();
    
    // 创建并初始化应用
    let mut app = App::with_defaults();
    app.initialize_services().await?;
    
    // 启动应用（包括所有业务服务）
    app.run().await?;
    
    Ok(())
}
```

## 扩展性

这个架构设计支持未来添加更多业务服务：
- 账户管理服务
- 市场数据服务  
- 风控服务
- 清算结算服务
- 等等

只需要在 `AppServices` 中添加新的服务引用，并通过依赖注入传递给需要的组件。 