# A股模拟交易系统核心引擎
（早期开发阶段， 持续更新中...， 请勿直接使用）
![License](https://img.shields.io/badge/license-MIT-green)

一个面向A股市场的高性能交易系统核心引擎，专注于构建可扩展的基础设施架构和三大核心交易引擎的协同设计。项目从零开始搭建了完整的交易系统基础设施，并实现了订单引擎、撮合引擎和行情引擎之间的高效互通机制。

## 📋 目录

- [系统架构](#系统架构)
- [核心特性](#核心特性)
- [技术栈](#技术栈)
- [网络通信架构](#网络通信架构)
- [项目结构](#项目结构)
- [快速开始](#快速开始)
- [配置说明](#配置说明)
- [开发指南](#开发指南)

## 🏗️ 系统架构

```mermaid
graph TB
    subgraph "数据层 (Data Layer)"
        A[MySQL 数据库<br/>股票基础信息<br/>交易数据<br/>技术指标]
        B[Tushare API<br/>实时数据源]
        C[Python 数据采集<br/>定时任务]
        B --> C
        C --> A
    end
    
    subgraph "核心交易引擎 (Core Trading Engines)"
        D[订单引擎<br/>Order Engine<br/>订单管理与验证]
        E[撮合引擎<br/>Matching Engine<br/>价格时间优先撮合]
        F[行情引擎<br/>Market Data Engine<br/>行情分发与订阅]
        
        D <--> E
        E <--> F
        F <--> D
    end
    
    subgraph "基础设施层 (Infrastructure Layer)"
        G[核心实体<br/>Core Entities<br/>共享数据模型]
        H[通信协议<br/>Trade Protocol<br/>Protobuf 定义]
        I[服务器核心<br/>Server Core<br/>网络通信与路由]
        
        D --> G
        E --> G
        F --> G
        I --> H
    end
    
    subgraph "客户端层 (Client Layer)"
        J[测试客户端<br/>Test Client<br/>命令行交互]
    end
    
    A -.-> D
    A -.-> F
    I <--> J
    I --> D
    I --> E
    I --> F
    
    style D fill:#e1f5fe
    style E fill:#e8f5e8
    style F fill:#fff3e0
    style A fill:#f3e5f5
    style I fill:#fce4ec
```

## ✨ 核心特性

### 三大核心引擎

- **📋 订单引擎 (Order Engine)**
  - 订单接收、验证和状态管理
  - 完整的订单生命周期追踪
  - 订单持久化和恢复机制

- **⚡ 撮合引擎 (Matching Engine)**
  - 价格优先、时间优先的撮合算法
  - 高性能订单簿管理
  - 实时成交回报生成

- **📊 行情引擎 (Market Data Engine)**
  - 实时行情数据处理
  - 多客户端订阅分发
  - 行情快照生成

### 基础设施特性

- **🚀 高性能架构**: 基于 Rust + Tokio 异步运行时
- **🔧 模块化设计**: Cargo Workspace 多模块架构
- **💾 数据持久化**: MySQL + SQLx 类型安全访问
- **🌐 标准化协议**: Protocol Buffers 通信协议
- **⚡ 原生网络通信**: 基于TCP/UDP直连，非HTTP协议
- **📈 真实数据**: Tushare API 数据源集成

## 🛠️ 技术栈

### 后端核心
- **Rust 1.70+**: 系统级性能与内存安全
- **Tokio**: 高性能异步运行时
- **TCP/UDP**: 原生网络协议，低延迟通信
- **SQLx**: 类型安全的数据库访问
- **Prost**: Protocol Buffers 实现
- **Serde**: 序列化/反序列化框架

### 数据层
- **MySQL 8.0+**: 关系型数据库
- **Python 3.8+**: 数据采集脚本
- **Tushare**: 金融数据API

### 开发工具
- **Cargo**: Rust 包管理器
- **Protocol Buffers**: 接口定义语言
- **Tracing**: 结构化日志记录

## 🌐 网络通信架构

### 为什么选择TCP/UDP而非HTTP？

本项目采用原生的TCP/UDP协议进行客户端与服务器之间的通信，而非传统的HTTP协议。这一设计决策基于以下考虑：

#### **1. 极低延迟要求**
- 金融交易系统对延迟极其敏感，每毫秒都可能影响交易结果
- TCP直连避免了HTTP协议的额外开销（请求头、状态码等）
- 减少了网络栈的处理层级，实现更短的响应时间

#### **2. 持久连接优势**
- 建立一次TCP连接后可持续使用，避免HTTP的连接建立/销毁开销
- 支持长连接保活机制，减少握手次数
- 更适合高频交易场景的连续通信需求

#### **3. 双向实时通信**
- 服务器可主动推送行情数据、成交回报等实时信息
- 避免HTTP的轮询机制，实现真正的事件驱动通信
- 支持心跳机制，确保连接状态的实时监控

#### **4. 协议栈优化**
- **TCP**：用于订单提交、账户查询等可靠性要求高的操作
- **UDP**（预留）：未来可用于高频行情推送，进一步降低延迟
- 根据不同业务场景选择最适合的传输层协议

#### **5. 自定义协议控制**
- 基于Protocol Buffers的二进制协议，数据紧凑、解析高效
- 完全控制消息格式和传输机制
- 支持消息压缩、加密等高级特性的灵活实现

这种设计使得系统能够达到微秒级的响应时间，满足高频交易和实时行情分发的严格性能要求。

## 📁 项目结构

```
├── server_rust/                 # Rust 服务器端
│   ├── core_entities/          # 核心数据模型
│   ├── order_engine/           # 订单引擎
│   ├── matching_engine/        # 撮合引擎
│   ├── market_data_engine/     # 行情引擎
│   ├── server_core/            # 服务器核心
│   ├── market_rules/           # 市场规则(预留)
│   ├── account_management/     # 账户管理(预留)
│   └── tick_generator/         # 行情模拟器(预留)
├── trade_protocal_lite/        # 通信协议定义
├── client/                     # 客户端
│   └── test_client/           # 测试客户端
├── tushare_python/            # 数据采集模块
├── database_design.md         # 数据库设计文档
└── create_tables.sql          # 数据库建表脚本
```

## 🚀 快速开始

### 环境要求

- Rust 1.70 或更高版本
- MySQL 8.0 或更高版本
- Python 3.8 或更高版本

### 1. 克隆项目

```bash
git clone https://github.com/your-username/A-shares-simulator.git
cd A-shares-simulator
```

### 2. 数据库设置

```bash
# 创建数据库
mysql -u root -p -e "CREATE DATABASE a_shares_trading;"

# 导入表结构
mysql -u root -p a_shares_trading < create_tables.sql
```

### 3. 配置文件

复制并修改配置文件：

```bash
cd server_rust
cp config.example.yml config.yml
# 编辑 config.yml 文件，填入数据库连接信息
```

### 4. 数据采集（可选）

```bash
cd tushare_python
pip install -r requirements.txt
# 配置 Tushare API Token
python get_all_stocks_basic_info.py
```

### 5. 构建和运行服务器

```bash
cd server_rust
cargo build --release
cargo run --bin server_core
```

### 6. 运行测试客户端

```bash
cd client/test_client
cargo run
```

## ⚙️ 配置说明

### 服务器配置 (server_rust/config.yml)

```yaml
server:
  host: "127.0.0.1"
  port: 8080
  
database:
  url: "mysql://username:password@localhost:3306/a_shares_trading"
  max_connections: 20
  
logging:
  level: "info"
  file_path: "logs/server.log"
```

### 客户端配置

客户端配置通过代码中的 `TestClientConfig` 结构体进行设置：

```rust
TestClientConfig {
    server_addr: "127.0.0.1:8080".to_string(),
    user_id: "test_user_001".to_string(),
    heartbeat_interval_secs: 20,
    message_timeout_secs: 10,
}
```

### Protocol Buffers 定义

系统使用 Protocol Buffers 定义客户端与服务器之间的通信协议。主要消息类型包括：

- `LoginRequest/LoginResponse`: 用户登录
- `NewOrderRequest/OrderUpdateResponse`: 订单提交和更新
- `CancelOrderRequest`: 订单撤销
- `MarketDataRequest/MarketDataSnapshot`: 行情订阅和推送
- `AccountQueryRequest/AccountInfoResponse`: 账户查询
- `Heartbeat/HeartbeatResponse`: 心跳保活

详细的协议定义请查看 `trade_protocal_lite/src/protos/trade_protocol.proto`

### 核心流程

1. **客户端连接**: TCP长连接建立 → 登录验证 → 会话建立
2. **订单处理**: 订单提交(TCP) → 验证 → 撮合 → 成交回报推送
3. **行情服务**: 订阅请求(TCP) → 实时推送 → 数据分发
4. **心跳机制**: 定时心跳(TCP) → 连接保活 → 超时检测

> **注意**: 所有通信均基于TCP协议，确保低延迟和高可靠性。未来行情推送可选择UDP协议以进一步优化性能。

## 🔧 开发指南

### 添加新的引擎模块

1. 在 `server_rust/` 下创建新的 crate
2. 在根目录 `Cargo.toml` 中添加成员
3. 实现标准的引擎接口
4. 在 `server_core` 中注册新引擎

### 扩展协议定义

1. 修改 `trade_protocal_lite/src/protos/trade_protocol.proto`
2. 运行 `cargo build` 重新生成代码
3. 更新客户端和服务器端的处理逻辑

### 数据库扩展

1. 更新 `database_design.md` 文档
2. 修改 `create_tables.sql` 脚本
3. 更新对应的 Rust 结构体定义

## 📄 许可证

本项目采用 MIT 许可证 - 查看 [LICENSE](LICENSE) 文件了解详情。

**注意**: 本项目仅用于学习和演示目的，不构成任何投资建议。在实际交易中请谨慎决策。

