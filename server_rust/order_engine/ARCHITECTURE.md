# 订单引擎架构设计

## 系统概述

订单引擎是A股模拟交易系统的核心组件，负责高效管理订单生命周期，与撮合引擎协调工作，提供高并发、高可用的订单处理服务。

## 核心组件架构

```
┌─────────────────────────────────────────────────────────────┐
│                    Order Engine                             │
├─────────────────────────────────────────────────────────────┤
│                                                            │
│  ┌─────────────────────┐    ┌─────────────────────────────┐│
│  │   Order Pool        │    │    Engine Core              ││
│  │                     │    │                             ││
│  │ ┌─────────────────┐ │    │ ┌─────────────────────────┐ ││
│  │ │ Orders          │ │    │ │ OrderNotification TX    │ ││
│  │ │ (DashMap)       │ │    │ │                         │ ││
│  │ └─────────────────┘ │    │ └─────────────────────────┘ ││
│  │                     │    │                             ││
│  │ ┌─────────────────┐ │    │ ┌─────────────────────────┐ ││
│  │ │ User Index      │ │    │ │ MatchNotification RX    │ ││
│  │ │ (DashMap)       │ │    │ │                         │ ││
│  │ └─────────────────┘ │    │ └─────────────────────────┘ ││
│  │                     │    │                             ││
│  │ ┌─────────────────┐ │    │ ┌─────────────────────────┐ ││
│  │ │ Stock Index     │ │    │ │ Validation              │ ││
│  │ │ (DashMap)       │ │    │ │                         │ ││
│  │ └─────────────────┘ │    │ └─────────────────────────┘ ││
│  │                     │    │                             ││
│  │ ┌─────────────────┐ │    │ ┌─────────────────────────┐ ││
│  │ │ Active Orders   │ │    │ │ Cleanup Task            │ ││
│  │ │ (DashMap)       │ │    │ │                         │ ││
│  │ └─────────────────┘ │    │ └─────────────────────────┘ ││
│  │                     │    │                             ││
│  │ ┌─────────────────┐ │    │                             ││
│  │ │ Statistics      │ │    │                             ││
│  │ │ (RwLock)        │ │    │                             ││
│  │ └─────────────────┘ │    │                             ││
│  └─────────────────────┘    └─────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
                       │                            │
         OrderNotification                MatchNotification
                       ↓                            ↑
┌─────────────────────────────────────────────────────────────┐
│                 Matching Engine                             │
└─────────────────────────────────────────────────────────────┘
```

## 数据结构设计

### OrderPool - 订单池

高并发的订单存储和索引系统：

```rust
pub struct OrderPool {
    /// 主索引：订单ID -> 订单
    orders: DashMap<String, Arc<RwLock<Order>>>,
    
    /// 用户索引：用户ID -> 订单ID集合
    user_orders: DashMap<String, Arc<RwLock<Vec<String>>>>,
    
    /// 股票索引：股票ID -> 订单ID集合
    stock_orders: DashMap<String, Arc<RwLock<Vec<String>>>>,
    
    /// 活跃订单索引：只包含未完成的订单
    active_orders: DashMap<String, Arc<RwLock<Order>>>,
    
    /// 统计信息
    stats: Arc<RwLock<OrderPoolStats>>,
}
```

**设计特点：**
- **DashMap**: 无锁并发哈希表，支持高并发读写
- **Arc<RwLock<T>>**: 智能指针 + 读写锁，优化内存使用和并发性能
- **多重索引**: 支持O(1)时间复杂度的快速查询
- **活跃订单索引**: 快速过滤活跃订单，避免全表扫描

### OrderEngine - 引擎核心

订单生命周期管理和通信协调：

```rust
pub struct OrderEngine {
    /// 订单池
    order_pool: Arc<OrderPool>,
    
    /// 订单验证器
    validator: Option<Arc<dyn OrderValidator>>,
    
    /// 向匹配引擎发送订单通知的channel
    order_notification_tx: mpsc::Sender<OrderNotification>,
    
    /// 从匹配引擎接收匹配结果的channel
    match_notification_rx: mpsc::Receiver<MatchNotification>,
    
    /// 配置
    config: OrderEngineConfig,
    
    /// 是否正在运行
    running: bool,
}
```

## 消息流设计

### 1. 订单提交流程

```
Client Request
     │
     ▼
┌─────────────────┐
│ submit_order    │
│                 │
│ 1. 验证         │
│ 2. 添加到池     │
│ 3. 发送通知     │
└─────────────────┘
     │
     ▼
┌─────────────────┐    OrderNotification    ┌─────────────────┐
│ Order Engine    │ ────────────────────── │ Matching Engine │
└─────────────────┘                        └─────────────────┘
```

### 2. 匹配结果处理流程

```
┌─────────────────┐    MatchNotification    ┌─────────────────┐
│ Matching Engine │ ────────────────────── │ Order Engine    │
└─────────────────┘                        └─────────────────┘
                                                    │
                                                    ▼
                                           ┌─────────────────┐
                                           │ 更新订单状态     │
                                           │                 │
                                           │ 1. 更新数量     │
                                           │ 2. 更新状态     │
                                           │ 3. 更新索引     │
                                           └─────────────────┘
```

### 3. 订单取消流程

```
Client Request
     │
     ▼
┌─────────────────┐    CancelOrder    ┌─────────────────┐
│ cancel_order    │ ────────────────  │ Matching Engine │
└─────────────────┘                   └─────────────────┘
     │                                         │
     ▼                                         ▼
     验证订单存在                    OrderCancelled/Rejected
     │                                         │
     ▼                                         ▼
┌─────────────────┐                    ┌─────────────────┐
│ 发送取消请求     │                    │ 更新订单状态     │
└─────────────────┘                    └─────────────────┘
```

## 并发安全设计

### 1. 线程安全保证

- **DashMap**: 内置线程安全，支持并发读写
- **RwLock**: 读写锁，读操作可并发，写操作互斥
- **Arc**: 引用计数指针，安全的内存共享
- **异步消息传递**: 避免共享状态的竞争

### 2. 性能优化

```rust
// 订单查询 - O(1) 时间复杂度
pub fn get_order(&self, order_id: &str) -> Option<Arc<RwLock<Order>>> {
    self.orders.get(order_id).map(|entry| entry.value().clone())
}

// 用户订单查询 - O(1) + O(n) 其中n是用户订单数
pub fn get_orders_by_user(&self, user_id: &str) -> Vec<Arc<RwLock<Order>>> {
    if let Some(order_ids) = self.user_orders.get(user_id) {
        order_ids
            .read()
            .iter()
            .filter_map(|order_id| self.orders.get(order_id).map(|entry| entry.value().clone()))
            .collect()
    } else {
        Vec::new()
    }
}
```

### 3. 内存管理

- **智能指针**: 自动内存管理，防止内存泄漏
- **定期清理**: 自动清理已完成的订单
- **引用计数**: 订单只在需要时保持在内存中

## 可扩展性设计

### 1. 模块化架构

- **OrderPool**: 独立的存储层，可单独使用
- **OrderEngine**: 业务逻辑层，协调各组件
- **OrderValidator**: 可插拔的验证组件

### 2. 配置驱动

```rust
pub struct OrderEngineConfig {
    /// 订单通知channel的缓冲区大小
    pub order_notification_buffer_size: usize,
    /// 匹配通知channel的缓冲区大小
    pub match_notification_buffer_size: usize,
    /// 是否启用订单验证
    pub enable_validation: bool,
    /// 订单池清理间隔（秒）
    pub cleanup_interval_seconds: u64,
    /// 保留已完成订单的时间（小时）
    pub retain_completed_orders_hours: i64,
}
```

### 3. 可扩展的消息系统

```rust
// 可以轻松添加新的通知类型
pub enum OrderNotification {
    NewOrder(Order),
    CancelOrder { order_id: Arc<str>, stock_id: Arc<str> },
    // 未来可扩展：
    // ModifyOrder { ... },
    // BatchOrders { ... },
}

pub enum MatchNotification {
    TradeExecuted { ... },
    OrderCancelled { ... },
    OrderCancelRejected { ... },
    // 未来可扩展：
    // MarketDataUpdate { ... },
    // SystemMaintenance { ... },
}
```

## 容错与恢复

### 1. 错误处理

- **全面的错误类型**: 使用 `Result<T, String>` 处理所有可能的错误
- **事务性操作**: 订单提交失败时自动回滚
- **优雅降级**: 部分功能失败不影响整体服务

### 2. 状态一致性

- **原子操作**: 确保订单状态更新的原子性
- **索引同步**: 主索引和辅助索引保持一致性
- **统计信息**: 实时更新，反映当前真实状态

### 3. 监控与日志

```rust
use tracing::{debug, error, info, warn};

// 结构化日志记录
info!("Successfully submitted order: {}", order_id);
warn!("Order validation failed for {}: {}", order.order_id, e);
error!("Failed to send order notification: {}", e);
```

## 性能特征

### 1. 吞吐量

- **并发订单处理**: 支持数千并发订单提交
- **快速查询**: O(1) 订单查询，O(n) 批量查询
- **异步处理**: 非阻塞的消息处理

### 2. 延迟

- **内存操作**: 所有操作都在内存中完成
- **无锁设计**: DashMap 避免锁竞争
- **批量操作**: 支持批量查询和更新

### 3. 资源使用

- **内存优化**: 智能指针减少内存拷贝
- **CPU 友好**: 无锁数据结构减少 CPU 开销
- **可配置清理**: 自动管理内存使用

## 测试策略

### 1. 单元测试

- **订单池操作**: 测试所有 CRUD 操作
- **索引一致性**: 验证索引更新的正确性
- **统计信息**: 确保统计数据的准确性

### 2. 集成测试

- **引擎通信**: 测试与匹配引擎的消息传递
- **并发访问**: 验证高并发场景下的正确性
- **错误恢复**: 测试各种错误情况的处理

### 3. 性能测试

- **压力测试**: 测试高并发订单提交
- **内存测试**: 验证长时间运行的内存稳定性
- **延迟测试**: 测量操作响应时间

## 部署考虑

### 1. 资源配置

```toml
[dependencies]
tokio = { version = "1.0", features = ["full"] }
dashmap = "6.1.0"
parking_lot = "0.12"
tracing = "0.1"
```

### 2. 运行时配置

```rust
let config = OrderEngineConfig {
    order_notification_buffer_size: 10000,   // 根据预期负载调整
    match_notification_buffer_size: 10000,
    cleanup_interval_seconds: 3600,          // 1小时清理一次
    retain_completed_orders_hours: 24,       // 保留24小时历史
};
```

### 3. 监控指标

- **订单吞吐量**: 每秒处理的订单数
- **池大小**: 当前订单池中的订单数量
- **响应延迟**: 订单操作的平均响应时间
- **错误率**: 操作失败的比例

这个架构设计确保了订单引擎在高并发、高可用环境下的稳定运行，同时保持了良好的可扩展性和维护性。 