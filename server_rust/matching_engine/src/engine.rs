// server_rust/matching_engine/src/engine.rs

//！ 外部系统 → [OrderNotification] → MatchingEngine
//！                                      ↓ (路由)
//！         [StockSpecificNotification] → Stock Task A
//！                                    → Stock Task B  
//！                                    → Stock Task C
//！                                      ↓ (撮合结果)
//！         [MatchNotification] ← Stock Tasks
//！                ↓
//！            外部系统

use rust_decimal::Decimal;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{error, info, instrument, warn};

// 使用 core_entities 中的共享类型
use core_entities::{
    MatchNotification, Trade, TradeExecution, OrderStatusInTrade,
    Order, OrderSide, OrderType, OrderStatus, OrderNotification, StockSpecificNotification
};

// 每个股票的订单簿
#[derive(Debug)]
struct StockOrderBook {
    stock_id: Arc<str>,
    bids: BTreeMap<Decimal, VecDeque<Order>>,
    asks: BTreeMap<Decimal, VecDeque<Order>>,
}

impl StockOrderBook {
    fn new(stock_id: Arc<str>) -> Self {
        StockOrderBook {
            stock_id,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    // 改进的 add_order，支持立即匹配
    fn add_order(&mut self, mut order: Order) -> Vec<TradeExecution> {
        info!("Adding order: {:?}", order);
        let mut trade_executions = Vec::new();

        match order.side {
            OrderSide::Buy => {
                // 买单：尝试匹配卖单
                while order.unfilled_quantity > 0 {
                    if self.asks.is_empty() {
                        break; // 没有卖单可匹配
                    }

                    let best_ask_price = *self.asks.first_key_value().unwrap().0;
                    
                    // 检查是否可以匹配
                    if order.price < best_ask_price {
                        break; // 买价小于最佳卖价，无法匹配
                    }

                    let best_ask_orders = self.asks.get_mut(&best_ask_price).unwrap();
                    if best_ask_orders.is_empty() {
                        self.asks.remove(&best_ask_price);
                        continue;
                    }

                    let sell_order = best_ask_orders.front_mut().unwrap();
                    let trade_quantity = std::cmp::min(order.unfilled_quantity, sell_order.unfilled_quantity);
                    
                    // 创建交易记录（使用卖单价格作为成交价）
                    let trade = Trade {
                        id: format!("trade_{}_{}", self.stock_id, chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)),
                        stock_id: Arc::clone(&self.stock_id),
                        price: best_ask_price,
                        quantity: trade_quantity,
                        timestamp: chrono::Utc::now(),
                        aggressor_order_id: Arc::clone(&order.order_id), // 新买单是攻击性订单
                        resting_order_id: Arc::clone(&sell_order.order_id),
                        buyer_order_id: Arc::clone(&order.order_id),
                        seller_order_id: Arc::clone(&sell_order.order_id),
                        buyer_user_id: Arc::clone(&order.user_id),
                        seller_user_id: Arc::clone(&sell_order.user_id),
                    };

                    // 更新订单的未成交数量
                    order.unfilled_quantity -= trade_quantity;
                    sell_order.unfilled_quantity -= trade_quantity;

                    // 创建综合的交易执行通知
                    let trade_execution = TradeExecution {
                        trade,
                        buyer_status: OrderStatusInTrade {
                            order_id: Arc::clone(&order.order_id),
                            filled_quantity_in_trade: trade_quantity,
                            total_filled_quantity: order.quantity - order.unfilled_quantity,
                            remaining_quantity: order.unfilled_quantity,
                            is_fully_filled: order.unfilled_quantity == 0,
                        },
                        seller_status: OrderStatusInTrade {
                            order_id: Arc::clone(&sell_order.order_id),
                            filled_quantity_in_trade: trade_quantity,
                            total_filled_quantity: sell_order.quantity - sell_order.unfilled_quantity,
                            remaining_quantity: sell_order.unfilled_quantity,
                            is_fully_filled: sell_order.unfilled_quantity == 0,
                        },
                    };
                    
                    trade_executions.push(trade_execution);

                    // 移除完全成交的卖单
                    if sell_order.unfilled_quantity == 0 {
                        best_ask_orders.pop_front();
                        if best_ask_orders.is_empty() {
                            self.asks.remove(&best_ask_price);
                        }
                    }
                }

                // 如果买单还有剩余数量，加入订单簿
                if order.unfilled_quantity > 0 {
                    self.bids
                        .entry(order.price)
                        .or_default()
                        .push_back(order);
                }
            }
            OrderSide::Sell => {
                // 卖单：尝试匹配买单
                while order.unfilled_quantity > 0 {
                    if self.bids.is_empty() {
                        break; // 没有买单可匹配
                    }

                    let best_bid_price = *self.bids.last_key_value().unwrap().0;
                    
                    // 检查是否可以匹配
                    if order.price > best_bid_price {
                        break; // 卖价大于最佳买价，无法匹配
                    }

                    let best_bid_orders = self.bids.get_mut(&best_bid_price).unwrap();
                    if best_bid_orders.is_empty() {
                        self.bids.remove(&best_bid_price);
                        continue;
                    }

                    let buy_order = best_bid_orders.front_mut().unwrap();
                    let trade_quantity = std::cmp::min(order.unfilled_quantity, buy_order.unfilled_quantity);
                    
                    // 创建交易记录（使用买单价格作为成交价）
                    let trade = Trade {
                        id: format!("trade_{}_{}", self.stock_id, chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)),
                        stock_id: Arc::clone(&self.stock_id),
                        price: best_bid_price,
                        quantity: trade_quantity,
                        timestamp: chrono::Utc::now(),
                        aggressor_order_id: Arc::clone(&order.order_id), // 新卖单是攻击性订单
                        resting_order_id: Arc::clone(&buy_order.order_id),
                        buyer_order_id: Arc::clone(&buy_order.order_id),
                        seller_order_id: Arc::clone(&order.order_id),
                        buyer_user_id: Arc::clone(&buy_order.user_id),
                        seller_user_id: Arc::clone(&order.user_id),
                    };

                    // 更新订单的未成交数量
                    order.unfilled_quantity -= trade_quantity;
                    buy_order.unfilled_quantity -= trade_quantity;

                    // 创建综合的交易执行通知
                    let trade_execution = TradeExecution {
                        trade,
                        buyer_status: OrderStatusInTrade {
                            order_id: Arc::clone(&buy_order.order_id),
                            filled_quantity_in_trade: trade_quantity,
                            total_filled_quantity: buy_order.quantity - buy_order.unfilled_quantity,
                            remaining_quantity: buy_order.unfilled_quantity,
                            is_fully_filled: buy_order.unfilled_quantity == 0,
                        },
                        seller_status: OrderStatusInTrade {
                            order_id: Arc::clone(&order.order_id),
                            filled_quantity_in_trade: trade_quantity,
                            total_filled_quantity: order.quantity - order.unfilled_quantity,
                            remaining_quantity: order.unfilled_quantity,
                            is_fully_filled: order.unfilled_quantity == 0,
                        },
                    };
                    
                    trade_executions.push(trade_execution);

                    // 移除完全成交的买单
                    if buy_order.unfilled_quantity == 0 {
                        best_bid_orders.pop_front();
                        if best_bid_orders.is_empty() {
                            self.bids.remove(&best_bid_price);
                        }
                    }
                }

                // 如果卖单还有剩余数量，加入订单簿
                if order.unfilled_quantity > 0 {
                    self.asks
                        .entry(order.price)
                        .or_default()
                        .push_back(order);
                }
            }
        }

        trade_executions
    }

    fn cancel_order(&mut self, order_id_to_cancel: &Arc<str>) -> Result<(), String> {
        info!("Cancelling order: {}", order_id_to_cancel);
        let mut found_and_removed = false;
        
        // 查找并移除买单
        let mut empty_bid_prices = Vec::new();
        for (price, price_level) in self.bids.iter_mut() {
            if let Some(pos) = price_level.iter().position(|o| o.order_id.as_ref() == order_id_to_cancel.as_ref()) {
                price_level.remove(pos);
                found_and_removed = true;
                if price_level.is_empty() {
                    empty_bid_prices.push(*price);
                }
                break;
            }
        }
        
        // 清理空的买单价格级别
        for price in empty_bid_prices {
            self.bids.remove(&price);
        }
        
        if !found_and_removed {
            // 查找并移除卖单
            let mut empty_ask_prices = Vec::new();
            for (price, price_level) in self.asks.iter_mut() {
                if let Some(pos) = price_level.iter().position(|o| o.order_id.as_ref() == order_id_to_cancel.as_ref()) {
                    price_level.remove(pos);
                    found_and_removed = true;
                    if price_level.is_empty() {
                        empty_ask_prices.push(*price);
                    }
                    break;
                }
            }
            
            // 清理空的卖单价格级别
            for price in empty_ask_prices {
                self.asks.remove(&price);
            }
        }

        if found_and_removed {
            Ok(())
        } else {
            Err(format!("Order {} not found for cancellation in stock {}", order_id_to_cancel, self.stock_id))
        }
    }
}

pub struct MatchingEngine {
    order_notifier_rx: Option<mpsc::Receiver<OrderNotification>>,
    match_result_tx: Arc<mpsc::Sender<MatchNotification>>,
    
    /// 主任务句柄
    main_task_handle: Option<JoinHandle<()>>,
    
    /// 是否正在运行
    is_running: bool,
}

impl MatchingEngine {
    pub fn new(
        order_notifier_rx: mpsc::Receiver<OrderNotification>,
        match_result_tx: mpsc::Sender<MatchNotification>,
    ) -> Self {
        MatchingEngine {
            order_notifier_rx: Some(order_notifier_rx),
            match_result_tx: Arc::new(match_result_tx),
            main_task_handle: None,
            is_running: false,
        }
    }

    /// 启动撮合引擎
    pub async fn start(&mut self) -> Result<(), String> {
        if self.is_running {
            return Err("Matching engine is already running".to_string());
        }

        info!("Starting matching engine");

        // 取出接收器用于主任务
        let mut order_notifier_rx = self.order_notifier_rx.take()
            .ok_or("Order notifier receiver already taken")?;

        let match_result_tx = Arc::clone(&self.match_result_tx);

        // 启动主任务
        let main_handle = tokio::spawn(async move {
            Self::run_main_loop(order_notifier_rx, match_result_tx).await;
        });

        self.main_task_handle = Some(main_handle);
        self.is_running = true;

        info!("Matching engine started");
        Ok(())
    }

    /// 停止撮合引擎
    pub async fn stop(&mut self) -> Result<(), String> {
        if !self.is_running {
            return Ok(());
        }

        info!("Stopping matching engine...");

        // 停止主任务
        if let Some(handle) = self.main_task_handle.take() {
            handle.abort();
            match handle.await {
                Ok(_) => info!("Matching engine main task stopped normally"),
                Err(e) if e.is_cancelled() => info!("Matching engine main task was cancelled"),
                Err(e) => warn!("Matching engine main task stopped with error: {}", e),
            }
        }

        self.is_running = false;
        info!("Matching engine stopped");
        Ok(())
    }

    /// 检查引擎是否正在运行
    pub fn is_running(&self) -> bool {
        self.is_running
    }

    /// 主运行循环（静态方法，在独立任务中运行）
    async fn run_main_loop(
        mut order_notifier_rx: mpsc::Receiver<OrderNotification>,
        match_result_tx: Arc<mpsc::Sender<MatchNotification>>,
    ) {
        info!("Starting matching engine");
        
        let mut stock_task_senders: HashMap<Arc<str>, mpsc::Sender<StockSpecificNotification>> = HashMap::new();
        let mut stock_task_handles: HashMap<Arc<str>, JoinHandle<()>> = HashMap::new();
        
        while let Some(notification) = order_notifier_rx.recv().await {
            let stock_id_to_route = match &notification {
                OrderNotification::NewOrder(order) => Arc::clone(&order.stock_id),
                OrderNotification::CancelOrder { stock_id, .. } => Arc::clone(stock_id),
            };

            let sender = if let Some(sender) = stock_task_senders.get(&stock_id_to_route) {
                sender.clone()
            } else {
                let (specific_tx, specific_rx) = mpsc::channel::<StockSpecificNotification>(100);
                stock_task_senders.insert(Arc::clone(&stock_id_to_route), specific_tx.clone());
                let match_result_tx_clone = Arc::clone(&match_result_tx);
                let stock_id_for_task = Arc::clone(&stock_id_to_route);

                let handle = tokio::spawn(async move {
                    run_stock_matching_task(stock_id_for_task, specific_rx, match_result_tx_clone).await;
                });
                stock_task_handles.insert(Arc::clone(&stock_id_to_route), handle);
                specific_tx
            };

            let specific_notification = match notification {
                OrderNotification::NewOrder(order) => StockSpecificNotification::NewOrder(order),
                OrderNotification::CancelOrder { order_id, .. } => StockSpecificNotification::CancelOrder { order_id },
            };

            if let Err(e) = sender.send(specific_notification).await {
                error!("Failed to send notification to stock task for {}: {}. Task might have died.", stock_id_to_route, e);
                stock_task_senders.remove(&stock_id_to_route);
                if let Some(handle) = stock_task_handles.remove(&stock_id_to_route) {
                    handle.abort();
                }
            }
        }

        info!("Matching engine main loop stopped, cleaning up stock tasks");
        
        // 清理所有股票任务
        for (stock_id, handle) in stock_task_handles {
            info!("Stopping stock task for {}", stock_id);
            handle.abort();
            let _ = handle.await;
        }
    }

    /// 获取旧版本的run方法，保持API兼容性（已废弃）
    #[deprecated(note = "Use start() method instead")]
    pub async fn run(&mut self) {
        warn!("Using deprecated run() method. Consider using start() instead.");
        if let Err(e) = self.start().await {
            error!("Failed to start matching engine: {}", e);
            return;
        }
        
        // 保持旧的行为：阻塞直到任务完成
        if let Some(handle) = self.main_task_handle.take() {
            let _ = handle.await;
        }
    }
}

impl Drop for MatchingEngine {
    fn drop(&mut self) {
        if self.is_running {
            warn!("MatchingEngine is being dropped while still running. Ensure stop() was called before dropping.");
        }
    }
}

#[instrument(skip(specific_order_rx, match_result_tx))]
async fn run_stock_matching_task(
    stock_id: Arc<str>,
    mut specific_order_rx: mpsc::Receiver<StockSpecificNotification>,
    match_result_tx: Arc<mpsc::Sender<MatchNotification>>,
) {
    info!("Starting stock matching task for stock: {}", stock_id);
    let mut order_book = StockOrderBook::new(Arc::clone(&stock_id));

    while let Some(notification) = specific_order_rx.recv().await {
        match notification {
            StockSpecificNotification::NewOrder(order) => {
                if order.stock_id.as_ref() != stock_id.as_ref() {
                    let rejection_reason = format!("Order sent to wrong stock task. Expected: {}, Got: {}", stock_id, order.stock_id);
                    if let Err(e) = match_result_tx.send(MatchNotification::OrderCancelRejected {
                        order_id: Arc::clone(&order.order_id),
                        stock_id: Arc::clone(&order.stock_id),
                        reason: rejection_reason,
                        timestamp: chrono::Utc::now(),
                    }).await {
                        error!("Stock task [{}]: Failed to send OrderCancelRejected notification: {}", stock_id, e);
                    }
                    continue;
                }
                let trade_executions = order_book.add_order(order.clone());
                for trade_execution in trade_executions {
                    if let Err(e) = match_result_tx.send(MatchNotification::TradeExecuted(trade_execution)).await {
                        error!("Stock task [{}]: Failed to send TradeExecuted notification: {}", stock_id, e);
                    }
                }
            }
            StockSpecificNotification::CancelOrder { order_id } => {
                match order_book.cancel_order(&order_id) {
                    Ok(_) => {
                        if let Err(e) = match_result_tx.send(MatchNotification::OrderCancelled {
                            order_id,
                            stock_id: Arc::clone(&stock_id),
                            timestamp: chrono::Utc::now(),
                        }).await {
                            error!("Stock task [{}]: Failed to send OrderCancelled notification: {}", stock_id, e);
                        }
                    }
                    Err(reason) => {
                        if let Err(e) = match_result_tx.send(MatchNotification::OrderCancelRejected {
                            order_id,
                            stock_id: Arc::clone(&stock_id),
                            reason,
                            timestamp: chrono::Utc::now(),
                        }).await {
                            error!("Stock task [{}]: Failed to send OrderCancelRejected notification: {}", stock_id, e);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::time::Duration;
    use tokio::time::timeout;

    // 辅助函数：创建测试订单
    fn create_test_order(
        order_id: &str,
        stock_id: &str,
        user_id: &str,
        side: OrderSide,
        price: Decimal,
        quantity: u64,
    ) -> Order {
        Order {
            order_id: order_id.to_string().into(),
            stock_id: stock_id.to_string().into(),
            user_id: user_id.to_string().into(),
            side,
            order_type: OrderType::Limit,
            price,
            quantity,
            unfilled_quantity: quantity,
            timestamp: chrono::Utc::now(),
            status: OrderStatus::New,
        }
    }

    #[test]
    fn test_stock_order_book_creation() {
        let order_book = StockOrderBook::new("SH600036".to_string().into());
        assert_eq!(order_book.stock_id.as_ref(), "SH600036");
        assert!(order_book.bids.is_empty());
        assert!(order_book.asks.is_empty());
    }

    #[test]
    fn test_add_buy_order_no_match() {
        let mut order_book = StockOrderBook::new("SH600036".to_string().into());
        let buy_order = create_test_order("order_001", "SH600036", "user_001", OrderSide::Buy, dec!(10.50), 1000);
        
        let trade_executions = order_book.add_order(buy_order.clone());
        
        // 应该没有交易发生
        assert!(trade_executions.is_empty());
        
        // 买单应该被添加到订单簿
        assert_eq!(order_book.bids.len(), 1);
        assert!(order_book.bids.contains_key(&dec!(10.50)));
        assert_eq!(order_book.bids[&dec!(10.50)].len(), 1);
        assert_eq!(order_book.bids[&dec!(10.50)][0].order_id.as_ref(), "order_001");
    }

    #[test]
    fn test_add_sell_order_no_match() {
        let mut order_book = StockOrderBook::new("SH600036".to_string().into());
        let sell_order = create_test_order("order_001", "SH600036", "user_001", OrderSide::Sell, dec!(10.60), 1000);
        
        let trade_executions = order_book.add_order(sell_order.clone());
        
        // 应该没有交易发生
        assert!(trade_executions.is_empty());
        
        // 卖单应该被添加到订单簿
        assert_eq!(order_book.asks.len(), 1);
        assert!(order_book.asks.contains_key(&dec!(10.60)));
        assert_eq!(order_book.asks[&dec!(10.60)].len(), 1);
        assert_eq!(order_book.asks[&dec!(10.60)][0].order_id.as_ref(), "order_001");
    }

    #[test]
    fn test_buy_order_matches_sell_order() {
        let mut order_book = StockOrderBook::new("SH600036".to_string().into());
        
        // 先添加卖单
        let sell_order = create_test_order("sell_001", "SH600036", "seller", OrderSide::Sell, dec!(10.50), 1000);
        order_book.add_order(sell_order);
        
        // 添加匹配的买单
        let buy_order = create_test_order("buy_001", "SH600036", "buyer", OrderSide::Buy, dec!(10.50), 800);
        let trade_executions = order_book.add_order(buy_order);
        
        // 应该有一次交易
        assert_eq!(trade_executions.len(), 1);
        
        let trade_execution = &trade_executions[0];
        let trade = &trade_execution.trade;
        
        // 验证交易详情
        assert_eq!(trade.stock_id.as_ref(), "SH600036");
        assert_eq!(trade.price, dec!(10.50)); // 应该使用卖单价格
        assert_eq!(trade.quantity, 800);
        assert_eq!(trade.buyer_order_id.as_ref(), "buy_001");
        assert_eq!(trade.seller_order_id.as_ref(), "sell_001");
        assert_eq!(trade.buyer_user_id.as_ref(), "buyer");
        assert_eq!(trade.seller_user_id.as_ref(), "seller");
        assert_eq!(trade.aggressor_order_id.as_ref(), "buy_001"); // 买单是攻击性订单
        assert_eq!(trade.resting_order_id.as_ref(), "sell_001");
        
        // 验证买方状态
        assert_eq!(trade_execution.buyer_status.order_id.as_ref(), "buy_001");
        assert_eq!(trade_execution.buyer_status.filled_quantity_in_trade, 800);
        assert_eq!(trade_execution.buyer_status.total_filled_quantity, 800);
        assert_eq!(trade_execution.buyer_status.remaining_quantity, 0);
        assert!(trade_execution.buyer_status.is_fully_filled);
        
        // 验证卖方状态
        assert_eq!(trade_execution.seller_status.order_id.as_ref(), "sell_001");
        assert_eq!(trade_execution.seller_status.filled_quantity_in_trade, 800);
        assert_eq!(trade_execution.seller_status.total_filled_quantity, 800);
        assert_eq!(trade_execution.seller_status.remaining_quantity, 200);
        assert!(!trade_execution.seller_status.is_fully_filled);
        
        // 验证订单簿状态
        assert!(order_book.bids.is_empty()); // 买单完全成交，应该被移除
        assert_eq!(order_book.asks.len(), 1); // 卖单部分成交，仍在订单簿中
        assert_eq!(order_book.asks[&dec!(10.50)].len(), 1);
        assert_eq!(order_book.asks[&dec!(10.50)][0].unfilled_quantity, 200);
    }

    #[test]
    fn test_sell_order_matches_buy_order() {
        let mut order_book = StockOrderBook::new("SH600036".to_string().into());
        
        // 先添加买单
        let buy_order = create_test_order("buy_001", "SH600036", "buyer", OrderSide::Buy, dec!(10.60), 1000);
        order_book.add_order(buy_order);
        
        // 添加匹配的卖单
        let sell_order = create_test_order("sell_001", "SH600036", "seller", OrderSide::Sell, dec!(10.50), 800);
        let trade_executions = order_book.add_order(sell_order);
        
        // 应该有一次交易
        assert_eq!(trade_executions.len(), 1);
        
        let trade_execution = &trade_executions[0];
        let trade = &trade_execution.trade;
        
        // 验证交易详情（应该使用买单价格）
        assert_eq!(trade.price, dec!(10.60));
        assert_eq!(trade.quantity, 800);
        assert_eq!(trade.buyer_order_id.as_ref(), "buy_001");
        assert_eq!(trade.seller_order_id.as_ref(), "sell_001");
        assert_eq!(trade.aggressor_order_id.as_ref(), "sell_001"); // 卖单是攻击性订单
        assert_eq!(trade.resting_order_id.as_ref(), "buy_001");
        
        // 验证订单簿状态
        assert!(order_book.asks.is_empty()); // 卖单完全成交
        assert_eq!(order_book.bids.len(), 1); // 买单部分成交
        assert_eq!(order_book.bids[&dec!(10.60)][0].unfilled_quantity, 200);
    }

    #[test]
    fn test_multiple_partial_matches() {
        let mut order_book = StockOrderBook::new("SH600036".to_string().into());
        
        // 添加多个小卖单
        let sell_order1 = create_test_order("sell_001", "SH600036", "seller1", OrderSide::Sell, dec!(10.50), 300);
        let sell_order2 = create_test_order("sell_002", "SH600036", "seller2", OrderSide::Sell, dec!(10.50), 400);
        let sell_order3 = create_test_order("sell_003", "SH600036", "seller3", OrderSide::Sell, dec!(10.55), 300); // 减少数量到300，让买单正好匹配完
        
        order_book.add_order(sell_order1);
        order_book.add_order(sell_order2);
        order_book.add_order(sell_order3);
        
        // 添加一个大买单，应该匹配所有三个卖单
        let buy_order = create_test_order("buy_001", "SH600036", "buyer", OrderSide::Buy, dec!(10.60), 1000);
        let trade_executions = order_book.add_order(buy_order);
        
        // 应该有三次交易（匹配所有三个卖单）
        assert_eq!(trade_executions.len(), 3);
        
        // 验证第一次交易（价格优先：10.50）
        let trade1 = &trade_executions[0].trade;
        assert_eq!(trade1.price, dec!(10.50));
        assert_eq!(trade1.quantity, 300);
        assert_eq!(trade1.seller_order_id.as_ref(), "sell_001");
        
        // 验证第二次交易（同价格时间优先：10.50）
        let trade2 = &trade_executions[1].trade;
        assert_eq!(trade2.price, dec!(10.50));
        assert_eq!(trade2.quantity, 400);
        assert_eq!(trade2.seller_order_id.as_ref(), "sell_002");
        
        // 验证第三次交易（次优价格：10.55）
        let trade3 = &trade_executions[2].trade;
        assert_eq!(trade3.price, dec!(10.55));
        assert_eq!(trade3.quantity, 300);
        assert_eq!(trade3.seller_order_id.as_ref(), "sell_003");
        
        // 验证最终买方状态（应该完全成交）
        let final_buyer_status = &trade_executions[2].buyer_status;
        assert_eq!(final_buyer_status.total_filled_quantity, 1000);
        assert_eq!(final_buyer_status.remaining_quantity, 0);
        assert!(final_buyer_status.is_fully_filled);
        
        // 验证订单簿状态（所有订单都应该被完全成交）
        assert!(order_book.asks.is_empty()); // 所有卖单都完全成交
        assert!(order_book.bids.is_empty()); // 买单也完全成交
    }

    #[test]
    fn test_price_priority_matching() {
        let mut order_book = StockOrderBook::new("SH600036".to_string().into());
        
        // 添加不同价格的卖单
        let sell_order_high = create_test_order("sell_high", "SH600036", "seller1", OrderSide::Sell, dec!(10.60), 500);
        let sell_order_low = create_test_order("sell_low", "SH600036", "seller2", OrderSide::Sell, dec!(10.50), 500);
        
        order_book.add_order(sell_order_high);
        order_book.add_order(sell_order_low);
        
        // 添加买单，应该优先匹配低价卖单
        let buy_order = create_test_order("buy_001", "SH600036", "buyer", OrderSide::Buy, dec!(10.65), 300);
        let trade_executions = order_book.add_order(buy_order);
        
        assert_eq!(trade_executions.len(), 1);
        let trade = &trade_executions[0].trade;
        assert_eq!(trade.price, dec!(10.50)); // 应该匹配低价卖单
        assert_eq!(trade.seller_order_id.as_ref(), "sell_low");
    }

    #[test]
    fn test_time_priority_matching() {
        let mut order_book = StockOrderBook::new("SH600036".to_string().into());
        
        // 添加相同价格的卖单（时间优先）
        let sell_order1 = create_test_order("sell_001", "SH600036", "seller1", OrderSide::Sell, dec!(10.50), 500);
        let sell_order2 = create_test_order("sell_002", "SH600036", "seller2", OrderSide::Sell, dec!(10.50), 500);
        
        order_book.add_order(sell_order1);
        order_book.add_order(sell_order2);
        
        // 添加买单，应该优先匹配先进入的卖单
        let buy_order = create_test_order("buy_001", "SH600036", "buyer", OrderSide::Buy, dec!(10.50), 300);
        let trade_executions = order_book.add_order(buy_order);
        
        assert_eq!(trade_executions.len(), 1);
        let trade = &trade_executions[0].trade;
        assert_eq!(trade.seller_order_id.as_ref(), "sell_001"); // 应该匹配第一个卖单
        
        // 第一个卖单应该剩余200
        assert_eq!(order_book.asks[&dec!(10.50)][0].unfilled_quantity, 200);
        assert_eq!(order_book.asks[&dec!(10.50)][0].order_id.as_ref(), "sell_001");
    }

    #[test]
    fn test_no_match_price_gap() {
        let mut order_book = StockOrderBook::new("SH600036".to_string().into());
        
        // 添加高价卖单
        let sell_order = create_test_order("sell_001", "SH600036", "seller", OrderSide::Sell, dec!(10.60), 500);
        order_book.add_order(sell_order);
        
        // 添加低价买单（价格差距太大，无法匹配）
        let buy_order = create_test_order("buy_001", "SH600036", "buyer", OrderSide::Buy, dec!(10.40), 500);
        let trade_executions = order_book.add_order(buy_order);
        
        // 应该没有交易
        assert!(trade_executions.is_empty());
        
        // 两个订单都应该在订单簿中
        assert_eq!(order_book.asks.len(), 1);
        assert_eq!(order_book.bids.len(), 1);
    }

    #[test]
    fn test_cancel_existing_order() {
        let mut order_book = StockOrderBook::new("SH600036".to_string().into());
        
        // 添加订单
        let buy_order = create_test_order("buy_001", "SH600036", "buyer", OrderSide::Buy, dec!(10.50), 500);
        let sell_order = create_test_order("sell_001", "SH600036", "seller", OrderSide::Sell, dec!(10.60), 500);
        
        order_book.add_order(buy_order);
        order_book.add_order(sell_order);
        
        // 取消买单
        let result = order_book.cancel_order(&"buy_001".to_string().into());
        assert!(result.is_ok());
        
        // 验证买单被移除
        assert!(order_book.bids.is_empty());
        assert_eq!(order_book.asks.len(), 1); // 卖单应该还在
        
        // 取消卖单
        let result = order_book.cancel_order(&"sell_001".to_string().into());
        assert!(result.is_ok());
        
        // 验证卖单也被移除
        assert!(order_book.asks.is_empty());
    }

    #[test]
    fn test_cancel_non_existing_order() {
        let mut order_book = StockOrderBook::new("SH600036".to_string().into());
        
        // 尝试取消不存在的订单
        let result = order_book.cancel_order(&"non_existing_order".to_string().into());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_cancel_after_partial_fill() {
        let mut order_book = StockOrderBook::new("SH600036".to_string().into());
        
        // 添加大卖单
        let sell_order = create_test_order("sell_001", "SH600036", "seller", OrderSide::Sell, dec!(10.50), 1000);
        order_book.add_order(sell_order);
        
        // 添加小买单进行部分成交
        let buy_order = create_test_order("buy_001", "SH600036", "buyer", OrderSide::Buy, dec!(10.50), 300);
        let trade_executions = order_book.add_order(buy_order);
        
        assert_eq!(trade_executions.len(), 1);
        assert_eq!(order_book.asks[&dec!(10.50)][0].unfilled_quantity, 700);
        
        // 取消剩余的卖单
        let result = order_book.cancel_order(&"sell_001".to_string().into());
        assert!(result.is_ok());
        assert!(order_book.asks.is_empty());
    }

    #[tokio::test]
    async fn test_matching_engine_basic_flow() {
        let (order_notifier_tx, order_notifier_rx) = mpsc::channel::<OrderNotification>(100);
        let (match_result_tx, mut match_result_rx) = mpsc::channel::<MatchNotification>(100);
        
        let mut matching_engine = MatchingEngine::new(order_notifier_rx, match_result_tx);
        
        // 启动撮合引擎
        let engine_handle = tokio::spawn(async move {
            matching_engine.run().await;
        });
        
        // 发送新订单通知
        let order = create_test_order("order_001", "SH600036", "user_001", OrderSide::Buy, dec!(10.50), 1000);
        let notification = OrderNotification::NewOrder(order);
        order_notifier_tx.send(notification).await.unwrap();
        
        // 给一些时间让撮合引擎处理订单
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // 关闭发送端以让撮合引擎退出
        drop(order_notifier_tx);
        
        // 等待撮合引擎结束
        let _ = timeout(Duration::from_secs(1), engine_handle).await;
        
        // 验证没有收到交易通知（因为没有匹配的订单）
        // 对于单个买单，应该没有任何通知发送（不是交易也不是取消）
        let result = timeout(Duration::from_millis(100), match_result_rx.recv()).await;
        
        // 期望超时或通道关闭，都表示没有实际的业务通知
        match result {
            Ok(Some(_)) => panic!("Expected no business notification for a single order with no matches"),
            Ok(None) | Err(_) => {
                // 这是期望的结果：要么通道关闭，要么超时
            }
        }
    }

    #[tokio::test]
    async fn test_matching_engine_with_trade() {
        let (order_notifier_tx, order_notifier_rx) = mpsc::channel::<OrderNotification>(100);
        let (match_result_tx, mut match_result_rx) = mpsc::channel::<MatchNotification>(100);
        
        let mut matching_engine = MatchingEngine::new(order_notifier_rx, match_result_tx);
        
        // 启动撮合引擎
        let engine_handle = tokio::spawn(async move {
            matching_engine.run().await;
        });
        
        // 发送卖单
        let sell_order = create_test_order("sell_001", "SH600036", "seller", OrderSide::Sell, dec!(10.50), 1000);
        order_notifier_tx.send(OrderNotification::NewOrder(sell_order)).await.unwrap();
        
        // 发送匹配的买单
        let buy_order = create_test_order("buy_001", "SH600036", "buyer", OrderSide::Buy, dec!(10.50), 800);
        order_notifier_tx.send(OrderNotification::NewOrder(buy_order)).await.unwrap();
        
        // 接收交易通知
        let match_notification = timeout(Duration::from_secs(1), match_result_rx.recv()).await
            .expect("Should receive notification")
            .expect("Should have notification");
        
        // 验证交易通知
        match match_notification {
            MatchNotification::TradeExecuted(trade_execution) => {
                assert_eq!(trade_execution.trade.stock_id.as_ref(), "SH600036");
                assert_eq!(trade_execution.trade.price, dec!(10.50));
                assert_eq!(trade_execution.trade.quantity, 800);
                assert_eq!(trade_execution.trade.buyer_order_id.as_ref(), "buy_001");
                assert_eq!(trade_execution.trade.seller_order_id.as_ref(), "sell_001");
            }
            _ => panic!("Expected TradeExecuted notification"),
        }
        
        // 关闭发送端
        drop(order_notifier_tx);
        let _ = timeout(Duration::from_secs(1), engine_handle).await;
    }

    #[tokio::test]
    async fn test_matching_engine_cancel_order() {
        let (order_notifier_tx, order_notifier_rx) = mpsc::channel::<OrderNotification>(100);
        let (match_result_tx, mut match_result_rx) = mpsc::channel::<MatchNotification>(100);
        
        let mut matching_engine = MatchingEngine::new(order_notifier_rx, match_result_tx);
        
        // 启动撮合引擎
        let engine_handle = tokio::spawn(async move {
            matching_engine.run().await;
        });
        
        // 发送新订单
        let order = create_test_order("order_001", "SH600036", "user_001", OrderSide::Buy, dec!(10.50), 1000);
        order_notifier_tx.send(OrderNotification::NewOrder(order)).await.unwrap();
        
        // 取消订单
        order_notifier_tx.send(OrderNotification::CancelOrder {
            order_id: "order_001".to_string().into(),
            stock_id: "SH600036".to_string().into(),
        }).await.unwrap();
        
        // 接收取消通知
        let match_notification = timeout(Duration::from_secs(1), match_result_rx.recv()).await
            .expect("Should receive notification")
            .expect("Should have notification");
        
        // 验证取消通知
        match match_notification {
            MatchNotification::OrderCancelled { order_id, stock_id, .. } => {
                assert_eq!(order_id.as_ref(), "order_001");
                assert_eq!(stock_id.as_ref(), "SH600036");
            }
            _ => panic!("Expected OrderCancelled notification"),
        }
        
        // 关闭发送端
        drop(order_notifier_tx);
        let _ = timeout(Duration::from_secs(1), engine_handle).await;
    }

    #[tokio::test]
    async fn test_matching_engine_cancel_non_existing_order() {
        let (order_notifier_tx, order_notifier_rx) = mpsc::channel::<OrderNotification>(100);
        let (match_result_tx, mut match_result_rx) = mpsc::channel::<MatchNotification>(100);
        
        let mut matching_engine = MatchingEngine::new(order_notifier_rx, match_result_tx);
        
        // 启动撮合引擎
        let engine_handle = tokio::spawn(async move {
            matching_engine.run().await;
        });
        
        // 尝试取消不存在的订单
        order_notifier_tx.send(OrderNotification::CancelOrder {
            order_id: "non_existing".to_string().into(),
            stock_id: "SH600036".to_string().into(),
        }).await.unwrap();
        
        // 接收拒绝通知
        let match_notification = timeout(Duration::from_secs(1), match_result_rx.recv()).await
            .expect("Should receive notification")
            .expect("Should have notification");
        
        // 验证拒绝通知
        match match_notification {
            MatchNotification::OrderCancelRejected { order_id, reason, .. } => {
                assert_eq!(order_id.as_ref(), "non_existing");
                assert!(reason.contains("not found"));
            }
            _ => panic!("Expected OrderCancelRejected notification"),
        }
        
        // 关闭发送端
        drop(order_notifier_tx);
        let _ = timeout(Duration::from_secs(1), engine_handle).await;
    }

    #[tokio::test]
    async fn test_matching_engine_multiple_stocks() {
        let (order_notifier_tx, order_notifier_rx) = mpsc::channel::<OrderNotification>(100);
        let (match_result_tx, mut match_result_rx) = mpsc::channel::<MatchNotification>(100);
        
        let mut matching_engine = MatchingEngine::new(order_notifier_rx, match_result_tx);
        
        // 启动撮合引擎
        let engine_handle = tokio::spawn(async move {
            matching_engine.run().await;
        });
        
        // 为不同股票发送订单
        let order1 = create_test_order("order_001", "SH600036", "user_001", OrderSide::Sell, dec!(10.50), 1000);
        let order2 = create_test_order("order_002", "SZ000001", "user_002", OrderSide::Buy, dec!(20.00), 500);
        let order3 = create_test_order("order_003", "SH600036", "user_003", OrderSide::Buy, dec!(10.50), 800);
        
        // 发送订单
        order_notifier_tx.send(OrderNotification::NewOrder(order1)).await.unwrap();
        order_notifier_tx.send(OrderNotification::NewOrder(order2)).await.unwrap();
        order_notifier_tx.send(OrderNotification::NewOrder(order3)).await.unwrap();
        
        // 应该只有SH600036的订单发生交易
        let match_notification = timeout(Duration::from_secs(1), match_result_rx.recv()).await
            .expect("Should receive notification")
            .expect("Should have notification");
        
        match match_notification {
            MatchNotification::TradeExecuted(trade_execution) => {
                assert_eq!(trade_execution.trade.stock_id.as_ref(), "SH600036");
                assert_eq!(trade_execution.trade.buyer_order_id.as_ref(), "order_003");
                assert_eq!(trade_execution.trade.seller_order_id.as_ref(), "order_001");
            }
            _ => panic!("Expected TradeExecuted notification"),
        }
        
        // 验证没有其他交易
        let result = timeout(Duration::from_millis(100), match_result_rx.recv()).await;
        assert!(result.is_err()); // 应该超时
        
        // 关闭发送端
        drop(order_notifier_tx);
        let _ = timeout(Duration::from_secs(1), engine_handle).await;
    }
}