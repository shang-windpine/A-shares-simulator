# 撮合引擎 (Matching Engine)

A股模拟交易软件的核心撮合引擎，负责订单匹配和交易生成。

## 功能特性

- ✅ **价格优先、时间优先**的撮合规则
- ✅ **异步处理**支持高并发
- ✅ **可插拔的存储层**抽象
- ✅ **观察者模式**支持事件通知
- ✅ **订单验证**和**价格改善策略**
- ✅ **完整的统计信息**
- ✅ **全面的单元测试**

## 核心组件

### 1. 数据类型 (`types.rs`)

- `Order` - 订单结构体
- `Trade` - 交易记录
- `OrderBookView` - 订单簿视图
- `MatchingStatistics` - 撮合统计信息

### 2. 抽象接口 (`traits.rs`)

- `OrderBookStore` - 订单簿存储接口
- `MatchLifecycleObserver` - 撮合生命周期观察者
- `MatchingEngine` - 撮合引擎核心接口
- `OrderValidator` - 订单验证器
- `PriceImprovementStrategy` - 价格改善策略

### 3. 核心实现 (`engine.rs`)

- `DefaultMatchingEngine` - 默认撮合引擎实现
- `DefaultOrderValidator` - 默认订单验证器
- `DefaultPriceImprovementStrategy` - 默认价格改善策略

### 4. 错误处理 (`errors.rs`)

- `MatchingEngineError` - 撮合引擎错误
- `OrderBookStoreError` - 存储层错误
- `MatchLifecycleObserverError` - 观察者错误

## 快速开始

### 基本使用

```rust
use matching_engine::*;
use rust_decimal_macros::dec;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 创建订单簿存储（需要实现 OrderBookStore trait）
    let store = YourOrderBookStore::new();

    // 2. 创建撮合引擎
    let validator = Arc::new(DefaultOrderValidator);
    let mut engine = DefaultMatchingEngine::new(store)
        .with_validator(validator);

    // 3. 启动引擎
    engine.start().await?;

    // 4. 创建订单
    let order = Order::new_limit_order(
        OrderId::from("ORDER_001"),
        StockId::from("SH600036"),
        UserId::from("user_001"),
        OrderSide::Buy,
        dec!(10.50),
        1000,
        chrono::Utc::now(),
    );

    // 5. 处理订单
    let trades = engine.process_order(order).await?;
    println!("产生了 {} 笔交易", trades.len());

    // 6. 停止引擎
    engine.stop().await?;
    Ok(())
}
```

### 实现订单簿存储

```rust
struct MyOrderBookStore {
    // 你的存储实现
}

#[async_trait::async_trait]
impl OrderBookStore for MyOrderBookStore {
    async fn submit_order(&mut self, order: &Order) -> OrderBookStoreResult<()> {
        // 实现订单提交逻辑
        Ok(())
    }

    async fn get_best_bid_ask(
        &self,
        stock_id: &StockId,
    ) -> OrderBookStoreResult<(Option<OrderBookEntry>, Option<OrderBookEntry>)> {
        // 实现获取最优买卖价逻辑
        Ok((None, None))
    }

    // 实现其他必需的方法...
}
```

### 添加观察者

```rust
struct MyObserver;

#[async_trait::async_trait]
impl MatchLifecycleObserver for MyObserver {
    async fn on_trade_generated(
        &self,
        trade: &Trade,
        remaining_qty: Quantity,
    ) -> MatchLifecycleObserverResult<()> {
        println!("交易生成: {} @ {}", trade.quantity, trade.price);
        Ok(())
    }

    // 实现其他观察者方法...
}

// 添加到引擎
let observer = Arc::new(MyObserver);
engine.add_observer(observer);
```

## 撮合规则

### 价格优先

- **买单**: 价格较高的买单优先成交
- **卖单**: 价格较低的卖单优先成交

### 时间优先

在同一价格层级，先提交的订单优先成交。

### 成交价格

成交价格为**被动方订单的价格**，实现价格改善效果。

## 运行示例

```bash
# 运行基本使用示例
cargo run --example basic_usage

# 运行所有测试
cargo test

# 运行特定测试
cargo test test_order_processing
```

## 测试覆盖

项目包含全面的单元测试：

- ✅ 数据类型测试 (23个测试用例)
- ✅ 接口行为测试
- ✅ 撮合逻辑测试
- ✅ 错误处理测试
- ✅ 观察者模式测试

```bash
$ cargo test
running 23 tests
test result: ok. 23 passed; 0 failed; 0 ignored
```

## 架构设计

撮合引擎采用分层架构设计：

```
┌─────────────────────────────────────┐
│           应用层                     │
│    (订单管理系统、API服务等)          │
└─────────────────────────────────────┘
                    │
┌─────────────────────────────────────┐
│         撮合引擎核心                 │
│  ┌─────────────┐ ┌─────────────┐    │
│  │ 订单验证器   │ │ 价格策略     │    │
│  └─────────────┘ └─────────────┘    │
│  ┌─────────────────────────────┐    │
│  │      撮合逻辑核心            │    │
│  └─────────────────────────────┘    │
│  ┌─────────────────────────────┐    │
│  │      观察者容器              │    │
│  └─────────────────────────────┘    │
└─────────────────────────────────────┘
                    │
┌─────────────────────────────────────┐
│         存储抽象层                   │
│    (Redis、内存存储等)               │
└─────────────────────────────────────┘
```

## 扩展点

- **存储层**: 实现 `OrderBookStore` trait 支持不同存储后端
- **验证器**: 实现 `OrderValidator` trait 自定义验证逻辑
- **价格策略**: 实现 `PriceImprovementStrategy` trait 自定义价格确定逻辑
- **观察者**: 实现 `MatchLifecycleObserver` trait 监听撮合事件

## 性能考虑

- 使用 `async/await` 支持高并发
- 存储层抽象避免阻塞操作
- 观察者异步通知不影响核心撮合性能
- 使用 `Arc<RwLock<>>` 实现线程安全的共享状态

## 依赖项

- `tokio` - 异步运行时
- `async-trait` - 异步trait支持
- `rust_decimal` - 高精度小数计算
- `chrono` - 时间处理
- `uuid` - 唯一ID生成
- `thiserror` - 错误处理

## 许可证

本项目采用 MIT 许可证。 