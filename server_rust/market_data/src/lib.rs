//! # 市场行情包 (Market Data)
//! 
//! 本模块实现了A股模拟交易软件的市场行情管理功能。
//! 
//! ## 功能特性
//! 
//! - **静态数据管理**: 从MySQL获取每日固定的基础行情数据（开盘价、收盘价、涨跌停价等）
//! - **动态数据更新**: 监听撮合引擎的交易数据，实时更新行情指标
//! - **行情计算**: 实时计算最高价、最低价、现价、成交量、成交额、均价等指标
//! - **多股票支持**: 支持同时管理多个股票的行情数据
//! 
//! ## 模块结构
//! 
//! ```text
//! market_data/
//! ├── engine.rs       - 市场行情引擎核心实现
//! ├── data_types.rs   - 行情数据类型定义
//! ├── database.rs     - 数据库操作接口
//! └── lib.rs          - 模块入口和重导出
//! ```
//! 
//! ## 使用示例
//! 
//! ```rust,no_run
//! use market_data::*;
//! use chrono::NaiveDate;
//! use tokio::sync::mpsc;
//! use std::sync::Arc;
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // 创建通道用于与撮合引擎和其他系统通信
//!     let (match_notification_tx, match_notification_rx) = mpsc::channel(1000);
//!     let (market_data_notification_tx, market_data_notification_rx) = mpsc::channel(1000);
//!     let (market_data_request_tx, market_data_request_rx) = mpsc::channel(100);
//!     let (market_data_response_tx, market_data_response_rx) = mpsc::channel(100);
//! 
//!     // 配置数据库
//!     let db_config = DatabaseConfig {
//!         database_url: "mysql://root:password@localhost:3306/a_shares_db".to_string(),
//!         max_connections: 10,
//!         connect_timeout_secs: 30,
//!     };
//! 
//!     // 创建数据存储
//!     let repository = Arc::new(
//!         MySqlMarketDataRepository::new(db_config).await?
//!     );
//! 
//!     // 初始化数据库表
//!     repository.initialize_tables().await?;
//! 
//!     // 创建市场行情引擎
//!     let mut engine = MarketDataEngineBuilder::new()
//!         .with_trade_date(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap())
//!         .with_auto_load_all_market_data(true)
//!         .with_repository(repository)
//!         .build(
//!             match_notification_rx,
//!             market_data_notification_tx,
//!             market_data_request_rx,
//!             market_data_response_tx,
//!         )?;
//! 
//!     // 启动引擎
//!     engine.start().await?;
//! 
//!     // 引擎现在会自动处理来自撮合引擎的交易通知，
//!     // 并实时更新市场数据
//! 
//!     Ok(())
//! }
//! ```

pub mod engine;
pub mod data_types;
pub mod database;

// 重新导出主要类型
pub use engine::{MarketDataEngine, MarketDataEngineBuilder, MarketDataEngineConfig, MarketDataEngineError};
pub use data_types::{
    MarketData, StaticMarketData, DynamicMarketData, ExtendedMarketData,
    MarketDataNotification, MarketDataRequest, MarketDataResponse
};
pub use database::{MarketDataRepository, MySqlMarketDataRepository, DatabaseConfig, DatabaseError};

// 重新导出核心实体类型
pub use core_entities::{
    MatchNotification, TradeExecution, Trade, OrderSide, Timestamp
}; 