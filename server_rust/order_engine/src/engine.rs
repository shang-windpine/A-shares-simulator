use crate::{Order, OrderNotification, OrderStatus, OrderValidator};
use crate::order_pool::{OrderPool, OrderPoolStats};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use core_entities::app_config::OrderEngineConfig;
use crate::service::OrderEngineService;

// 使用core_entities中的MatchNotification
use core_entities::MatchNotification;

/// 订单引擎核心服务
/// 负责管理订单池，处理订单请求，与匹配引擎通信
/// 
/// 使用内部可变性设计，支持在Arc中被启动和停止
pub struct OrderEngine {
    /// 订单池
    order_pool: Arc<OrderPool>,
    
    /// 订单验证器
    validator: Option<Arc<dyn OrderValidator>>,
    
    /// 向匹配引擎发送订单通知的channel
    order_notification_tx: mpsc::Sender<OrderNotification>,
    
    /// 配置
    config: Arc<OrderEngineConfig>,
    
    /// 内部状态（使用Mutex保护）
    inner: Arc<Mutex<OrderEngineInner>>,
}

/// OrderEngine的内部可变状态
struct OrderEngineInner {
    /// 从匹配引擎接收匹配结果的channel
    match_notification_rx: Option<mpsc::Receiver<MatchNotification>>,
    
    /// 主任务句柄
    main_task_handle: Option<JoinHandle<()>>,
    
    /// 清理任务句柄
    cleanup_task_handle: Option<JoinHandle<()>>,
    
    /// 是否正在运行
    is_running: bool,
}

impl OrderEngine {
    /// 创建新的订单引擎
    pub fn new(
        order_notification_tx: mpsc::Sender<OrderNotification>,
        match_notification_rx: mpsc::Receiver<MatchNotification>,
        validator: Option<Arc<dyn OrderValidator>>,
        config: Arc<OrderEngineConfig>,
    ) -> Self {
        let inner = OrderEngineInner {
            match_notification_rx: Some(match_notification_rx),
            main_task_handle: None,
            cleanup_task_handle: None,
            is_running: false,
        };

        Self {
            order_pool: Arc::new(OrderPool::new()),
            validator,
            order_notification_tx,
            config,
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    /// 启动订单引擎
    pub async fn start(&self) -> Result<(), String> {
        let mut inner = self.inner.lock().await;
        
        if inner.is_running {
            return Err("Order engine is already running".to_string());
        }

        info!("Starting order engine...");

        // 取出接收器用于主任务
        let match_notification_rx = inner.match_notification_rx.take()
            .ok_or("Match notification receiver already taken")?;

        // 启动清理任务
        let cleanup_handle = self.start_cleanup_task().await;
        inner.cleanup_task_handle = Some(cleanup_handle);

        // 启动主处理任务
        let order_pool = Arc::clone(&self.order_pool);
        let main_handle = tokio::spawn(async move {
            Self::run_main_loop(match_notification_rx, order_pool).await;
        });
        inner.main_task_handle = Some(main_handle);

        inner.is_running = true;
        info!("Order engine started");
        Ok(())
    }

    /// 停止订单引擎
    pub async fn stop(&self) -> Result<(), String> {
        let mut inner = self.inner.lock().await;
        
        if !inner.is_running {
            return Ok(());
        }

        info!("Stopping order engine...");

        // 停止主任务
        if let Some(handle) = inner.main_task_handle.take() {
            handle.abort();
            match handle.await {
                Ok(_) => info!("Order engine main task stopped normally"),
                Err(e) if e.is_cancelled() => info!("Order engine main task was cancelled"),
                Err(e) => warn!("Order engine main task stopped with error: {}", e),
            }
        }

        // 停止清理任务
        if let Some(handle) = inner.cleanup_task_handle.take() {
            handle.abort();
            match handle.await {
                Ok(_) => info!("Order engine cleanup task stopped normally"),
                Err(e) if e.is_cancelled() => info!("Order engine cleanup task was cancelled"),
                Err(e) => warn!("Order engine cleanup task stopped with error: {}", e),
            }
        }

        inner.is_running = false;
        info!("Order engine stopped");
        Ok(())
    }

    /// 检查引擎是否正在运行
    pub async fn is_running(&self) -> bool {
        let inner = self.inner.lock().await;
        inner.is_running
    }

    /// 提交新订单
    pub async fn submit_order(&self, mut order: Order) -> Result<String, String> {
        if !self.is_running().await {
            return Err("Order engine is not running".to_string());
        }

        // 如果没有订单ID，生成一个
        if order.order_id.is_empty() {
            order.order_id = Uuid::new_v4().to_string().into();
        }

        info!("Submitting order: {}", order.order_id);

        // 验证订单
        if let Some(validator) = &self.validator {
            if let Err(e) = validator.validate_order(&order) {
                warn!("Order validation failed for {}: {}", order.order_id, e);
                return Err(format!("Order validation failed: {}", e));
            }
        }

        let order_id = order.order_id.to_string();

        // 添加到订单池
        self.order_pool.add_order(order.clone())?;

        // 发送到匹配引擎
        let notification = OrderNotification::NewOrder(order);
        if let Err(e) = self.order_notification_tx.send(notification).await {
            error!("Failed to send order notification: {}", e);
            // 回滚：从订单池移除
            let _ = self.order_pool.cancel_order(&order_id);
            return Err("Failed to send order to matching engine".to_string());
        }

        info!("Successfully submitted order: {}", order_id);
        Ok(order_id)
    }

    /// 取消订单
    pub async fn cancel_order(&self, order_id: &str, stock_id: &str) -> Result<(), String> {
        if !self.is_running().await {
            return Err("Order engine is not running".to_string());
        }

        info!("Cancelling order: {}", order_id);

        // 检查订单是否存在于池中
        if self.order_pool.get_order(order_id).is_none() {
            return Err(format!("Order {} not found", order_id));
        }

        // 发送取消请求到匹配引擎
        let notification = OrderNotification::CancelOrder {
            order_id: order_id.into(),
            stock_id: stock_id.into(),
        };

        if let Err(e) = self.order_notification_tx.send(notification).await {
            error!("Failed to send cancel notification: {}", e);
            return Err("Failed to send cancel request to matching engine".to_string());
        }

        info!("Successfully sent cancel request for order: {}", order_id);
        Ok(())
    }

    /// 获取订单信息
    pub fn get_order(&self, order_id: &str) -> Option<Order> {
        self.order_pool
            .get_order(order_id)
            .map(|order_arc| order_arc.read().clone())
    }

    /// 获取用户的所有订单
    pub fn get_user_orders(&self, user_id: &str) -> Vec<Order> {
        self.order_pool
            .get_orders_by_user(user_id)
            .into_iter()
            .map(|order_arc| order_arc.read().clone())
            .collect()
    }

    /// 获取股票的所有订单
    pub fn get_stock_orders(&self, stock_id: &str) -> Vec<Order> {
        self.order_pool
            .get_orders_by_stock(stock_id)
            .into_iter()
            .map(|order_arc| order_arc.read().clone())
            .collect()
    }

    /// 获取所有活跃订单
    pub fn get_active_orders(&self) -> Vec<Order> {
        self.order_pool
            .get_active_orders()
            .into_iter()
            .map(|order_arc| order_arc.read().clone())
            .collect()
    }

    /// 获取订单池统计信息
    pub fn get_stats(&self) -> OrderPoolStats {
        self.order_pool.get_stats()
    }

    /// 获取订单池引用（仅用于测试）
    #[cfg(test)]
    pub fn get_order_pool(&self) -> &OrderPool {
        &self.order_pool
    }

    /// 主运行循环（静态方法，在独立任务中运行）
    async fn run_main_loop(
        mut match_notification_rx: mpsc::Receiver<MatchNotification>,
        order_pool: Arc<OrderPool>,
    ) {
        info!("Order engine main loop started");

        while let Some(notification) = match_notification_rx.recv().await {
            if let Err(e) = Self::handle_match_notification_static(notification, &order_pool).await {
                error!("Error handling match notification: {}", e);
            }
        }

        info!("Order engine main loop stopped");
    }

    /// 处理来自匹配引擎的通知（静态方法）
    async fn handle_match_notification_static(
        notification: MatchNotification,
        order_pool: &Arc<OrderPool>,
    ) -> Result<(), String> {
        match notification {
            MatchNotification::TradeExecuted(trade_execution) => {
                debug!(
                    "Handling trade executed: buyer={}, seller={}, price={}",
                    trade_execution.buyer_status.order_id, 
                    trade_execution.seller_status.order_id, 
                    trade_execution.trade.price
                );

                // 更新买方订单
                if let Err(e) = Self::update_order_on_trade_static(
                    &trade_execution.buyer_status.order_id,
                    trade_execution.buyer_status.filled_quantity_in_trade,
                    order_pool,
                ) {
                    error!("Failed to update buyer order {}: {}", trade_execution.buyer_status.order_id, e);
                }

                // 更新卖方订单
                if let Err(e) = Self::update_order_on_trade_static(
                    &trade_execution.seller_status.order_id,
                    trade_execution.seller_status.filled_quantity_in_trade,
                    order_pool,
                ) {
                    error!("Failed to update seller order {}: {}", trade_execution.seller_status.order_id, e);
                }

                info!(
                    "Successfully processed trade: {} shares of {} at {}",
                    trade_execution.trade.quantity, 
                    trade_execution.trade.stock_id, 
                    trade_execution.trade.price
                );
            }
            MatchNotification::OrderCancelled {
                order_id,
                stock_id: _,
                timestamp: _,
            } => {
                debug!("Handling order cancelled: {}", order_id);
                
                if let Err(e) = order_pool.cancel_order(&order_id) {
                    error!("Failed to cancel order {}: {}", order_id, e);
                } else {
                    info!("Successfully cancelled order: {}", order_id);
                }
            }
            MatchNotification::OrderCancelRejected {
                order_id,
                stock_id: _,
                reason,
                timestamp: _,
            } => {
                warn!(
                    "Order cancel rejected for {}: {}",
                    order_id, reason
                );
                // 取消被拒绝，订单状态保持不变
            }
        }

        Ok(())
    }

    /// 根据交易结果更新订单（静态方法）
    fn update_order_on_trade_static(
        order_id: &str, 
        filled_quantity: u64,
        order_pool: &Arc<OrderPool>,
    ) -> Result<(), String> {
        // 获取订单检查是否存在
        let order_arc = order_pool
            .get_order(order_id)
            .ok_or_else(|| format!("Order {} not found in pool", order_id))?;

        // 检查成交数量是否有效
        {
            let order = order_arc.read();
            if order.unfilled_quantity < filled_quantity {
                return Err(format!(
                    "Invalid filled quantity {} for order {} with unfilled quantity {}",
                    filled_quantity, order_id, order.unfilled_quantity
                ));
            }
        }

        // 计算新状态
        let new_unfilled_quantity = {
            let order = order_arc.read();
            order.unfilled_quantity - filled_quantity
        };

        let new_status = if new_unfilled_quantity == 0 {
            OrderStatus::Filled
        } else {
            OrderStatus::PartiallyFilled
        };

        // 使用订单池的方法更新状态（这会正确更新索引和统计信息）
        order_pool.update_order(order_id, new_status, filled_quantity)
    }

    /// 启动清理任务
    async fn start_cleanup_task(&self) -> JoinHandle<()> {
        let order_pool = Arc::clone(&self.order_pool);
        let cleanup_interval = self.config.cleanup_interval_seconds;
        let retain_hours = self.config.retain_completed_orders_hours;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(cleanup_interval)
            );

            loop {
                interval.tick().await;
                
                let cutoff_time = chrono::Utc::now() 
                    - chrono::Duration::hours(retain_hours);
                
                let cleaned_count = order_pool.cleanup_completed_orders(cutoff_time);
                
                if cleaned_count > 0 {
                    info!("Cleaned up {} completed orders", cleaned_count);
                }
            }
        })
    }
}

impl Drop for OrderEngine {
    fn drop(&mut self) {
        // 注意：Drop trait不能是async的，所以我们只能记录警告
        // 实际的清理工作应该在调用stop()方法时完成
        warn!("OrderEngine is being dropped. Ensure stop() was called before dropping.");
    }
}

#[async_trait::async_trait]
impl OrderEngineService for OrderEngine {
    /// 启动订单引擎
    async fn start(&self) -> Result<(), String> {
        OrderEngine::start(self).await
    }

    /// 停止订单引擎
    async fn stop(&self) -> Result<(), String> {
        OrderEngine::stop(self).await
    }

    /// 检查引擎是否正在运行
    async fn is_running(&self) -> bool {
        OrderEngine::is_running(self).await
    }

    /// 提交新订单
    async fn submit_order(&self, order: Order) -> Result<String, String> {
        OrderEngine::submit_order(self, order).await
    }

    /// 取消订单
    async fn cancel_order(&self, order_id: &str, stock_id: &str) -> Result<(), String> {
        OrderEngine::cancel_order(self, order_id, stock_id).await
    }

    /// 获取订单信息
    fn get_order(&self, order_id: &str) -> Option<Order> {
        OrderEngine::get_order(self, order_id)
    }

    /// 获取用户的所有订单
    fn get_user_orders(&self, user_id: &str) -> Vec<Order> {
        OrderEngine::get_user_orders(self, user_id)
    }

    /// 获取股票的所有订单
    fn get_stock_orders(&self, stock_id: &str) -> Vec<Order> {
        OrderEngine::get_stock_orders(self, stock_id)
    }

    /// 获取所有活跃订单
    fn get_active_orders(&self) -> Vec<Order> {
        OrderEngine::get_active_orders(self)
    }

    /// 获取订单池统计信息
    fn get_stats(&self) -> OrderPoolStats {
        OrderEngine::get_stats(self)
    }

    /// 健康检查
    async fn health_check(&self) -> bool {
        self.is_running().await
    }
}

/// 订单引擎工厂
pub struct OrderEngineFactory;

impl OrderEngineFactory {
    /// 创建订单引擎和相关的channels
    pub fn create_with_channels(
        validator: Option<Arc<dyn OrderValidator>>,
        config: OrderEngineConfig,
    ) -> (
        OrderEngine,
        mpsc::Receiver<OrderNotification>,
        mpsc::Sender<MatchNotification>,
    ) {
        let (order_tx, order_rx) = mpsc::channel(config.order_notification_buffer_size);
        let (match_tx, match_rx) = mpsc::channel(config.match_notification_buffer_size);

        let engine = OrderEngine::new(order_tx, match_rx, validator, Arc::new(config));

        (engine, order_rx, match_tx)
    }

    /// 创建订单引擎和相关的channels（使用Arc共享配置）
    pub fn create_with_shared_config(
        validator: Option<Arc<dyn OrderValidator>>,
        config: Arc<OrderEngineConfig>,
    ) -> (
        OrderEngine,
        mpsc::Receiver<OrderNotification>,
        mpsc::Sender<MatchNotification>,
    ) {
        let (order_tx, order_rx) = mpsc::channel(config.order_notification_buffer_size);
        let (match_tx, match_rx) = mpsc::channel(config.match_notification_buffer_size);

        let engine = OrderEngine::new(order_tx, match_rx, validator, config);

        (engine, order_rx, match_tx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OrderSide;
    use rust_decimal_macros::dec;
    use tokio::time::Duration;

    #[tokio::test]
    async fn test_order_engine_basic_flow() {
        let config = OrderEngineConfig::default();
        let (mut engine, order_rx, match_tx) = 
            OrderEngineFactory::create_with_channels(None, config);

        // 启动引擎在后台
        let engine_handle = tokio::spawn(async move {
            engine.start().await
        });

        // 创建测试订单
        let order = Order::new_limit_order(
            "test_order_001".to_string(),
            "SH600036".to_string(),
            "test_user".to_string(),
            OrderSide::Buy,
            dec!(10.50),
            1000,
            chrono::Utc::now(),
        );

        // 模拟接收订单通知（但实际上在这个测试中我们没有提交订单）
        // 我们只是验证引擎能正常启动和停止

        // 让引擎运行一小段时间
        tokio::time::sleep(Duration::from_millis(100)).await;

        // 通过关闭match_tx channel来优雅地停止引擎
        // 这会让match_notification_rx.recv()返回None，从而退出主循环
        drop(match_tx);

        // 等待引擎停止，但设置超时防止无限等待
        let result = tokio::time::timeout(Duration::from_secs(5), engine_handle).await;
        
        match result {
            Ok(engine_result) => {
                // 引擎应该正常停止
                assert!(engine_result.is_ok());
            }
            Err(_) => {
                panic!("Engine failed to stop within timeout");
            }
        }
    }

    #[tokio::test]
    async fn test_match_notification_handling() {
        let config = OrderEngineConfig::default();
        let (engine, _order_rx, _match_tx) = 
            OrderEngineFactory::create_with_channels(None, config);

        // 添加测试订单到池
        let order = Order::new_limit_order(
            "test_order_001".to_string(),
            "SH600036".to_string(),
            "test_user".to_string(),
            OrderSide::Buy,
            dec!(10.50),
            1000,
            chrono::Utc::now(),
        );

        engine.order_pool.add_order(order).unwrap();

        // 验证初始状态
        let initial_order = engine.get_order("test_order_001").unwrap();
        assert_eq!(initial_order.unfilled_quantity, 1000);
        assert_eq!(initial_order.status, OrderStatus::New);

        // 模拟交易执行通知
        let trade_notification = MatchNotification::TradeExecuted(core_entities::TradeExecution {
            trade: core_entities::Trade {
                id: "trade_001".to_string(),
                stock_id: "SH600036".into(),
                price: dec!(10.50),
                quantity: 500,
                timestamp: chrono::Utc::now(),
                aggressor_order_id: "test_order_001".into(),
                resting_order_id: "test_order_002".into(),
                buyer_order_id: "test_order_001".into(),
                seller_order_id: "test_order_002".into(),
                buyer_user_id: "test_user".into(),
                seller_user_id: "other_user".into(),
            },
            buyer_status: core_entities::OrderStatusInTrade {
                order_id: "test_order_001".into(),
                filled_quantity_in_trade: 500,
                total_filled_quantity: 500,
                remaining_quantity: 500,
                is_fully_filled: false,
            },
            seller_status: core_entities::OrderStatusInTrade {
                order_id: "test_order_002".into(),
                filled_quantity_in_trade: 500,
                total_filled_quantity: 500,
                remaining_quantity: 0,
                is_fully_filled: true,
            },
        });

        // 直接调用处理方法
        OrderEngine::handle_match_notification_static(trade_notification, &engine.order_pool).await.unwrap();
        
        // 验证订单状态更新
        let updated_order = engine.get_order("test_order_001").unwrap();
        assert_eq!(updated_order.unfilled_quantity, 500);
        assert_eq!(updated_order.status, OrderStatus::PartiallyFilled);

        // 测试完全成交的情况
        let trade_notification_2 = MatchNotification::TradeExecuted(core_entities::TradeExecution {
            trade: core_entities::Trade {
                id: "trade_002".to_string(),
                stock_id: "SH600036".into(),
                price: dec!(10.50),
                quantity: 500,
                timestamp: chrono::Utc::now(),
                aggressor_order_id: "test_order_002".into(),
                resting_order_id: "test_order_001".into(),
                buyer_order_id: "test_order_002".into(),
                seller_order_id: "test_order_001".into(),
                buyer_user_id: "other_user".into(),
                seller_user_id: "test_user".into(),
            },
            buyer_status: core_entities::OrderStatusInTrade {
                order_id: "test_order_002".into(),
                filled_quantity_in_trade: 500,
                total_filled_quantity: 500,
                remaining_quantity: 0,
                is_fully_filled: true,
            },
            seller_status: core_entities::OrderStatusInTrade {
                order_id: "test_order_001".into(),
                filled_quantity_in_trade: 500,
                total_filled_quantity: 500,
                remaining_quantity: 0,
                is_fully_filled: true,
            },
        });

        OrderEngine::handle_match_notification_static(trade_notification_2, &engine.order_pool).await.unwrap();
        
        let final_order = engine.get_order("test_order_001").unwrap();
        assert_eq!(final_order.unfilled_quantity, 0);
        assert_eq!(final_order.status, OrderStatus::Filled);
    }

    #[tokio::test]
    async fn test_order_cancel_notification() {
        let config = OrderEngineConfig::default();
        let (engine, _order_rx, _match_tx) = 
            OrderEngineFactory::create_with_channels(None, config);

        // 添加测试订单到池
        let order = Order::new_limit_order(
            "test_order_cancel".to_string(),
            "SH600036".to_string(),
            "test_user".to_string(),
            OrderSide::Sell,
            dec!(11.00),
            500,
            chrono::Utc::now(),
        );

        engine.order_pool.add_order(order).unwrap();

        // 验证初始状态
        let initial_order = engine.get_order("test_order_cancel").unwrap();
        assert_eq!(initial_order.status, OrderStatus::New);

        // 模拟订单取消通知
        let cancel_notification = MatchNotification::OrderCancelled {
            order_id: "test_order_cancel".into(),
            stock_id: "SH600036".into(),
            timestamp: chrono::Utc::now(),
        };

        // 直接调用处理方法
        OrderEngine::handle_match_notification_static(cancel_notification, &engine.order_pool).await.unwrap();
        
        // 验证订单状态更新为已取消
        let cancelled_order = engine.get_order("test_order_cancel").unwrap();
        assert_eq!(cancelled_order.status, OrderStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_order_submission_and_engine_lifecycle() {
        let config = OrderEngineConfig::default();
        let (engine, mut order_rx, match_tx) = 
            OrderEngineFactory::create_with_channels(None, config);

        // 创建测试订单
        let order = Order::new_limit_order(
            "test_order_001".to_string(),
            "SH600036".to_string(),
            "test_user".to_string(),
            OrderSide::Buy,
            dec!(10.50),
            1000,
            chrono::Utc::now(),
        );

        // 提交订单（在引擎启动之前）
        let order_id = engine.submit_order(order).await.unwrap();
        assert_eq!(order_id, "test_order_001");

        // 验证订单已经添加到池中
        let stored_order = engine.get_order("test_order_001").unwrap();
        assert_eq!(stored_order.order_id.as_ref(), "test_order_001");
        assert_eq!(stored_order.unfilled_quantity, 1000);

        // 验证能够接收到订单通知
        let notification = tokio::time::timeout(
            Duration::from_millis(100), 
            order_rx.recv()
        ).await.unwrap().unwrap();

        if let OrderNotification::NewOrder(received_order) = notification {
            assert_eq!(received_order.order_id.as_ref(), "test_order_001");
        } else {
            panic!("Expected NewOrder notification");
        }

        // 关闭match channel以确保干净退出
        drop(match_tx);
    }
} 