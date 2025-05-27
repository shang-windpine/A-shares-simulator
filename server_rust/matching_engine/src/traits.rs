//! 撮合引擎抽象接口定义
//!
//! 本模块定义了撮合引擎系统的核心抽象接口，采用分层架构设计，实现了业务逻辑与数据存储的解耦。
//!
//! # 架构层次结构
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     撮合引擎系统架构                              │
//! └─────────────────────────────────────────────────────────────────┘
//!
//!          外部调用者 (API层/交易客户端)
//!                      │
//!                      ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                 MatchingEngine                                   │
//! │                (撮合引擎核心接口)                                 │
//! │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
//! │  │  process_order  │  │  cancel_order   │  │ get_order_book  │  │
//! │  │                 │  │                 │  │                 │  │
//! │  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
//! └─────────────────────────────────────────────────────────────────┘
//!                      │                │
//!                      ▼                ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    辅助组件层                                     │
//! │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
//! │  │ OrderValidator  │  │PriceImprovement │  │ ObserverContainer│  │
//! │  │   (订单验证)     │  │   Strategy      │  │  (观察者容器)    │  │
//! │  │                 │  │  (价格策略)     │  │                 │  │
//! │  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
//! └─────────────────────────────────────────────────────────────────┘
//!                      │                              │
//!                      ▼                              ▼
//! ┌─────────────────────────────────────┐    ┌─────────────────────┐
//! │        OrderBookStore               │    │ MatchLifecycle      │
//! │       (订单簿存储层)                 │    │   Observer          │
//! │  ┌─────────────────────────────────┐ │    │  (生命周期观察者)    │
//! │  │ • submit_order                  │ │    │ ┌─────────────────┐ │
//! │  │ • cancel_order                  │ │    │ │on_before_match  │ │
//! │  │ • get_order_book_snapshot       │ │    │ │on_trade_generated│ │
//! │  │ • update_order_book_after_trade │ │    │ │on_order_cancelled│ │
//! │  │ • get_best_bid_ask              │ │    │ │    ...          │ │
//! │  └─────────────────────────────────┘ │    │ └─────────────────┘ │
//! └─────────────────────────────────────┘    └─────────────────────┘
//!                      │
//!                      ▼
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                 底层存储 (Redis/内存数据库)                        │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # 核心组件说明
//!
//! ## MatchingEngine (撮合引擎核心)
//! - **职责**: 提供完整的订单撮合业务流程
//! - **特性**: 业务逻辑编排、错误处理、观察者通知
//! - **返回**: `MatchingEngineResult<T>` 包含完整业务错误信息
//!
//! ## OrderBookStore (订单簿存储层)  
//! - **职责**: 纯数据层操作，管理订单簿在存储中的CRUD
//! - **特性**: 高性能数据操作、存储抽象
//! - **返回**: `OrderBookStoreResult<T>` 专注存储层错误
//!
//! ## MatchLifecycleObserver (生命周期观察者)
//! - **职责**: 在撮合关键节点提供钩子机制
//! - **特性**: 事件驱动、可插拔的业务扩展点
//! - **用途**: 风控、审计、通知、统计等
//!
//! ## OrderValidator (订单验证器)
//! - **职责**: 订单合规性和有效性验证
//! - **特性**: 可配置的验证规则链
//!
//! ## PriceImprovementStrategy (价格改善策略)
//! - **职责**: 撮合时的成交价格确定策略
//! - **特性**: 支持多种价格改善算法
//!
//! # 调用流程示例
//!
//! ```text
//! 取消订单流程:
//! 1. MatchingEngine::cancel_order()     ← 外部调用
//!    ├─ 业务验证 (权限、状态检查)
//!    ├─ OrderBookStore::cancel_order()  ← 数据层操作  
//!    ├─ Observer::on_order_cancelled()  ← 事件通知
//!    └─ 返回结果给调用方
//!
//! 处理订单流程:
//! 1. MatchingEngine::process_order()
//!    ├─ OrderValidator::validate_order() 
//!    ├─ Observer::on_before_match_attempt()
//!    ├─ OrderBookStore 的多个方法调用
//!    ├─ PriceImprovementStrategy::determine_trade_price()
//!    ├─ Observer::on_trade_generated()
//!    └─ Observer::on_after_order_processed()
//! ```

use crate::errors::*;
use crate::types::*;
use async_trait::async_trait;
use std::sync::Arc;

/// 订单簿存储抽象接口
/// 
/// 该 trait 定义了撮合引擎与底层内存数据库（用于存储订单簿和即时行情）交互的契约。
/// 这使得引擎不直接依赖于特定数据库实现（如 Redis）。
#[async_trait]
pub trait OrderBookStore: Send + Sync {
    /// 提交新订单到指定股票的订单簿
    async fn submit_order(&mut self, order: &Order) -> OrderBookStoreResult<()>;

    /// 尝试取消订单簿中的某个订单
    /// 返回被成功取消的订单详情，如果订单不存在或无法取消则返回错误。
    async fn cancel_order(
        &mut self,
        stock_id: &StockId,
        order_id: &OrderId,
    ) -> OrderBookStoreResult<Order>;

    /// 获取指定股票的订单簿快照（例如，买卖各 N 档深度）
    async fn get_order_book_snapshot(
        &self,
        stock_id: &StockId,
        depth: usize,
    ) -> OrderBookStoreResult<OrderBookView>;

    /// 获取指定股票的最优买一价/量和卖一价/量
    /// 返回 (Option<OrderBookEntry>, Option<OrderBookEntry>) 代表 (BestBid, BestAsk)
    async fn get_best_bid_ask(
        &self,
        stock_id: &StockId,
    ) -> OrderBookStoreResult<(Option<OrderBookEntry>, Option<OrderBookEntry>)>;

    /// 根据订单ID获取订单详情
    async fn get_order_by_id(
        &self,
        stock_id: &StockId,
        order_id: &OrderId,
    ) -> OrderBookStoreResult<Option<Order>>;

    /// 更新内存中的市场行情tick（可选，根据行情数据来源）
    async fn update_market_tick(&mut self, tick: &MarketTick) -> OrderBookStoreResult<()>;

    /// 根据成交结果更新订单簿（移除已成交部分，修改部分成交订单）
    /// 需要传入成交的买单ID、卖单ID以及成交数量和价格
    async fn update_order_book_after_trade(
        &mut self,
        stock_id: &StockId,
        trade_confirmation: &TradeConfirmation,
    ) -> OrderBookStoreResult<()>;

    /// 获取订单簿中指定价格层级的所有订单（按时间排序）
    async fn get_orders_at_price_level(
        &self,
        stock_id: &StockId,
        side: OrderSide,
        price: Price,
    ) -> OrderBookStoreResult<Vec<Order>>;

    /// 检查订单簿是否为空
    async fn is_order_book_empty(&self, stock_id: &StockId) -> OrderBookStoreResult<bool>;

    /// 获取订单簿的统计信息
    async fn get_order_book_stats(
        &self,
        stock_id: &StockId,
    ) -> OrderBookStoreResult<OrderBookStats>;
}

/// 撮合生命周期观察者/钩子接口
/// 
/// 该 trait 允许外部模块在撮合生命周期的关键节点注入逻辑，而无需修改撮合引擎核心。
#[async_trait]
pub trait MatchLifecycleObserver: Send + Sync {
    /// 订单尝试匹配前调用
    async fn on_before_match_attempt(
        &self,
        order: &Order,
        current_order_book: &OrderBookView,
    ) -> MatchLifecycleObserverResult<()>;

    /// 订单成功匹配并产生交易时调用 (可能多次，如果一个订单与多个对手方订单成交)
    async fn on_trade_generated(
        &self,
        trade: &Trade,
        remaining_aggressor_order_qty: Quantity,
    ) -> MatchLifecycleObserverResult<()>;

    /// 订单处理完成（完全成交、部分成交或未成交）后调用
    async fn on_after_order_processed(
        &self,
        order_id: &OrderId,
        final_status: OrderStatus,
    ) -> MatchLifecycleObserverResult<()>;

    /// 订单被取消时调用
    async fn on_order_cancelled(
        &self,
        order: &Order,
        cancellation_reason: &str,
    ) -> MatchLifecycleObserverResult<()>;

    /// 订单被拒绝时调用
    async fn on_order_rejected(
        &self,
        order: &Order,
        rejection_reason: &str,
    ) -> MatchLifecycleObserverResult<()>;

    /// 撮合引擎启动时调用
    async fn on_engine_started(&self) -> MatchLifecycleObserverResult<()>;

    /// 撮合引擎停止时调用
    async fn on_engine_stopped(&self) -> MatchLifecycleObserverResult<()>;
}

/// 撮合引擎核心接口
/// 
/// 定义了撮合引擎的核心功能接口
#[async_trait]
pub trait MatchingEngine: Send + Sync {
    /// 处理新订单
    async fn process_order(&mut self, order: Order) -> MatchingEngineResult<Vec<Trade>>;

    /// 取消订单
    async fn cancel_order(
        &mut self,
        stock_id: &StockId,
        order_id: &OrderId,
    ) -> MatchingEngineResult<Order>;

    /// 获取订单簿快照
    async fn get_order_book_snapshot(
        &self,
        stock_id: &StockId,
        depth: usize,
    ) -> MatchingEngineResult<OrderBookView>;

    /// 获取撮合统计信息
    async fn get_matching_statistics(&self) -> MatchingEngineResult<MatchingStatistics>;

    /// 启动撮合引擎
    async fn start(&mut self) -> MatchingEngineResult<()>;

    /// 停止撮合引擎
    async fn stop(&mut self) -> MatchingEngineResult<()>;

    /// 检查引擎是否运行中
    fn is_running(&self) -> bool;
}

/// 订单验证器接口
/// 
/// 用于在订单进入撮合前进行验证
pub trait OrderValidator: Send + Sync {
    /// 验证订单是否有效
    fn validate_order(&self, order: &Order) -> MatchingEngineResult<()>;

    /// 验证价格是否有效
    fn validate_price(&self, price: Price) -> MatchingEngineResult<()>;

    /// 验证数量是否有效
    fn validate_quantity(&self, quantity: Quantity) -> MatchingEngineResult<()>;

    /// 验证股票代码是否有效
    fn validate_stock_id(&self, stock_id: &StockId) -> MatchingEngineResult<()>;
}

/// 价格改善策略接口
/// 
/// 定义订单匹配时的价格确定策略
pub trait PriceImprovementStrategy: Send + Sync {
    /// 确定成交价格
    /// 
    /// # 参数
    /// * `aggressor_order` - 主动方订单
    /// * `resting_order` - 被动方订单
    /// * `market_context` - 市场上下文信息
    /// 
    /// # 返回
    /// 确定的成交价格
    fn determine_trade_price(
        &self,
        aggressor_order: &Order,
        resting_order: &Order,
        market_context: &MarketContext,
    ) -> MatchingEngineResult<Price>;
}

/// 市场上下文信息
#[derive(Debug, Clone, PartialEq)]
pub struct MarketContext {
    /// 股票代码
    pub stock_id: StockId,
    /// 当前最优买价
    pub best_bid: Option<Price>,
    /// 当前最优卖价
    pub best_ask: Option<Price>,
    /// 最近成交价
    pub last_trade_price: Option<Price>,
    /// 上下文时间戳
    pub timestamp: Timestamp,
}

/// 订单簿统计信息
#[derive(Debug, Clone, PartialEq)]
pub struct OrderBookStats {
    /// 买单总数
    pub total_bid_orders: usize,
    /// 卖单总数
    pub total_ask_orders: usize,
    /// 买单总量
    pub total_bid_quantity: Quantity,
    /// 卖单总量
    pub total_ask_quantity: Quantity,
    /// 价格层级数（买单）
    pub bid_price_levels: usize,
    /// 价格层级数（卖单）
    pub ask_price_levels: usize,
    /// 最大买卖价差
    pub max_spread: Option<Price>,
    /// 最小买卖价差
    pub min_spread: Option<Price>,
}

/// 观察者容器 - 使用Arc来避免Box<dyn Trait>的object safety问题
pub struct ObserverContainer {
    observers: Vec<Arc<dyn MatchLifecycleObserver>>,
}

impl ObserverContainer {
    /// 创建新的观察者容器
    pub fn new() -> Self {
        Self {
            observers: Vec::new(),
        }
    }

    /// 添加观察者
    pub fn add_observer(&mut self, observer: Arc<dyn MatchLifecycleObserver>) {
        self.observers.push(observer);
    }

    /// 移除所有观察者
    pub fn clear_observers(&mut self) {
        self.observers.clear();
    }

    /// 获取观察者数量
    pub fn observer_count(&self) -> usize {
        self.observers.len()
    }

    /// 通知所有观察者 - 订单匹配前
    pub async fn notify_before_match_attempt(
        &self,
        order: &Order,
        current_order_book: &OrderBookView,
    ) -> MatchLifecycleObserverResult<()> {
        for observer in &self.observers {
            observer
                .on_before_match_attempt(order, current_order_book)
                .await?;
        }
        Ok(())
    }

    /// 通知所有观察者 - 交易生成
    pub async fn notify_trade_generated(
        &self,
        trade: &Trade,
        remaining_aggressor_order_qty: Quantity,
    ) -> MatchLifecycleObserverResult<()> {
        for observer in &self.observers {
            observer
                .on_trade_generated(trade, remaining_aggressor_order_qty)
                .await?;
        }
        Ok(())
    }

    /// 通知所有观察者 - 订单处理完成
    pub async fn notify_after_order_processed(
        &self,
        order_id: &OrderId,
        final_status: OrderStatus,
    ) -> MatchLifecycleObserverResult<()> {
        for observer in &self.observers {
            observer
                .on_after_order_processed(order_id, final_status)
                .await?;
        }
        Ok(())
    }

    /// 通知所有观察者 - 订单取消
    pub async fn notify_order_cancelled(
        &self,
        order: &Order,
        cancellation_reason: &str,
    ) -> MatchLifecycleObserverResult<()> {
        for observer in &self.observers {
            observer
                .on_order_cancelled(order, cancellation_reason)
                .await?;
        }
        Ok(())
    }

    /// 通知所有观察者 - 订单拒绝
    pub async fn notify_order_rejected(
        &self,
        order: &Order,
        rejection_reason: &str,
    ) -> MatchLifecycleObserverResult<()> {
        for observer in &self.observers {
            observer
                .on_order_rejected(order, rejection_reason)
                .await?;
        }
        Ok(())
    }

    /// 通知所有观察者 - 引擎启动
    pub async fn notify_engine_started(&self) -> MatchLifecycleObserverResult<()> {
        for observer in &self.observers {
            observer.on_engine_started().await?;
        }
        Ok(())
    }

    /// 通知所有观察者 - 引擎停止
    pub async fn notify_engine_stopped(&self) -> MatchLifecycleObserverResult<()> {
        for observer in &self.observers {
            observer.on_engine_stopped().await?;
        }
        Ok(())
    }
}

impl Default for ObserverContainer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::sync::Mutex;

    #[test]
    fn test_observer_container_creation() {
        let container = ObserverContainer::new();
        assert_eq!(container.observer_count(), 0);
    }

    #[test]
    fn test_market_context_creation() {
        let context = MarketContext {
            stock_id: "SH600036".to_string(),
            best_bid: Some(dec!(10.50)),
            best_ask: Some(dec!(10.51)),
            last_trade_price: Some(dec!(10.50)),
            timestamp: chrono::Utc::now(),
        };

        assert_eq!(context.stock_id, "SH600036");
        assert_eq!(context.best_bid, Some(dec!(10.50)));
        assert_eq!(context.best_ask, Some(dec!(10.51)));
    }

    #[test]
    fn test_order_book_stats_creation() {
        let stats = OrderBookStats {
            total_bid_orders: 10,
            total_ask_orders: 8,
            total_bid_quantity: 5000,
            total_ask_quantity: 4000,
            bid_price_levels: 5,
            ask_price_levels: 4,
            max_spread: Some(dec!(0.02)),
            min_spread: Some(dec!(0.01)),
        };

        assert_eq!(stats.total_bid_orders, 10);
        assert_eq!(stats.total_ask_orders, 8);
        assert_eq!(stats.total_bid_quantity, 5000);
        assert_eq!(stats.total_ask_quantity, 4000);
    }

    // 模拟测试用的观察者实现
    struct MockObserver {
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl MockObserver {
        fn new() -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl MatchLifecycleObserver for MockObserver {
        async fn on_before_match_attempt(
            &self,
            _order: &Order,
            _current_order_book: &OrderBookView,
        ) -> MatchLifecycleObserverResult<()> {
            self.calls
                .lock()
                .unwrap()
                .push("on_before_match_attempt".to_string());
            Ok(())
        }

        async fn on_trade_generated(
            &self,
            _trade: &Trade,
            _remaining_aggressor_order_qty: Quantity,
        ) -> MatchLifecycleObserverResult<()> {
            self.calls
                .lock()
                .unwrap()
                .push("on_trade_generated".to_string());
            Ok(())
        }

        async fn on_after_order_processed(
            &self,
            _order_id: &OrderId,
            _final_status: OrderStatus,
        ) -> MatchLifecycleObserverResult<()> {
            self.calls
                .lock()
                .unwrap()
                .push("on_after_order_processed".to_string());
            Ok(())
        }

        async fn on_order_cancelled(
            &self,
            _order: &Order,
            _cancellation_reason: &str,
        ) -> MatchLifecycleObserverResult<()> {
            self.calls
                .lock()
                .unwrap()
                .push("on_order_cancelled".to_string());
            Ok(())
        }

        async fn on_order_rejected(
            &self,
            _order: &Order,
            _rejection_reason: &str,
        ) -> MatchLifecycleObserverResult<()> {
            self.calls
                .lock()
                .unwrap()
                .push("on_order_rejected".to_string());
            Ok(())
        }

        async fn on_engine_started(&self) -> MatchLifecycleObserverResult<()> {
            self.calls
                .lock()
                .unwrap()
                .push("on_engine_started".to_string());
            Ok(())
        }

        async fn on_engine_stopped(&self) -> MatchLifecycleObserverResult<()> {
            self.calls
                .lock()
                .unwrap()
                .push("on_engine_stopped".to_string());
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_observer_container_functionality() {
        let mut container = ObserverContainer::new();
        let mock1 = Arc::new(MockObserver::new());
        let mock2 = Arc::new(MockObserver::new());

        let calls1 = mock1.calls.clone();
        let calls2 = mock2.calls.clone();

        container.add_observer(mock1);
        container.add_observer(mock2);

        assert_eq!(container.observer_count(), 2);

        // 测试引擎启动事件
        container.notify_engine_started().await.unwrap();

        let calls1_result = calls1.lock().unwrap().clone();
        let calls2_result = calls2.lock().unwrap().clone();

        assert_eq!(calls1_result, vec!["on_engine_started"]);
        assert_eq!(calls2_result, vec!["on_engine_started"]);
    }

    #[tokio::test]
    async fn test_multiple_observer_notifications() {
        let mut container = ObserverContainer::new();
        let mock = Arc::new(MockObserver::new());
        let calls = mock.calls.clone();

        container.add_observer(mock);

        // 测试多个事件通知
        container.notify_engine_started().await.unwrap();
        
        // 创建测试订单和订单簿视图
        let order = Order::new_limit_order(
            "test_order".to_string(),
            "SH600036".to_string(),
            "test_user".to_string(),
            OrderSide::Buy,
            dec!(10.50),
            1000,
            chrono::Utc::now(),
        );
        
        let order_book = OrderBookView::new(
            "SH600036".to_string(),
            chrono::Utc::now(),
        );

        container
            .notify_before_match_attempt(&order, &order_book)
            .await
            .unwrap();
            
        container
            .notify_after_order_processed(&order.id, OrderStatus::New)
            .await
            .unwrap();

        container.notify_engine_stopped().await.unwrap();

        let calls_result = calls.lock().unwrap().clone();
        assert_eq!(
            calls_result,
            vec![
                "on_engine_started",
                "on_before_match_attempt",
                "on_after_order_processed",
                "on_engine_stopped"
            ]
        );
    }
} 