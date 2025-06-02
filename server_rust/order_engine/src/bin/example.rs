use order_engine::{
    Order, OrderSide, OrderEngineConfig, OrderEngineFactory, 
    MatchNotification, OrderNotification
};
use rust_decimal_macros::dec;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{info, Level};
use tracing_subscriber;
use core_entities;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("启动订单引擎示例程序");

    // 创建配置
    let config = OrderEngineConfig {
        order_notification_buffer_size: 1000,
        match_notification_buffer_size: 1000,
        enable_validation: false, // 简化示例，不使用验证器
        cleanup_interval_seconds: 60,
        retain_completed_orders_hours: 1,
    };

    // 创建订单引擎和通信channels
    let (engine, mut order_rx, match_tx) = 
        OrderEngineFactory::create_with_channels(None, config);

    // 启动订单引擎
    let engine_clone = Arc::new(tokio::sync::Mutex::new(engine));
    let engine_for_task = Arc::clone(&engine_clone);
    
    let engine_task = tokio::spawn(async move {
        let mut engine = engine_for_task.lock().await;
        if let Err(e) = engine.start().await {
            eprintln!("订单引擎启动失败: {}", e);
        }
    });

    // 模拟匹配引擎 - 处理订单通知并发送匹配结果
    let matching_task = tokio::spawn(async move {
        info!("模拟匹配引擎已启动");
        
        while let Some(notification) = order_rx.recv().await {
            match notification {
                OrderNotification::NewOrder(order) => {
                    info!("匹配引擎收到新订单: {}", order.order_id);
                    
                    // 模拟撮合延迟
                    sleep(Duration::from_millis(100)).await;
                    
                    // 模拟部分成交
                    let filled_quantity = order.quantity / 2;
                    
                    let trade_notification = MatchNotification::TradeExecuted(
                        core_entities::TradeExecution {
                            trade: core_entities::Trade {
                                id: format!("trade_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)),
                                stock_id: order.stock_id.clone(),
                                price: order.price,
                                quantity: filled_quantity,
                                timestamp: chrono::Utc::now(),
                                aggressor_order_id: order.order_id.clone(),
                                resting_order_id: "mock_seller_order".into(),
                                buyer_order_id: order.order_id.clone(),
                                seller_order_id: "mock_seller_order".into(),
                                buyer_user_id: "user_001".into(),
                                seller_user_id: "mock_seller".into(),
                            },
                            buyer_status: core_entities::OrderStatusInTrade {
                                order_id: order.order_id.clone(),
                                filled_quantity_in_trade: filled_quantity,
                                total_filled_quantity: filled_quantity,
                                remaining_quantity: order.quantity - filled_quantity,
                                is_fully_filled: filled_quantity == order.quantity,
                            },
                            seller_status: core_entities::OrderStatusInTrade {
                                order_id: "mock_seller_order".into(),
                                filled_quantity_in_trade: filled_quantity,
                                total_filled_quantity: filled_quantity,
                                remaining_quantity: 0,
                                is_fully_filled: true,
                            },
                        }
                    );
                    
                    if let Err(e) = match_tx.send(trade_notification).await {
                        eprintln!("发送交易通知失败: {}", e);
                        break;
                    }
                    
                    info!("模拟交易完成: {} 股 {}", filled_quantity, order.stock_id);
                }
                OrderNotification::CancelOrder { order_id, stock_id } => {
                    info!("匹配引擎收到取消订单请求: {}", order_id);
                    
                    let cancel_notification = MatchNotification::OrderCancelled {
                        order_id,
                        stock_id,
                        timestamp: chrono::Utc::now(),
                    };
                    
                    if let Err(e) = match_tx.send(cancel_notification).await {
                        eprintln!("发送取消通知失败: {}", e);
                        break;
                    }
                }
            }
        }
        
        info!("模拟匹配引擎已停止");
    });

    // 等待一小段时间，让引擎启动
    sleep(Duration::from_millis(500)).await;

    // 示例：提交一些测试订单
    {
        let engine = engine_clone.lock().await;
        
        // 创建买单
        let buy_order = Order::new_limit_order(
            "buy_order_001".to_string(),
            "SH600036".to_string(),
            "user_001".to_string(),
            OrderSide::Buy,
            dec!(10.50),
            1000,
            chrono::Utc::now(),
        );

        info!("提交买单...");
        if let Ok(order_id) = engine.submit_order(buy_order).await {
            info!("买单提交成功: {}", order_id);
        }

        // 创建卖单
        let sell_order = Order::new_limit_order(
            "sell_order_001".to_string(),
            "SH600036".to_string(),
            "user_002".to_string(),
            OrderSide::Sell,
            dec!(10.60),
            500,
            chrono::Utc::now(),
        );

        info!("提交卖单...");
        if let Ok(order_id) = engine.submit_order(sell_order).await {
            info!("卖单提交成功: {}", order_id);
        }

        // 等待交易处理
        sleep(Duration::from_millis(500)).await;

        // 查看订单状态
        if let Some(order) = engine.get_order("buy_order_001") {
            info!("买单状态: {:?}, 剩余数量: {}", order.status, order.unfilled_quantity);
        }

        if let Some(order) = engine.get_order("sell_order_001") {
            info!("卖单状态: {:?}, 剩余数量: {}", order.status, order.unfilled_quantity);
        }

        // 查看统计信息
        let stats = engine.get_stats();
        info!("订单池统计: 总订单数={}, 活跃订单数={}, 已成交订单数={}",
              stats.total_orders, stats.active_orders, stats.filled_orders);

        // 测试取消订单
        info!("取消买单...");
        if let Err(e) = engine.cancel_order("buy_order_001", "SH600036").await {
            info!("取消订单失败: {}", e);
        } else {
            info!("取消订单请求已发送");
        }

        // 等待取消处理
        sleep(Duration::from_millis(300)).await;

        // 再次查看买单状态
        if let Some(order) = engine.get_order("buy_order_001") {
            info!("买单最终状态: {:?}", order.status);
        }

        // 最终统计
        let final_stats = engine.get_stats();
        info!("最终统计: 总订单数={}, 活跃订单数={}, 已成交订单数={}, 已取消订单数={}",
              final_stats.total_orders, final_stats.active_orders, 
              final_stats.filled_orders, final_stats.cancelled_orders);
    }

    info!("示例程序运行完成，停止服务...");

    // 停止服务
    {
        let mut engine = engine_clone.lock().await;
        engine.stop();
    }

    // 等待任务完成
    engine_task.abort();
    matching_task.abort();

    info!("示例程序结束");
    Ok(())
} 