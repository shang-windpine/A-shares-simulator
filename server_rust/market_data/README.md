# 市场行情包 (Market Data)

A股模拟交易软件的市场行情管理模块。

## 功能特性

- 📊 **静态数据管理**: 从MySQL获取每日固定的基础行情数据（开盘价、收盘价、涨跌停价等）
- 🔄 **动态数据更新**: 监听撮合引擎的交易数据，实时更新行情指标
- 📈 **行情计算**: 实时计算最高价、最低价、现价、成交量、成交额、均价等指标
- 🏢 **多股票支持**: 支持同时管理多个股票的行情数据
- 🗄️ **数据持久化**: 支持MySQL数据库的静态数据存储
- ⚡ **高性能**: 基于异步Rust实现，支持高并发处理

## 数据结构

### 静态市场数据
- 今日开盘价
- 昨日收盘价
- 今日涨跌停价
- 交易日期

### 动态市场数据
- 现价（最新成交价）
- 最高价、最低价
- 累计成交量、成交额
- 成交笔数
- 加权平均价格（VWAP）

### 扩展数据（预留）
- 总市值、流通市值
- 总股本、流通股本
- 换手率

## 架构设计

```
撮合引擎 → [MatchNotification] → 市场行情引擎 → [MarketDataNotification] → 外部系统
     ↓                                    ↑
MySQL数据库 ← [静态数据] ← 市场行情引擎 ← [MarketDataRequest] ← 外部系统
```

## 快速开始

### 1. 添加依赖

在你的 `Cargo.toml` 中添加：

```toml
[dependencies]
market_data = { path = "../market_data" }
core_entities = { path = "../core_entities" }
tokio = { version = "1.0", features = ["full"] }
```

### 2. 创建市场行情引擎

```rust
use market_data::*;
use tokio::sync::mpsc;
use std::sync::Arc;
use chrono::NaiveDate;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 创建通信通道
    let (match_notification_tx, match_notification_rx) = mpsc::channel(1000);
    let (market_data_notification_tx, market_data_notification_rx) = mpsc::channel(1000);
    let (market_data_request_tx, market_data_request_rx) = mpsc::channel(100);
    let (market_data_response_tx, market_data_response_rx) = mpsc::channel(100);

    // 配置数据库
    let db_config = DatabaseConfig {
        database_url: "mysql://root:password@localhost:3306/a_shares_db".to_string(),
        max_connections: 10,
        connect_timeout_secs: 30,
    };

    // 创建数据存储
    let repository = Arc::new(
        MySqlMarketDataRepository::new(db_config).await?
    );

    // 初始化数据库表
    repository.initialize_tables().await?;

    // 创建并启动市场行情引擎
    let mut engine = MarketDataEngineBuilder::new()
        .with_trade_date(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap())
        .with_auto_load_all_market_data(true)
        .with_repository(repository)
        .build(
            match_notification_rx,
            market_data_notification_tx,
            market_data_request_rx,
            market_data_response_tx,
        )?;

    engine.start().await?;

    // 引擎现在会自动处理来自撮合引擎的交易通知
    // 并实时更新市场数据

    Ok(())
}
```

### 3. 监听市场数据更新

```rust
while let Some(notification) = market_data_notification_rx.recv().await {
    match notification {
        MarketDataNotification::MarketDataUpdated { stock_id, market_data, timestamp } => {
            println!(
                "[{}] {} 行情更新: 现价=¥{}, 成交量={}",
                timestamp.format("%H:%M:%S"),
                stock_id,
                market_data.dynamic_data.current_price,
                market_data.dynamic_data.volume
            );
        }
        MarketDataNotification::TradeProcessed { stock_id, trade_id, price, quantity, .. } => {
            println!("交易处理: {} - {} @ ¥{} x {}", stock_id, trade_id, price, quantity);
        }
        _ => {}
    }
}
```

### 4. 请求市场数据

```rust
// 请求单个股票数据
market_data_request_tx.send(MarketDataRequest::GetMarketData {
    stock_id: "SH600036".into(),
}).await?;

// 接收响应
if let Some(response) = market_data_response_rx.recv().await {
    match response {
        MarketDataResponse::MarketData(market_data) => {
            println!("股票数据: {:?}", market_data);
        }
        _ => {}
    }
}
```

## 数据库表结构

市场行情包会自动创建以下数据库表：

```sql
CREATE TABLE market_static_data (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    stock_id VARCHAR(20) NOT NULL COMMENT '股票代码',
    trade_date DATE NOT NULL COMMENT '交易日期',
    open_price DECIMAL(12,4) NOT NULL COMMENT '开盘价',
    prev_close_price DECIMAL(12,4) NOT NULL COMMENT '昨日收盘价',
    limit_up_price DECIMAL(12,4) NOT NULL COMMENT '涨停价',
    limit_down_price DECIMAL(12,4) NOT NULL COMMENT '跌停价',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    
    UNIQUE KEY uk_stock_date (stock_id, trade_date),
    INDEX idx_trade_date (trade_date),
    INDEX idx_stock_id (stock_id)
);
```

## 运行示例

查看完整的集成示例：

```bash
cd server_rust/market_data
cargo run --example market_data_integration
```

## API 参考

### 核心类型

- `MarketData`: 完整的市场数据（静态 + 动态 + 扩展）
- `StaticMarketData`: 静态市场数据
- `DynamicMarketData`: 动态市场数据
- `MarketDataEngine`: 市场行情引擎

### 通知类型

- `MarketDataNotification::MarketDataUpdated`: 市场数据更新通知
- `MarketDataNotification::TradeProcessed`: 交易处理完成通知
- `MarketDataNotification::Error`: 错误通知

### 请求类型

- `MarketDataRequest::GetMarketData`: 获取单个股票数据
- `MarketDataRequest::GetMultipleMarketData`: 获取多个股票数据
- `MarketDataRequest::ReloadStaticData`: 重新加载静态数据

## 配置选项

### 引擎配置

```rust
let config = MarketDataEngineConfig {
    trade_date: NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
    auto_load_all_market_data: true,
    notification_buffer_size: 1000,
    request_buffer_size: 100,
};
```

### 数据库配置

```rust
let db_config = DatabaseConfig {
    database_url: "mysql://root:password@localhost:3306/a_shares_db".to_string(),
    max_connections: 10,
    connect_timeout_secs: 30,
};
```

## 测试

运行单元测试：

```bash
cargo test
```

运行集成测试（需要数据库）：

```bash
cargo test -- --ignored
```

## 性能特性

- ⚡ 异步处理：基于tokio的异步架构
- 🚀 内存缓存：热数据保存在内存中，快速访问
- 📊 批量操作：支持批量数据库操作
- 🔄 实时更新：毫秒级的行情数据更新

## 与其他模块的集成

### 撮合引擎集成

市场行情引擎自动监听撮合引擎的 `MatchNotification::TradeExecuted` 通知，并更新相应的市场数据。

### 订单引擎集成

可以通过市场数据请求获取实时行情，用于订单验证和风险控制。

## 未来扩展

下一个迭代计划添加的功能：

- 总市值计算
- 流通市值计算  
- 换手率计算
- 技术指标计算（MA、MACD等）
- WebSocket推送支持
- 行情快照功能 