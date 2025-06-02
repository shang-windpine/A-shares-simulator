//! 市场行情包集成示例
//! 
//! 本示例展示了如何将市场行情引擎与撮合引擎集成，
//! 实现实时的市场数据更新。

use std::sync::Arc;
use chrono::{NaiveDate, Utc};
use tokio::sync::mpsc;
use rust_decimal_macros::dec;

use market_data::*;
use core_entities::*;

/// 模拟数据存储实现（用于示例）
#[derive(Clone)]
struct MockRepository;

#[async_trait::async_trait]
impl MarketDataRepository for MockRepository {
    async fn get_static_market_data(
        &self,
        stock_id: &str,
        trade_date: NaiveDate,
    ) -> Result<StaticMarketData, DatabaseError> {
        // 模拟从数据库获取静态数据
        Ok(StaticMarketData {
            stock_id: stock_id.into(),
            trade_date,
            open_price: dec!(10.00),
            prev_close_price: dec!(9.50),
            limit_up_price: dec!(10.45),
            limit_down_price: dec!(8.55),
            created_at: Utc::now(),
        })
    }

    async fn get_multiple_static_market_data(
        &self,
        stock_ids: &[&str],
        trade_date: NaiveDate,
    ) -> Result<Vec<StaticMarketData>, DatabaseError> {
        let mut results = Vec::new();
        for stock_id in stock_ids {
            results.push(self.get_static_market_data(stock_id, trade_date).await?);
        }
        Ok(results)
    }

    async fn get_all_static_market_data(
        &self,
        trade_date: NaiveDate,
    ) -> Result<Vec<StaticMarketData>, DatabaseError> {
        // 模拟全市场数据
        let stock_ids = vec!["SH600036", "SH600519", "SZ000001"];
        let mut results = Vec::new();
        for stock_id in stock_ids {
            results.push(self.get_static_market_data(stock_id, trade_date).await?);
        }
        Ok(results)
    }

    async fn save_static_market_data(
        &self,
        _data: &StaticMarketData,
    ) -> Result<(), DatabaseError> {
        // 模拟保存操作
        Ok(())
    }

    async fn save_multiple_static_market_data(
        &self,
        _data_list: &[StaticMarketData],
    ) -> Result<(), DatabaseError> {
        // 模拟批量保存操作
        Ok(())
    }
}

/// 模拟撮合引擎发送交易通知
async fn simulate_matching_engine(
    match_notification_tx: mpsc::Sender<MatchNotification>
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🔄 开始模拟撮合引擎交易...");
    
    // 模拟几笔交易
    let trades = vec![
        ("SH600036", dec!(10.20), 1000),
        ("SH600036", dec!(10.25), 500),
        ("SH600519", dec!(258.50), 200),
        ("SH600036", dec!(10.15), 800),
        ("SZ000001", dec!(12.80), 1500),
    ];

    for (i, (stock_id, price, quantity)) in trades.into_iter().enumerate() {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        
        let trade = Trade {
            id: format!("trade_{:03}", i + 1),
            stock_id: stock_id.into(),
            price,
            quantity,
            timestamp: Utc::now(),
            aggressor_order_id: format!("order_buy_{}", i + 1).into(),
            resting_order_id: format!("order_sell_{}", i + 1).into(),
            buyer_order_id: format!("order_buy_{}", i + 1).into(),
            seller_order_id: format!("order_sell_{}", i + 1).into(),
            buyer_user_id: format!("user_buyer_{}", i + 1).into(),
            seller_user_id: format!("user_seller_{}", i + 1).into(),
        };

        let trade_execution = TradeExecution {
            trade: trade.clone(),
            buyer_status: OrderStatusInTrade {
                order_id: trade.buyer_order_id.clone(),
                filled_quantity_in_trade: quantity,
                total_filled_quantity: quantity,
                remaining_quantity: 0,
                is_fully_filled: true,
            },
            seller_status: OrderStatusInTrade {
                order_id: trade.seller_order_id.clone(),
                filled_quantity_in_trade: quantity,
                total_filled_quantity: quantity,
                remaining_quantity: 0,
                is_fully_filled: true,
            },
        };

        println!("📈 发送交易通知: {} @ ¥{} x {}", stock_id, price, quantity);
        
        match_notification_tx.send(MatchNotification::TradeExecuted(trade_execution)).await?;
    }

    println!("✅ 撮合引擎模拟完成");
    Ok(())
}

/// 监听市场数据通知
async fn monitor_market_data_notifications(
    mut market_data_notification_rx: mpsc::Receiver<MarketDataNotification>
) {
    println!("👁️  开始监听市场数据通知...");
    
    while let Some(notification) = market_data_notification_rx.recv().await {
        match notification {
            MarketDataNotification::MarketDataUpdated { stock_id, market_data, timestamp } => {
                let dynamic = &market_data.dynamic_data;
                println!(
                    "📊 [{}] {} 行情更新: 现价=¥{}, 最高=¥{}, 最低=¥{}, 成交量={}, 成交额=¥{}, 均价=¥{}",
                    timestamp.format("%H:%M:%S"),
                    stock_id,
                    dynamic.current_price,
                    dynamic.high_price,
                    dynamic.low_price,
                    dynamic.volume,
                    dynamic.turnover,
                    dynamic.vwap
                );
                
                // 显示涨跌幅
                let change_percent = market_data.price_change_percent();
                let change_amount = market_data.price_change_amount();
                let change_sign = if change_amount >= dec!(0) { "+" } else { "" };
                println!(
                    "   📈 涨跌: {}¥{} ({}{}%)",
                    change_sign, change_amount, change_sign, change_percent.round_dp(2)
                );
            }
            MarketDataNotification::TradeProcessed { stock_id, trade_id, price, quantity, timestamp } => {
                println!(
                    "✅ [{}] 交易处理完成: {} - {} @ ¥{} x {}",
                    timestamp.format("%H:%M:%S"),
                    stock_id,
                    trade_id,
                    price,
                    quantity
                );
            }
            MarketDataNotification::MarketDataInitialized { stock_id, timestamp } => {
                println!(
                    "🚀 [{}] {} 市场数据初始化完成",
                    timestamp.format("%H:%M:%S"),
                    stock_id
                );
            }
            MarketDataNotification::Error { stock_id, error_message, timestamp } => {
                println!(
                    "❌ [{}] 错误 {}: {}",
                    timestamp.format("%H:%M:%S"),
                    stock_id.as_ref().unwrap_or(&"未知".into()),
                    error_message
                );
            }
        }
    }
}

/// 发送市场数据请求并处理响应
async fn test_market_data_requests(
    market_data_request_tx: mpsc::Sender<MarketDataRequest>,
    mut market_data_response_rx: mpsc::Receiver<MarketDataResponse>
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🔍 测试市场数据请求...");
    
    // 等待一些交易完成
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    
    // 请求单个股票的数据
    market_data_request_tx.send(MarketDataRequest::GetMarketData {
        stock_id: "SH600036".into(),
    }).await?;
    
    if let Some(response) = market_data_response_rx.recv().await {
        match response {
            MarketDataResponse::MarketData(market_data) => {
                println!("📋 SH600036 详细数据:");
                println!("   静态: 开盘=¥{}, 昨收=¥{}, 涨停=¥{}, 跌停=¥{}",
                    market_data.static_data.open_price,
                    market_data.static_data.prev_close_price,
                    market_data.static_data.limit_up_price,
                    market_data.static_data.limit_down_price
                );
                println!("   动态: 现价=¥{}, 成交量={}, 成交笔数={}",
                    market_data.dynamic_data.current_price,
                    market_data.dynamic_data.volume,
                    market_data.dynamic_data.trade_count
                );
            }
            _ => println!("❌ 意外的响应类型"),
        }
    }
    
    // 请求多个股票的数据
    market_data_request_tx.send(MarketDataRequest::GetMultipleMarketData {
        stock_ids: vec!["SH600036".into(), "SH600519".into(), "SZ000001".into()],
    }).await?;
    
    if let Some(response) = market_data_response_rx.recv().await {
        match response {
            MarketDataResponse::MultipleMarketData(data_list) => {
                println!("📊 多股票数据摘要:");
                for market_data in data_list {
                    println!("   {} - 现价: ¥{}, 成交量: {}",
                        market_data.static_data.stock_id,
                        market_data.dynamic_data.current_price,
                        market_data.dynamic_data.volume
                    );
                }
            }
            _ => println!("❌ 意外的响应类型"),
        }
    }
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 初始化日志
    tracing_subscriber::fmt::init();
    
    println!("🚀 启动市场行情包集成示例");
    println!("{}", "=".repeat(60));
    
    // 创建通道
    let (match_notification_tx, match_notification_rx) = mpsc::channel(1000);
    let (market_data_notification_tx, market_data_notification_rx) = mpsc::channel(1000);
    let (market_data_request_tx, market_data_request_rx) = mpsc::channel(100);
    let (market_data_response_tx, market_data_response_rx) = mpsc::channel(100);
    
    // 创建模拟数据存储
    let repository = Arc::new(MockRepository);
    
    // 创建市场行情引擎
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
    
    // 启动引擎
    println!("🔧 启动市场行情引擎...");
    engine.start().await?;
    println!("✅ 市场行情引擎启动完成");
    
    // 启动各个任务
    let simulation_handle = tokio::spawn(simulate_matching_engine(match_notification_tx));
    let monitoring_handle = tokio::spawn(monitor_market_data_notifications(market_data_notification_rx));
    let request_handle = tokio::spawn(test_market_data_requests(market_data_request_tx, market_data_response_rx));
    
    // 等待模拟任务完成
    simulation_handle.await??;
    request_handle.await??;
    
    // 让监控任务多运行一会儿
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    // 停止引擎
    println!("🛑 停止市场行情引擎...");
    engine.stop().await?;
    println!("✅ 市场行情引擎已停止");
    
    println!("{}", "=".repeat(60));
    println!("🎉 示例运行完成");
    
    Ok(())
} 