//! 撮合引擎基本使用示例

use matching_engine::*;
use rust_decimal_macros::dec;
use std::sync::Arc;
use tokio;

// 简单的内存订单簿存储实现（仅用于示例）
struct InMemoryOrderBookStore;

#[async_trait::async_trait]
impl OrderBookStore for InMemoryOrderBookStore {
    async fn submit_order(&mut self, _order: &Order) -> OrderBookStoreResult<()> {
        println!("订单已提交到订单簿");
        Ok(())
    }

    async fn cancel_order(
        &mut self,
        _stock_id: &StockId,
        order_id: &OrderId,
    ) -> OrderBookStoreResult<Order> {
        println!("订单 {} 已取消", order_id);
        Err(OrderBookStoreError::DataNotFound {
            key: order_id.to_string(),
        })
    }

    async fn get_order_book_snapshot(
        &self,
        stock_id: &StockId,
        _depth: usize,
    ) -> OrderBookStoreResult<OrderBookView> {
        Ok(OrderBookView::new(stock_id.clone(), chrono::Utc::now()))
    }

    async fn get_best_bid_ask(
        &self,
        _stock_id: &StockId,
    ) -> OrderBookStoreResult<(Option<OrderBookEntry>, Option<OrderBookEntry>)> {
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
        _stock_id: &StockId,
        _order_id: &OrderId,
    ) -> OrderBookStoreResult<Option<Order>> {
        Ok(None)
    }

    async fn update_market_tick(&mut self, _tick: &MarketTick) -> OrderBookStoreResult<()> {
        Ok(())
    }

    async fn update_order_book_after_trade(
        &mut self,
        _stock_id: &StockId,
        trade_confirmation: &TradeConfirmation,
    ) -> OrderBookStoreResult<()> {
        println!("订单簿已更新，交易ID: {}", trade_confirmation.trade.id);
        Ok(())
    }

    async fn get_orders_at_price_level(
        &self,
        _stock_id: &StockId,
        side: OrderSide,
        price: Price,
    ) -> OrderBookStoreResult<Vec<Order>> {
        // 返回一个模拟的对手方订单
        let opposite_side = match side {
            OrderSide::Buy => OrderSide::Sell,
            OrderSide::Sell => OrderSide::Buy,
        };

        Ok(vec![Order::new_limit_order(
            "resting_order_001".to_string(),
            "SH600036".to_string(),
            "market_maker".to_string(),
            opposite_side,
            price,
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
            total_bid_orders: 5,
            total_ask_orders: 3,
            total_bid_quantity: 2500,
            total_ask_quantity: 1800,
            bid_price_levels: 3,
            ask_price_levels: 2,
            max_spread: Some(dec!(0.02)),
            min_spread: Some(dec!(0.01)),
        })
    }
}

// 简单的观察者实现
struct LoggingObserver;

#[async_trait::async_trait]
impl MatchLifecycleObserver for LoggingObserver {
    async fn on_before_match_attempt(
        &self,
        order: &Order,
        _current_order_book: &OrderBookView,
    ) -> MatchLifecycleObserverResult<()> {
        println!("🔍 开始匹配订单: {} {} {} @ {}", 
                 order.id, order.side, order.quantity, order.price);
        Ok(())
    }

    async fn on_trade_generated(
        &self,
        trade: &Trade,
        remaining_qty: Quantity,
    ) -> MatchLifecycleObserverResult<()> {
        println!("✅ 交易生成: {} {} @ {} (剩余: {})", 
                 trade.quantity, trade.stock_id, trade.price, remaining_qty);
        Ok(())
    }

    async fn on_after_order_processed(
        &self,
        order_id: &OrderId,
        final_status: OrderStatus,
    ) -> MatchLifecycleObserverResult<()> {
        println!("📋 订单处理完成: {} -> {}", order_id, final_status);
        Ok(())
    }

    async fn on_order_cancelled(
        &self,
        order: &Order,
        reason: &str,
    ) -> MatchLifecycleObserverResult<()> {
        println!("❌ 订单取消: {} (原因: {})", order.id, reason);
        Ok(())
    }

    async fn on_order_rejected(
        &self,
        order: &Order,
        reason: &str,
    ) -> MatchLifecycleObserverResult<()> {
        println!("🚫 订单拒绝: {} (原因: {})", order.id, reason);
        Ok(())
    }

    async fn on_engine_started(&self) -> MatchLifecycleObserverResult<()> {
        println!("🚀 撮合引擎启动");
        Ok(())
    }

    async fn on_engine_stopped(&self) -> MatchLifecycleObserverResult<()> {
        println!("🛑 撮合引擎停止");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 撮合引擎基本使用示例 ===\n");

    // 1. 创建订单簿存储
    let store = InMemoryOrderBookStore;

    // 2. 创建撮合引擎
    let validator = Arc::new(DefaultOrderValidator);
    let price_strategy = Arc::new(DefaultPriceImprovementStrategy);
    let mut engine = DefaultMatchingEngine::new(store)
        .with_validator(validator)
        .with_price_strategy(price_strategy);

    // 3. 添加观察者
    let observer = Arc::new(LoggingObserver);
    engine.add_observer(observer);

    // 4. 启动引擎
    engine.start().await?;

    // 5. 创建测试订单
    let buy_order = Order::new_limit_order(
        "BUY_001".to_string(),
        "SH600036".to_string(),
        "trader_001".to_string(),
        OrderSide::Buy,
        dec!(10.05), // 买入价格高于卖一价，可以成交
        1000,
        chrono::Utc::now(),
    );

    let sell_order = Order::new_limit_order(
        "SELL_001".to_string(),
        "SH600036".to_string(),
        "trader_002".to_string(),
        OrderSide::Sell,
        dec!(9.95), // 卖出价格低于买一价，可以成交
        800,
        chrono::Utc::now(),
    );

    // 6. 处理订单
    println!("处理买单:");
    let buy_trades = engine.process_order(buy_order).await?;
    println!("买单产生 {} 笔交易\n", buy_trades.len());

    println!("处理卖单:");
    let sell_trades = engine.process_order(sell_order).await?;
    println!("卖单产生 {} 笔交易\n", sell_trades.len());

    // 7. 获取统计信息
    let stats = engine.get_matching_statistics().await?;
    println!("=== 撮合统计 ===");
    println!("总处理订单数: {}", stats.total_orders_processed);
    println!("总交易数: {}", stats.total_trades_generated);
    println!("完全成交订单数: {}", stats.fully_filled_orders);
    println!("部分成交订单数: {}", stats.partially_filled_orders);
    println!("取消订单数: {}", stats.cancelled_orders);

    // 8. 获取订单簿快照
    let snapshot = engine.get_order_book_snapshot(&"SH600036".to_string(), 5).await?;
    println!("\n=== 订单簿快照 ===");
    println!("股票: {}", snapshot.stock_id);
    println!("买单档位数: {}", snapshot.bids.len());
    println!("卖单档位数: {}", snapshot.asks.len());

    // 9. 停止引擎
    engine.stop().await?;

    println!("\n=== 示例完成 ===");
    Ok(())
} 