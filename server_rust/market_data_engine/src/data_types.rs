//! 市场行情数据类型定义

use std::sync::Arc;
use chrono::{DateTime, Utc, NaiveDate};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use core_entities::Timestamp;

/// 静态市场数据 - 每日固定的基础数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticMarketData {
    /// 股票代码
    pub stock_id: Arc<str>,
    /// 交易日期
    pub trade_date: NaiveDate,
    /// 今日开盘价
    pub open_price: Decimal,
    /// 昨日收盘价
    pub prev_close_price: Decimal,
    /// 今日涨停价
    pub limit_up_price: Decimal,
    /// 今日跌停价
    pub limit_down_price: Decimal,
    /// 数据创建时间
    pub created_at: Timestamp,
}

/// 动态市场数据 - 实时更新的交易数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicMarketData {
    /// 股票代码
    pub stock_id: Arc<str>,
    /// 交易日期
    pub trade_date: NaiveDate,
    /// 现价（最新成交价）
    pub current_price: Decimal,
    /// 最高价
    pub high_price: Decimal,
    /// 最低价
    pub low_price: Decimal,
    /// 累计成交量
    pub volume: u64,
    /// 累计成交额
    pub turnover: Decimal,
    /// 成交笔数
    pub trade_count: u64,
    /// 加权平均价格（均价）
    pub vwap: Decimal,
    /// 最后更新时间
    pub last_updated: Timestamp,
}

impl DynamicMarketData {
    /// 创建新的动态市场数据
    pub fn new(stock_id: Arc<str>, trade_date: NaiveDate) -> Self {
        Self {
            stock_id,
            trade_date,
            current_price: Decimal::ZERO,
            high_price: Decimal::ZERO,
            low_price: Decimal::MAX,
            volume: 0,
            turnover: Decimal::ZERO,
            trade_count: 0,
            vwap: Decimal::ZERO,
            last_updated: Utc::now(),
        }
    }

    /// 更新交易数据
    pub fn update_with_trade(&mut self, price: Decimal, quantity: u64, timestamp: Timestamp) {
        // 更新现价
        self.current_price = price;
        
        // 更新最高价
        if price > self.high_price {
            self.high_price = price;
        }
        
        // 更新最低价（如果是第一笔交易或价格更低）
        if self.low_price == Decimal::MAX || price < self.low_price {
            self.low_price = price;
        }
        
        // 更新成交量和成交额
        self.volume += quantity;
        let trade_amount = price * Decimal::from(quantity);
        self.turnover += trade_amount;
        self.trade_count += 1;
        
        // 更新加权平均价格（VWAP）
        if self.volume > 0 {
            self.vwap = self.turnover / Decimal::from(self.volume);
        }
        
        // 更新时间戳
        self.last_updated = timestamp;
    }
}

/// 完整的市场数据（静态 + 动态）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    /// 静态数据
    pub static_data: StaticMarketData,
    /// 动态数据
    pub dynamic_data: DynamicMarketData,
    /// 扩展字段 - 预留给下一迭代
    pub extended_data: ExtendedMarketData,
}

impl MarketData {
    /// 创建新的市场数据
    pub fn new(static_data: StaticMarketData) -> Self {
        let dynamic_data = DynamicMarketData::new(
            Arc::clone(&static_data.stock_id),
            static_data.trade_date,
        );
        
        let extended_data = ExtendedMarketData::new(Arc::clone(&static_data.stock_id));
        
        Self {
            static_data,
            dynamic_data,
            extended_data,
        }
    }

    /// 创建带有扩展数据的市场数据
    pub fn new_with_extended(static_data: StaticMarketData, extended_data: ExtendedMarketData) -> Self {
        let dynamic_data = DynamicMarketData::new(
            Arc::clone(&static_data.stock_id),
            static_data.trade_date,
        );
        
        Self {
            static_data,
            dynamic_data,
            extended_data,
        }
    }

    /// 计算涨跌幅
    pub fn price_change_percent(&self) -> Decimal {
        if self.static_data.prev_close_price.is_zero() {
            return Decimal::ZERO;
        }
        
        let change = self.dynamic_data.current_price - self.static_data.prev_close_price;
        (change / self.static_data.prev_close_price) * Decimal::from(100)
    }

    /// 计算涨跌额
    pub fn price_change_amount(&self) -> Decimal {
        self.dynamic_data.current_price - self.static_data.prev_close_price
    }

    /// 检查是否涨停
    pub fn is_limit_up(&self) -> bool {
        self.dynamic_data.current_price >= self.static_data.limit_up_price
    }

    /// 检查是否跌停
    pub fn is_limit_down(&self) -> bool {
        self.dynamic_data.current_price <= self.static_data.limit_down_price
    }
}

/// 扩展市场数据 - 预留给下一个迭代的字段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedMarketData {
    /// 股票代码
    pub stock_id: Arc<str>,
    /// 总市值（预留）
    pub total_market_cap: Option<Decimal>,
    /// 总股本（预留）
    pub total_shares: Option<u64>,
    /// 流通市值（预留）
    pub circulating_market_cap: Option<Decimal>,
    /// 流通股本（预留）
    pub circulating_shares: Option<u64>,
    /// 换手率（预留）
    pub turnover_rate: Option<Decimal>,
}

impl ExtendedMarketData {
    pub fn new(stock_id: Arc<str>) -> Self {
        Self {
            stock_id,
            total_market_cap: None,
            total_shares: None,
            circulating_market_cap: None,
            circulating_shares: None,
            turnover_rate: None,
        }
    }
}

/// 市场数据通知类型 - 从行情引擎发出的消息
#[derive(Debug, Clone)]
pub enum MarketDataNotification {
    /// 市场数据更新
    MarketDataUpdated {
        stock_id: Arc<str>,
        market_data: MarketData,
        timestamp: Timestamp,
    },
    /// 交易数据处理完成
    TradeProcessed {
        stock_id: Arc<str>,
        trade_id: String,
        price: Decimal,
        quantity: u64,
        timestamp: Timestamp,
    },
    /// 市场数据初始化完成
    MarketDataInitialized {
        stock_id: Arc<str>,
        timestamp: Timestamp,
    },
    /// 错误通知
    Error {
        stock_id: Option<Arc<str>>,
        error_message: String,
        timestamp: Timestamp,
    },
}

/// 市场数据请求类型
#[derive(Debug, Clone)]
pub enum MarketDataRequest {
    /// 获取单个股票的市场数据
    GetMarketData {
        stock_id: Arc<str>,
    },
    /// 获取多个股票的市场数据
    GetMultipleMarketData {
        stock_ids: Vec<Arc<str>>,
    },
    /// 订阅股票的实时行情
    Subscribe {
        stock_id: Arc<str>,
    },
    /// 取消订阅
    Unsubscribe {
        stock_id: Arc<str>,
    },
    /// 重新加载静态数据
    ReloadStaticData {
        stock_id: Option<Arc<str>>, // None 表示重新加载所有股票
        trade_date: NaiveDate,
    },
}

/// 市场数据响应类型
#[derive(Debug, Clone)]
pub enum MarketDataResponse {
    /// 单个市场数据
    MarketData(MarketData),
    /// 多个市场数据
    MultipleMarketData(Vec<MarketData>),
    /// 操作成功确认
    Success {
        message: String,
        timestamp: Timestamp,
    },
    /// 错误响应
    Error {
        error_message: String,
        timestamp: Timestamp,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use chrono::NaiveDate;

    #[test]
    fn test_dynamic_market_data_creation() {
        let stock_id: Arc<str> = "SH600036".into();
        let trade_date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        
        let dynamic_data = DynamicMarketData::new(Arc::clone(&stock_id), trade_date);
        
        assert_eq!(dynamic_data.stock_id.as_ref(), "SH600036");
        assert_eq!(dynamic_data.trade_date, trade_date);
        assert_eq!(dynamic_data.current_price, Decimal::ZERO);
        assert_eq!(dynamic_data.volume, 0);
        assert_eq!(dynamic_data.trade_count, 0);
    }

    #[test]
    fn test_dynamic_market_data_update() {
        let stock_id: Arc<str> = "SH600036".into();
        let trade_date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let mut dynamic_data = DynamicMarketData::new(Arc::clone(&stock_id), trade_date);
        
        // 第一笔交易
        dynamic_data.update_with_trade(dec!(10.50), 1000, Utc::now());
        
        assert_eq!(dynamic_data.current_price, dec!(10.50));
        assert_eq!(dynamic_data.high_price, dec!(10.50));
        assert_eq!(dynamic_data.low_price, dec!(10.50));
        assert_eq!(dynamic_data.volume, 1000);
        assert_eq!(dynamic_data.turnover, dec!(10500));
        assert_eq!(dynamic_data.trade_count, 1);
        assert_eq!(dynamic_data.vwap, dec!(10.50));
        
        // 第二笔交易 - 更高价格
        dynamic_data.update_with_trade(dec!(10.60), 500, Utc::now());
        
        assert_eq!(dynamic_data.current_price, dec!(10.60));
        assert_eq!(dynamic_data.high_price, dec!(10.60));
        assert_eq!(dynamic_data.low_price, dec!(10.50));
        assert_eq!(dynamic_data.volume, 1500);
        assert_eq!(dynamic_data.turnover, dec!(15800));
        assert_eq!(dynamic_data.trade_count, 2);
        
        // 第三笔交易 - 更低价格
        dynamic_data.update_with_trade(dec!(10.40), 200, Utc::now());
        
        assert_eq!(dynamic_data.current_price, dec!(10.40));
        assert_eq!(dynamic_data.high_price, dec!(10.60));
        assert_eq!(dynamic_data.low_price, dec!(10.40));
        assert_eq!(dynamic_data.volume, 1700);
    }

    #[test]
    fn test_market_data_calculations() {
        let static_data = StaticMarketData {
            stock_id: "SH600036".into(),
            trade_date: NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            open_price: dec!(10.00),
            prev_close_price: dec!(9.50),
            limit_up_price: dec!(10.45),
            limit_down_price: dec!(8.55),
            created_at: Utc::now(),
        };
        
        let mut market_data = MarketData::new(static_data);
        market_data.dynamic_data.update_with_trade(dec!(10.20), 1000, Utc::now());
        
        // 测试涨跌幅计算
        let change_percent = market_data.price_change_percent();
        assert!((change_percent - dec!(7.368421052631578947)).abs() < dec!(0.000001));
        
        // 测试涨跌额计算
        let change_amount = market_data.price_change_amount();
        assert_eq!(change_amount, dec!(0.70));
        
        // 测试涨跌停判断
        assert!(!market_data.is_limit_up());
        assert!(!market_data.is_limit_down());
        
        // 测试涨停
        market_data.dynamic_data.current_price = dec!(10.45);
        assert!(market_data.is_limit_up());
        
        // 测试跌停
        market_data.dynamic_data.current_price = dec!(8.55);
        assert!(market_data.is_limit_down());
    }
} 