//! 撮合引擎核心实现

use crate::errors::*;
use crate::traits::*;
use crate::types::*;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// 默认撮合引擎实现
pub struct DefaultMatchingEngine<S: OrderBookStore> {
    /// 订单簿存储
    order_book_store: Arc<RwLock<S>>,
    /// 观察者容器
    observers: ObserverContainer,
    /// 订单验证器
    validator: Option<Arc<dyn OrderValidator>>,
    /// 价格改善策略
    price_strategy: Option<Arc<dyn PriceImprovementStrategy>>,
    /// 撮合统计信息
    statistics: Arc<RwLock<MatchingStatistics>>,
    /// 引擎运行状态
    is_running: Arc<RwLock<bool>>,
}

impl<S: OrderBookStore> DefaultMatchingEngine<S> {
    /// 创建新的撮合引擎实例
    pub fn new(order_book_store: S) -> Self {
        Self {
            order_book_store: Arc::new(RwLock::new(order_book_store)),
            observers: ObserverContainer::new(),
            validator: None,
            price_strategy: None,
            statistics: Arc::new(RwLock::new(MatchingStatistics {
                total_orders_processed: 0,
                total_trades_generated: 0,
                fully_filled_orders: 0,
                partially_filled_orders: 0,
                cancelled_orders: 0,
                per_stock_stats: HashMap::new(),
            })),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// 设置订单验证器
    pub fn with_validator(mut self, validator: Arc<dyn OrderValidator>) -> Self {
        self.validator = Some(validator);
        self
    }

    /// 设置价格改善策略
    pub fn with_price_strategy(mut self, strategy: Arc<dyn PriceImprovementStrategy>) -> Self {
        self.price_strategy = Some(strategy);
        self
    }

    /// 添加观察者
    pub fn add_observer(&mut self, observer: Arc<dyn MatchLifecycleObserver>) {
        self.observers.add_observer(observer);
    }

    /// 生成交易ID
    fn generate_trade_id() -> TradeId {
        Uuid::new_v4().to_string()
    }

    /// 确定成交价格
    fn determine_trade_price(
        &self,
        _aggressor_order: &Order,
        resting_order: &Order,
        _market_context: &MarketContext,
    ) -> MatchingEngineResult<Price> {
        // 根据设计文档，成交价为被动方订单的价格
        Ok(resting_order.price)
    }

    /// 创建交易记录
    fn create_trade(
        &self,
        aggressor_order: &Order,
        resting_order: &Order,
        trade_price: Price,
        trade_quantity: Quantity,
        timestamp: Timestamp,
    ) -> Trade {
        let (buyer_order_id, seller_order_id, buyer_user_id, seller_user_id) = 
            match aggressor_order.side {
                OrderSide::Buy => (
                    aggressor_order.id.clone(),
                    resting_order.id.clone(),
                    aggressor_order.user_id.clone(),
                    resting_order.user_id.clone(),
                ),
                OrderSide::Sell => (
                    resting_order.id.clone(),
                    aggressor_order.id.clone(),
                    resting_order.user_id.clone(),
                    aggressor_order.user_id.clone(),
                ),
            };

        Trade {
            id: Self::generate_trade_id(),
            stock_id: aggressor_order.stock_id.clone(),
            price: trade_price,
            quantity: trade_quantity,
            timestamp,
            aggressor_order_id: aggressor_order.id.clone(),
            resting_order_id: resting_order.id.clone(),
            buyer_order_id,
            seller_order_id,
            buyer_user_id,
            seller_user_id,
        }
    }

    /// 更新统计信息
    async fn update_statistics(&self, trade: &Trade, order_status: OrderStatus) -> MatchingEngineResult<()> {
        let mut stats = self.statistics.write().await;
        
        stats.total_trades_generated += 1;
        
        match order_status {
            OrderStatus::Filled => stats.fully_filled_orders += 1,
            OrderStatus::PartiallyFilled => stats.partially_filled_orders += 1,
            OrderStatus::Cancelled => stats.cancelled_orders += 1,
            _ => {}
        }

        // 更新股票级别统计
        let stock_stats = stats.per_stock_stats
            .entry(trade.stock_id.clone())
            .or_insert_with(|| StockMatchingStats {
                orders_processed: 0,
                trades_generated: 0,
                total_volume: 0,
                total_turnover: rust_decimal::Decimal::ZERO,
                last_price: None,
            });

        stock_stats.trades_generated += 1;
        stock_stats.total_volume += trade.quantity;
        stock_stats.total_turnover += trade.price * rust_decimal::Decimal::from(trade.quantity);
        stock_stats.last_price = Some(trade.price);

        Ok(())
    }
}

#[async_trait]
impl<S: OrderBookStore + 'static> MatchingEngine for DefaultMatchingEngine<S> {
    async fn process_order(&mut self, mut order: Order) -> MatchingEngineResult<Vec<Trade>> {
        // 验证订单
        if let Some(validator) = &self.validator {
            validator.validate_order(&order)?;
        }

        let mut trades = Vec::new();
        let mut store = self.order_book_store.write().await;
        
        // 获取当前订单簿快照
        let order_book = store
            .get_order_book_snapshot(&order.stock_id, 10)
            .await
            .map_err(|e| MatchingEngineError::StorageError { 
                reason: format!("获取订单簿快照失败: {:?}", e) 
            })?;

        // 通知观察者：订单匹配前
        self.observers
            .notify_before_match_attempt(&order, &order_book)
            .await
            .map_err(|e| MatchingEngineError::ObserverNotificationFailed { 
                reason: format!("观察者通知失败: {:?}", e) 
            })?;

        // 获取对手方最优价格
        let (best_bid, best_ask) = store
            .get_best_bid_ask(&order.stock_id)
            .await
            .map_err(|e| MatchingEngineError::StorageError { 
                reason: format!("获取最优买卖价失败: {:?}", e) 
            })?;

        // 根据订单方向确定匹配逻辑
        let can_match = match order.side {
            OrderSide::Buy => {
                // 买单：检查是否有可匹配的卖单
                if let Some(ask) = &best_ask {
                    order.price >= ask.price
                } else {
                    false
                }
            }
            OrderSide::Sell => {
                // 卖单：检查是否有可匹配的买单
                if let Some(bid) = &best_bid {
                    order.price <= bid.price
                } else {
                    false
                }
            }
        };

        if can_match {
            // 执行匹配逻辑
            let opposite_side = match order.side {
                OrderSide::Buy => OrderSide::Sell,
                OrderSide::Sell => OrderSide::Buy,
            };

            // 获取对手方价格层级的订单
            let target_price = match order.side {
                OrderSide::Buy => best_ask.as_ref().unwrap().price,
                OrderSide::Sell => best_bid.as_ref().unwrap().price,
            };

            let mut resting_orders = store
                .get_orders_at_price_level(&order.stock_id, opposite_side, target_price)
                .await
                .map_err(|e| MatchingEngineError::StorageError { 
                    reason: format!("获取价格层级订单失败: {:?}", e) 
                })?;

            // 按时间优先排序（已经在存储层实现）
            for resting_order in &mut resting_orders {
                if order.unfilled_quantity == 0 {
                    break;
                }

                // 确定成交数量
                let trade_quantity = std::cmp::min(order.unfilled_quantity, resting_order.unfilled_quantity);
                
                // 确定成交价格
                let market_context = MarketContext {
                    stock_id: order.stock_id.clone(),
                    best_bid: best_bid.as_ref().map(|b| b.price),
                    best_ask: best_ask.as_ref().map(|a| a.price),
                    last_trade_price: None, // TODO: 从存储中获取
                    timestamp: chrono::Utc::now(),
                };

                let trade_price = self.determine_trade_price(&order, resting_order, &market_context)?;

                // 创建交易记录
                let trade = self.create_trade(
                    &order,
                    resting_order,
                    trade_price,
                    trade_quantity,
                    chrono::Utc::now(),
                );

                // 更新订单数量
                order.unfilled_quantity -= trade_quantity;
                resting_order.unfilled_quantity -= trade_quantity;

                // 更新订单状态
                if order.unfilled_quantity == 0 {
                    order.status = OrderStatus::Filled;
                } else {
                    order.status = OrderStatus::PartiallyFilled;
                }

                if resting_order.unfilled_quantity == 0 {
                    resting_order.status = OrderStatus::Filled;
                } else {
                    resting_order.status = OrderStatus::PartiallyFilled;
                }

                // 创建交易确认
                let trade_confirmation = TradeConfirmation {
                    trade: trade.clone(),
                    aggressor_remaining_quantity: order.unfilled_quantity,
                    resting_remaining_quantity: resting_order.unfilled_quantity,
                };

                // 更新订单簿
                store
                    .update_order_book_after_trade(&order.stock_id, &trade_confirmation)
                    .await
                    .map_err(|e| MatchingEngineError::StorageError { 
                        reason: format!("更新订单簿失败: {:?}", e) 
                    })?;

                // 通知观察者：交易生成
                self.observers
                    .notify_trade_generated(&trade, order.unfilled_quantity)
                    .await
                    .map_err(|e| MatchingEngineError::ObserverNotificationFailed { 
                        reason: format!("交易生成通知失败: {:?}", e) 
                    })?;

                // 更新统计信息
                self.update_statistics(&trade, order.status).await?;

                trades.push(trade);
            }
        }

        // 如果订单还有未成交部分，添加到订单簿
        if order.unfilled_quantity > 0 {
            store
                .submit_order(&order)
                .await
                .map_err(|e| MatchingEngineError::StorageError { 
                    reason: format!("提交订单到订单簿失败: {:?}", e) 
                })?;
        }

        // 通知观察者：订单处理完成
        self.observers
            .notify_after_order_processed(&order.id, order.status)
            .await
            .map_err(|e| MatchingEngineError::ObserverNotificationFailed { 
                reason: format!("订单处理完成通知失败: {:?}", e) 
            })?;

        // 更新总订单处理数
        {
            let mut stats = self.statistics.write().await;
            stats.total_orders_processed += 1;
        }

        Ok(trades)
    }

    async fn cancel_order(
        &mut self,
        stock_id: &StockId,
        order_id: &OrderId,
    ) -> MatchingEngineResult<Order> {
        let mut store = self.order_book_store.write().await;
        
        let cancelled_order = store
            .cancel_order(stock_id, order_id)
            .await
            .map_err(|e| MatchingEngineError::StorageError { 
                reason: format!("取消订单失败: {:?}", e) 
            })?;

        // 通知观察者：订单取消
        self.observers
            .notify_order_cancelled(&cancelled_order, "用户主动取消")
            .await
            .map_err(|e| MatchingEngineError::ObserverNotificationFailed { 
                reason: format!("订单取消通知失败: {:?}", e) 
            })?;

        // 更新统计信息
        {
            let mut stats = self.statistics.write().await;
            stats.cancelled_orders += 1;
        }

        Ok(cancelled_order)
    }

    async fn get_order_book_snapshot(
        &self,
        stock_id: &StockId,
        depth: usize,
    ) -> MatchingEngineResult<OrderBookView> {
        let store = self.order_book_store.read().await;
        
        store
            .get_order_book_snapshot(stock_id, depth)
            .await
            .map_err(|e| MatchingEngineError::StorageError { 
                reason: format!("获取订单簿快照失败: {:?}", e) 
            })
    }

    async fn get_matching_statistics(&self) -> MatchingEngineResult<MatchingStatistics> {
        let stats = self.statistics.read().await;
        Ok(stats.clone())
    }

    async fn start(&mut self) -> MatchingEngineResult<()> {
        let mut running = self.is_running.write().await;
        if *running {
            return Err(MatchingEngineError::InternalError { 
                reason: "引擎已经在运行中".to_string() 
            });
        }

        *running = true;

        // 通知观察者：引擎启动
        self.observers
            .notify_engine_started()
            .await
            .map_err(|e| MatchingEngineError::ObserverNotificationFailed { 
                reason: format!("引擎启动通知失败: {:?}", e) 
            })?;

        Ok(())
    }

    async fn stop(&mut self) -> MatchingEngineResult<()> {
        let mut running = self.is_running.write().await;
        if !*running {
            return Err(MatchingEngineError::InternalError { 
                reason: "引擎未在运行".to_string() 
            });
        }

        *running = false;

        // 通知观察者：引擎停止
        self.observers
            .notify_engine_stopped()
            .await
            .map_err(|e| MatchingEngineError::ObserverNotificationFailed { 
                reason: format!("引擎停止通知失败: {:?}", e) 
            })?;

        Ok(())
    }

    fn is_running(&self) -> bool {
        // 注意：这里使用try_read来避免阻塞，如果无法获取锁则返回false
        self.is_running.try_read().map(|guard| *guard).unwrap_or(false)
    }
}

/// 默认订单验证器实现
pub struct DefaultOrderValidator;

impl OrderValidator for DefaultOrderValidator {
    fn validate_order(&self, order: &Order) -> MatchingEngineResult<()> {
        self.validate_price(order.price)?;
        self.validate_quantity(order.quantity)?;
        self.validate_stock_id(&order.stock_id)?;

        if order.unfilled_quantity > order.quantity {
            return Err(MatchingEngineError::InvalidOrder {
                reason: "未成交数量不能大于总数量".to_string(),
            });
        }

        Ok(())
    }

    fn validate_price(&self, price: Price) -> MatchingEngineResult<()> {
        if price <= rust_decimal::Decimal::ZERO {
            return Err(MatchingEngineError::InvalidOrder {
                reason: "价格必须大于0".to_string(),
            });
        }
        Ok(())
    }

    fn validate_quantity(&self, quantity: Quantity) -> MatchingEngineResult<()> {
        if quantity == 0 {
            return Err(MatchingEngineError::InvalidOrder {
                reason: "数量必须大于0".to_string(),
            });
        }
        Ok(())
    }

    fn validate_stock_id(&self, stock_id: &StockId) -> MatchingEngineResult<()> {
        if stock_id.is_empty() {
            return Err(MatchingEngineError::InvalidOrder {
                reason: "股票代码不能为空".to_string(),
            });
        }
        Ok(())
    }
}

/// 默认价格改善策略实现（被动方价格优先）
pub struct DefaultPriceImprovementStrategy;

impl PriceImprovementStrategy for DefaultPriceImprovementStrategy {
    fn determine_trade_price(
        &self,
        _aggressor_order: &Order,
        resting_order: &Order,
        _market_context: &MarketContext,
    ) -> MatchingEngineResult<Price> {
        // 根据设计文档，成交价为被动方订单的价格
        Ok(resting_order.price)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::sync::Mutex;

    // 模拟订单簿存储实现
    struct MockOrderBookStore {
        orders: Arc<Mutex<HashMap<(StockId, OrderId), Order>>>,
        order_books: Arc<Mutex<HashMap<StockId, OrderBookView>>>,
    }

    impl MockOrderBookStore {
        fn new() -> Self {
            Self {
                orders: Arc::new(Mutex::new(HashMap::new())),
                order_books: Arc::new(Mutex::new(HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl OrderBookStore for MockOrderBookStore {
        async fn submit_order(&mut self, order: &Order) -> OrderBookStoreResult<()> {
            let mut orders = self.orders.lock().unwrap();
            orders.insert((order.stock_id.clone(), order.id.clone()), order.clone());
            Ok(())
        }

        async fn cancel_order(
            &mut self,
            stock_id: &StockId,
            order_id: &OrderId,
        ) -> OrderBookStoreResult<Order> {
            let mut orders = self.orders.lock().unwrap();
            let key = (stock_id.clone(), order_id.clone());
            
            if let Some(mut order) = orders.remove(&key) {
                order.status = OrderStatus::Cancelled;
                Ok(order)
            } else {
                Err(OrderBookStoreError::DataNotFound {
                    key: format!("{}:{}", stock_id, order_id),
                })
            }
        }

        async fn get_order_book_snapshot(
            &self,
            stock_id: &StockId,
            _depth: usize,
        ) -> OrderBookStoreResult<OrderBookView> {
            let order_books = self.order_books.lock().unwrap();
            Ok(order_books
                .get(stock_id)
                .cloned()
                .unwrap_or_else(|| OrderBookView::new(stock_id.clone(), chrono::Utc::now())))
        }

        async fn get_best_bid_ask(
            &self,
            _stock_id: &StockId,
        ) -> OrderBookStoreResult<(Option<OrderBookEntry>, Option<OrderBookEntry>)> {
            // 简化实现：返回模拟数据
            Ok((
                Some(OrderBookEntry {
                    price: dec!(10.00),
                    total_quantity: 1000,
                    order_count: 1,
                }),
                Some(OrderBookEntry {
                    price: dec!(10.01),
                    total_quantity: 800,
                    order_count: 1,
                }),
            ))
        }

        async fn get_order_by_id(
            &self,
            stock_id: &StockId,
            order_id: &OrderId,
        ) -> OrderBookStoreResult<Option<Order>> {
            let orders = self.orders.lock().unwrap();
            Ok(orders.get(&(stock_id.clone(), order_id.clone())).cloned())
        }

        async fn update_market_tick(&mut self, _tick: &MarketTick) -> OrderBookStoreResult<()> {
            Ok(())
        }

        async fn update_order_book_after_trade(
            &mut self,
            _stock_id: &StockId,
            _trade_confirmation: &TradeConfirmation,
        ) -> OrderBookStoreResult<()> {
            Ok(())
        }

        async fn get_orders_at_price_level(
            &self,
            _stock_id: &StockId,
            _side: OrderSide,
            _price: Price,
        ) -> OrderBookStoreResult<Vec<Order>> {
            // 简化实现：返回一个模拟订单
            Ok(vec![Order::new_limit_order(
                "resting_order".to_string(),
                "SH600036".to_string(),
                "user_002".to_string(),
                OrderSide::Sell,
                dec!(10.01),
                500,
                chrono::Utc::now(),
            )])
        }

        async fn is_order_book_empty(&self, _stock_id: &StockId) -> OrderBookStoreResult<bool> {
            Ok(false)
        }

        async fn get_order_book_stats(
            &self,
            _stock_id: &StockId,
        ) -> OrderBookStoreResult<OrderBookStats> {
            Ok(OrderBookStats {
                total_bid_orders: 0,
                total_ask_orders: 0,
                total_bid_quantity: 0,
                total_ask_quantity: 0,
                bid_price_levels: 0,
                ask_price_levels: 0,
                max_spread: None,
                min_spread: None,
            })
        }
    }

    #[tokio::test]
    async fn test_engine_creation() {
        let store = MockOrderBookStore::new();
        let engine = DefaultMatchingEngine::new(store);
        
        assert!(!engine.is_running());
    }

    #[tokio::test]
    async fn test_engine_start_stop() {
        let store = MockOrderBookStore::new();
        let mut engine = DefaultMatchingEngine::new(store);
        
        // 启动引擎
        engine.start().await.unwrap();
        assert!(engine.is_running());
        
        // 停止引擎
        engine.stop().await.unwrap();
        assert!(!engine.is_running());
    }

    #[tokio::test]
    async fn test_order_validation() {
        let validator = DefaultOrderValidator;
        
        // 有效订单
        let valid_order = Order::new_limit_order(
            "order_001".to_string(),
            "SH600036".to_string(),
            "user_001".to_string(),
            OrderSide::Buy,
            dec!(10.50),
            1000,
            chrono::Utc::now(),
        );
        
        assert!(validator.validate_order(&valid_order).is_ok());
        
        // 无效价格
        let invalid_price_order = Order::new_limit_order(
            "order_002".to_string(),
            "SH600036".to_string(),
            "user_001".to_string(),
            OrderSide::Buy,
            dec!(0.0),
            1000,
            chrono::Utc::now(),
        );
        
        assert!(validator.validate_order(&invalid_price_order).is_err());
        
        // 无效数量
        let invalid_quantity_order = Order::new_limit_order(
            "order_003".to_string(),
            "SH600036".to_string(),
            "user_001".to_string(),
            OrderSide::Buy,
            dec!(10.50),
            0,
            chrono::Utc::now(),
        );
        
        assert!(validator.validate_order(&invalid_quantity_order).is_err());
    }

    #[tokio::test]
    async fn test_price_improvement_strategy() {
        let strategy = DefaultPriceImprovementStrategy;
        
        let aggressor_order = Order::new_limit_order(
            "aggressor".to_string(),
            "SH600036".to_string(),
            "user_001".to_string(),
            OrderSide::Buy,
            dec!(10.50),
            1000,
            chrono::Utc::now(),
        );
        
        let resting_order = Order::new_limit_order(
            "resting".to_string(),
            "SH600036".to_string(),
            "user_002".to_string(),
            OrderSide::Sell,
            dec!(10.45),
            500,
            chrono::Utc::now(),
        );
        
        let market_context = MarketContext {
            stock_id: "SH600036".to_string(),
            best_bid: Some(dec!(10.40)),
            best_ask: Some(dec!(10.45)),
            last_trade_price: Some(dec!(10.42)),
            timestamp: chrono::Utc::now(),
        };
        
        let trade_price = strategy
            .determine_trade_price(&aggressor_order, &resting_order, &market_context)
            .unwrap();
            
        // 应该使用被动方订单的价格
        assert_eq!(trade_price, dec!(10.45));
    }

    #[tokio::test]
    async fn test_order_processing() {
        let store = MockOrderBookStore::new();
        let validator = Arc::new(DefaultOrderValidator);
        let mut engine = DefaultMatchingEngine::new(store)
            .with_validator(validator);
        
        engine.start().await.unwrap();
        
        let order = Order::new_limit_order(
            "test_order".to_string(),
            "SH600036".to_string(),
            "user_001".to_string(),
            OrderSide::Buy,
            dec!(10.50),
            1000,
            chrono::Utc::now(),
        );
        
        let trades = engine.process_order(order).await.unwrap();
        
        // 在这个简化的测试中，应该产生一笔交易
        assert_eq!(trades.len(), 1);
        
        let stats = engine.get_matching_statistics().await.unwrap();
        assert_eq!(stats.total_orders_processed, 1);
        assert_eq!(stats.total_trades_generated, 1);
    }
} 