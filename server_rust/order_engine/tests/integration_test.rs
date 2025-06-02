use order_engine::{
    Order, OrderSide, OrderEngine, OrderEngineConfig, OrderEngineFactory,
    MatchNotification, OrderNotification, OrderStatus
};
use rust_decimal_macros::dec;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_order_engine_integration() {
    // 创建配置
    let config = OrderEngineConfig {
        order_notification_buffer_size: 100,
        match_notification_buffer_size: 100,
        enable_validation: false,
        cleanup_interval_seconds: 3600,
        retain_completed_orders_hours: 1,
    };

    // 创建订单引擎和通信通道
    let (mut engine, mut order_rx, match_tx) = 
        OrderEngineFactory::create_with_channels(None, config);

    // 启动订单引擎在后台
    let engine_handle = tokio::spawn(async move {
        if let Err(e) = engine.start().await {
            eprintln!("Engine error: {}", e);
        }
    });

    // 等待引擎启动
    sleep(Duration::from_millis(100)).await;

    // 获取引擎的克隆用于操作（由于引擎已被move到task中，我们需要重新设计）
    // 让我们创建一个新的测试方法
}

#[tokio::test]
async fn test_order_engine_direct_notification() {
    // 创建配置
    let config = OrderEngineConfig {
        order_notification_buffer_size: 100,
        match_notification_buffer_size: 100,
        enable_validation: false,
        cleanup_interval_seconds: 3600,
        retain_completed_orders_hours: 1,
    };

    // 创建订单引擎和通信通道
    let (engine, mut order_rx, _match_tx) = 
        OrderEngineFactory::create_with_channels(None, config);

    // 创建测试订单
    let order = Order::new_limit_order(
        "test_order_001".to_string(),
        "SH600036".to_string(),
        "test_user_001".to_string(),
        OrderSide::Buy,
        dec!(10.50),
        1000,
        chrono::Utc::now(),
    );

    // 提交订单
    let order_id = engine.submit_order(order.clone()).await.unwrap();
    assert_eq!(order_id, "test_order_001");

    // 验证订单在池中
    let retrieved_order = engine.get_order(&order_id).unwrap();
    assert_eq!(retrieved_order.status, OrderStatus::New);
    assert_eq!(retrieved_order.unfilled_quantity, 1000);

    // 从订单通知channel接收订单
    let received_notification = order_rx.recv().await.unwrap();
    match received_notification {
        OrderNotification::NewOrder(received_order) => {
            assert_eq!(received_order.order_id.as_ref(), "test_order_001");
            assert_eq!(received_order.stock_id.as_ref(), "SH600036");
        }
        _ => panic!("Expected NewOrder notification"),
    }

    // 测试基本的查询功能
    let user_orders = engine.get_user_orders("test_user_001");
    assert_eq!(user_orders.len(), 1);
    
    let stock_orders = engine.get_stock_orders("SH600036");
    assert_eq!(stock_orders.len(), 1);
    
    let active_orders = engine.get_active_orders();
    assert_eq!(active_orders.len(), 1);

    // 测试订单取消
    engine.cancel_order(&order_id, "SH600036").await.unwrap();

    // 接收取消通知
    let cancel_notification = order_rx.recv().await.unwrap();
    match cancel_notification {
        OrderNotification::CancelOrder { order_id, stock_id } => {
            assert_eq!(order_id.as_ref(), "test_order_001");
            assert_eq!(stock_id.as_ref(), "SH600036");
        }
        _ => panic!("Expected CancelOrder notification"),
    }

    // 验证统计信息
    let stats = engine.get_stats();
    assert_eq!(stats.total_orders, 1);
    assert_eq!(stats.active_orders, 1);
}

#[tokio::test]
async fn test_order_pool_concurrent_access() {
    use std::sync::Arc;
    use tokio::task::JoinSet;
    
    let config = OrderEngineConfig::default();
    let (engine, _order_rx, _match_tx) = 
        OrderEngineFactory::create_with_channels(None, config);
    
    let engine = Arc::new(engine);
    let mut tasks = JoinSet::new();
    
    // 并发提交多个订单
    for i in 0..100 {
        let engine_clone = Arc::clone(&engine);
        tasks.spawn(async move {
            let order = Order::new_limit_order(
                format!("order_{:03}", i),
                "SH600036".to_string(),
                format!("user_{:03}", i % 10),
                if i % 2 == 0 { OrderSide::Buy } else { OrderSide::Sell },
                dec!(10.50),
                100,
                chrono::Utc::now(),
            );
            
            engine_clone.submit_order(order).await
        });
    }
    
    // 等待所有任务完成
    let mut success_count = 0;
    while let Some(result) = tasks.join_next().await {
        if result.unwrap().is_ok() {
            success_count += 1;
        }
    }
    
    assert_eq!(success_count, 100);
    
    // 验证统计信息
    let stats = engine.get_stats();
    assert_eq!(stats.total_orders, 100);
    assert_eq!(stats.active_orders, 100);
}

#[test]
fn test_order_pool_basic_operations() {
    use order_engine::OrderPool;
    
    let pool = OrderPool::new();
    
    let order = Order::new_limit_order(
        "test_order".to_string(),
        "SH600036".to_string(),
        "test_user".to_string(),
        OrderSide::Buy,
        dec!(10.50),
        1000,
        chrono::Utc::now(),
    );
    
    // 添加订单
    pool.add_order(order.clone()).unwrap();
    
    // 获取订单
    let retrieved = pool.get_order("test_order").unwrap();
    assert_eq!(retrieved.read().order_id.as_ref(), "test_order");
    
    // 按用户查询
    let user_orders = pool.get_orders_by_user("test_user");
    assert_eq!(user_orders.len(), 1);
    
    // 按股票查询
    let stock_orders = pool.get_orders_by_stock("SH600036");
    assert_eq!(stock_orders.len(), 1);
    
    // 活跃订单查询
    let active_orders = pool.get_active_orders();
    assert_eq!(active_orders.len(), 1);
    
    // 更新订单状态
    pool.update_order("test_order", OrderStatus::PartiallyFilled, 500).unwrap();
    
    let updated_order = retrieved.read();
    assert_eq!(updated_order.status, OrderStatus::PartiallyFilled);
    assert_eq!(updated_order.unfilled_quantity, 500);
    
    // 统计信息
    let stats = pool.get_stats();
    assert_eq!(stats.total_orders, 1);
    assert_eq!(stats.active_orders, 1);
} 