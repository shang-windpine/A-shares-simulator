//! 市场数据服务接口定义

use std::sync::Arc;
use async_trait::async_trait;
use crate::data_types::MarketData;

/// 市场数据服务接口
/// 
/// 这个接口定义了外部系统（如 message_dispatcher）访问市场数据的标准方法。
/// 通过这个接口，我们可以保持模块间的松耦合，同时提供清晰的服务边界。
#[async_trait]
pub trait MarketDataService: Send + Sync {
    /// 获取单个股票的市场数据
    /// 
    /// # 参数
    /// * `stock_id` - 股票代码（如 "SH600036"）
    /// 
    /// # 返回值
    /// * `Some(MarketData)` - 如果找到了该股票的数据
    /// * `None` - 如果没有找到该股票的数据
    async fn get_market_data(&self, stock_id: &str) -> Option<MarketData>;

    /// 获取多个股票的市场数据
    /// 
    /// # 参数
    /// * `stock_ids` - 股票代码列表
    /// 
    /// # 返回值
    /// * `Vec<MarketData>` - 找到的市场数据列表（可能少于请求的数量）
    async fn get_multiple_market_data(&self, stock_ids: &[&str]) -> Vec<MarketData>;

    /// 订阅股票的实时行情（预留接口）
    /// 
    /// # 参数
    /// * `stock_id` - 股票代码
    /// 
    /// # 返回值
    /// * `Ok(())` - 订阅成功
    /// * `Err(String)` - 订阅失败，包含错误信息
    async fn subscribe_market_data(&self, stock_id: &str) -> Result<(), String>;

    /// 取消订阅股票的实时行情（预留接口）
    /// 
    /// # 参数
    /// * `stock_id` - 股票代码
    /// 
    /// # 返回值
    /// * `Ok(())` - 取消订阅成功
    /// * `Err(String)` - 取消订阅失败，包含错误信息
    async fn unsubscribe_market_data(&self, stock_id: &str) -> Result<(), String>;

    /// 检查服务是否健康运行
    async fn health_check(&self) -> bool;

    /// 获取当前缓存的所有股票代码
    async fn get_available_stocks(&self) -> Vec<Arc<str>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_types::{StaticMarketData, ExtendedMarketData};
    use chrono::{NaiveDate, Utc};
    use rust_decimal_macros::dec;
    use std::collections::HashMap;
    use tokio::sync::RwLock;

    // 测试用的模拟实现
    struct MockMarketDataService {
        data: Arc<RwLock<HashMap<String, MarketData>>>,
    }

    impl MockMarketDataService {
        fn new() -> Self {
            let mut data = HashMap::new();
            
            // 添加一些测试数据
            let static_data = StaticMarketData {
                stock_id: "SH600036".into(),
                trade_date: NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
                open_price: dec!(10.00),
                prev_close_price: dec!(9.50),
                limit_up_price: dec!(10.45),
                limit_down_price: dec!(8.55),
                created_at: Utc::now(),
            };
            
            let market_data = MarketData::new(static_data);
            data.insert("SH600036".to_string(), market_data);
            
            Self {
                data: Arc::new(RwLock::new(data)),
            }
        }
    }

    #[async_trait]
    impl MarketDataService for MockMarketDataService {
        async fn get_market_data(&self, stock_id: &str) -> Option<MarketData> {
            let data = self.data.read().await;
            data.get(stock_id).cloned()
        }

        async fn get_multiple_market_data(&self, stock_ids: &[&str]) -> Vec<MarketData> {
            let data = self.data.read().await;
            let mut results = Vec::new();
            for stock_id in stock_ids {
                if let Some(market_data) = data.get(*stock_id) {
                    results.push(market_data.clone());
                }
            }
            results
        }

        async fn subscribe_market_data(&self, _stock_id: &str) -> Result<(), String> {
            Ok(())
        }

        async fn unsubscribe_market_data(&self, _stock_id: &str) -> Result<(), String> {
            Ok(())
        }

        async fn health_check(&self) -> bool {
            true
        }

        async fn get_available_stocks(&self) -> Vec<Arc<str>> {
            let data = self.data.read().await;
            data.keys().map(|k| k.as_str().into()).collect()
        }
    }

    #[tokio::test]
    async fn test_mock_service() {
        let service = MockMarketDataService::new();
        
        // 测试获取单个股票数据
        let market_data = service.get_market_data("SH600036").await;
        assert!(market_data.is_some());
        
        let data = market_data.unwrap();
        assert_eq!(data.static_data.stock_id.as_ref(), "SH600036");
        
        // 测试获取不存在的股票
        let non_existent = service.get_market_data("SH999999").await;
        assert!(non_existent.is_none());
        
        // 测试获取多个股票数据
        let multiple_data = service.get_multiple_market_data(&["SH600036", "SH999999"]).await;
        assert_eq!(multiple_data.len(), 1);
        
        // 测试健康检查
        assert!(service.health_check().await);
        
        // 测试获取可用股票列表
        let stocks = service.get_available_stocks().await;
        assert_eq!(stocks.len(), 1);
        assert_eq!(stocks[0].as_ref(), "SH600036");
    }
} 