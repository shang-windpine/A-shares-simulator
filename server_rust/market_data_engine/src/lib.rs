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
//! ```

pub mod engine;
pub mod data_types;
pub mod database;
pub mod service;

// 重新导出主要类型
pub use engine::{MarketDataEngine, MarketDataEngineBuilder, MarketDataEngineConfig, MarketDataEngineError};
pub use data_types::{
    MarketData, StaticMarketData, DynamicMarketData, ExtendedMarketData,
    MarketDataNotification, MarketDataRequest, MarketDataResponse
};
pub use database::{MarketDataRepository, MySqlMarketDataRepository, DatabaseConfig, DatabaseError};
pub use service::MarketDataService;

// 重新导出核心实体类型
pub use core_entities::{
    MatchNotification, TradeExecution, Trade, OrderSide, Timestamp
}; 