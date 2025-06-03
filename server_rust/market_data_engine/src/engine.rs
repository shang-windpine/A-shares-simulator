//! 市场行情引擎核心实现

use std::collections::HashMap;
use std::sync::Arc;
use chrono::{NaiveDate, Utc};
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tracing::{error, info, warn, instrument};
use rust_decimal_macros::dec;
use dashmap::DashMap;

use crate::data_types::{
    MarketData, StaticMarketData, DynamicMarketData, 
    MarketDataNotification, MarketDataRequest, MarketDataResponse
};
use crate::database::{MarketDataRepository, DatabaseError};
use core_entities::{MatchNotification, TradeExecution, Trade, Timestamp};
use crate::service::MarketDataService;

/// 市场行情引擎配置
#[derive(Debug, Clone)]
pub struct MarketDataEngineConfig {
    /// 交易日期
    pub trade_date: NaiveDate,
    /// 是否在启动时自动加载全市场数据
    pub auto_load_all_market_data: bool,
    /// 行情通知缓冲区大小
    pub notification_buffer_size: usize,
    /// 请求响应缓冲区大小
    pub request_buffer_size: usize,
}

impl Default for MarketDataEngineConfig {
    fn default() -> Self {
        Self {
            trade_date: chrono::NaiveDate::from_ymd_opt(2024, 9, 23).unwrap(),
            auto_load_all_market_data: true,
            notification_buffer_size: 1000,
            request_buffer_size: 100,
        }
    }
}

/// 市场行情引擎错误类型
#[derive(Debug, thiserror::Error)]
pub enum MarketDataEngineError {
    #[error("数据库错误: {0}")]
    DatabaseError(#[from] DatabaseError),
    
    #[error("股票 {stock_id} 的市场数据未找到")]
    MarketDataNotFound { stock_id: String },
    
    #[error("引擎已停止运行")]
    EngineShutdown,
    
    #[error("通道发送错误: {0}")]
    ChannelSendError(String),
    
    #[error("配置错误: {0}")]
    ConfigError(String),
}

/// 市场行情引擎
pub struct MarketDataEngine {
    /// 配置
    config: MarketDataEngineConfig,
    /// 数据存储
    repository: Arc<dyn MarketDataRepository>,
    /// 内存中的市场数据缓存
    market_data_cache: Arc<DashMap<Arc<str>, MarketData>>,
    /// 撮合引擎通知接收器
    match_notification_rx: Option<mpsc::Receiver<MatchNotification>>,
    /// 市场数据通知发送器
    market_data_notification_tx: mpsc::Sender<MarketDataNotification>,
    /// 市场数据请求接收器
    market_data_request_rx: Option<mpsc::Receiver<MarketDataRequest>>,
    /// 市场数据响应发送器
    market_data_response_tx: mpsc::Sender<MarketDataResponse>,
    /// 主任务句柄
    main_task_handle: Option<JoinHandle<()>>,
    /// 是否正在运行
    is_running: bool,
}

impl MarketDataEngine {
    /// 创建新的市场行情引擎
    pub fn new(
        config: MarketDataEngineConfig,
        repository: Arc<dyn MarketDataRepository>,
        match_notification_rx: mpsc::Receiver<MatchNotification>,
        market_data_notification_tx: mpsc::Sender<MarketDataNotification>,
        market_data_request_rx: mpsc::Receiver<MarketDataRequest>,
        market_data_response_tx: mpsc::Sender<MarketDataResponse>,
    ) -> Self {
        Self {
            config,
            repository,
            market_data_cache: Arc::new(DashMap::new()),
            match_notification_rx: Some(match_notification_rx),
            market_data_notification_tx,
            market_data_request_rx: Some(market_data_request_rx),
            market_data_response_tx,
            main_task_handle: None,
            is_running: false,
        }
    }

    /// 启动引擎
    #[instrument(skip(self))]
    pub async fn start(&mut self) -> Result<(), MarketDataEngineError> {
        info!("正在启动市场行情引擎...");
        
        // 检查是否已经在运行
        if self.is_running {
            warn!("市场行情引擎已经在运行");
            return Err(MarketDataEngineError::ConfigError("引擎已在运行".to_string()));
        }

        // 初始化市场数据
        if self.config.auto_load_all_market_data {
            self.load_all_market_data().await?;
        }

        // 启动主循环
        let match_notification_rx = self.match_notification_rx.take()
            .ok_or(MarketDataEngineError::ConfigError("撮合通知接收器未初始化".to_string()))?;
        
        let market_data_request_rx = self.market_data_request_rx.take()
            .ok_or(MarketDataEngineError::ConfigError("市场数据请求接收器未初始化".to_string()))?;

        let market_data_cache = Arc::clone(&self.market_data_cache);
        let market_data_notification_tx = self.market_data_notification_tx.clone();
        let market_data_response_tx = self.market_data_response_tx.clone();
        let repository = Arc::clone(&self.repository);
        let trade_date = self.config.trade_date;

        let handle = tokio::spawn(async move {
            Self::run_engine_loop(
                match_notification_rx,
                market_data_request_rx,
                market_data_cache,
                market_data_notification_tx,
                market_data_response_tx,
                repository,
                trade_date,
            ).await;
        });

        self.main_task_handle = Some(handle);
        self.is_running = true;

        info!("市场行情引擎启动完成");
        Ok(())
    }

    /// 停止引擎
    #[instrument(skip(self))]
    pub async fn stop(&mut self) -> Result<(), MarketDataEngineError> {
        if !self.is_running {
            return Ok(());
        }

        info!("正在停止市场行情引擎...");
        
        // 停止主任务
        if let Some(handle) = self.main_task_handle.take() {
            handle.abort();
            match handle.await {
                Ok(_) => info!("市场数据引擎主任务正常停止"),
                Err(e) if e.is_cancelled() => info!("市场数据引擎主任务被取消"),
                Err(e) => warn!("市场数据引擎主任务停止时出错: {}", e),
            }
        }

        self.is_running = false;
        info!("市场行情引擎已停止");
        Ok(())
    }

    /// 检查引擎是否正在运行
    pub fn is_running(&self) -> bool {
        self.is_running
    }

    /// 获取市场数据
    pub async fn get_market_data(&self, stock_id: &str) -> Option<MarketData> {
        self.market_data_cache.get(stock_id).map(|entry| entry.value().clone())
    }

    /// 获取所有市场数据
    pub async fn get_all_market_data(&self) -> HashMap<Arc<str>, MarketData> {
        self.market_data_cache
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    /// 加载全市场数据
    #[instrument(skip(self))]
    async fn load_all_market_data(&self) -> Result<(), MarketDataEngineError> {
        info!("正在加载全市场静态数据...");
        
        let static_data_list = self.repository
            .get_all_static_market_data(self.config.trade_date)
            .await?;

        for static_data in static_data_list {
            let market_data = MarketData::new(static_data.clone());
            self.market_data_cache.insert(Arc::clone(&static_data.stock_id), market_data);
        }

        info!("成功加载了 {} 只股票的市场数据", self.market_data_cache.len());
        Ok(())
    }

    /// 引擎主循环（静态方法，在独立任务中运行）
    #[instrument(skip_all)]
    async fn run_engine_loop(
        mut match_notification_rx: mpsc::Receiver<MatchNotification>,
        mut market_data_request_rx: mpsc::Receiver<MarketDataRequest>,
        market_data_cache: Arc<DashMap<Arc<str>, MarketData>>,
        market_data_notification_tx: mpsc::Sender<MarketDataNotification>,
        market_data_response_tx: mpsc::Sender<MarketDataResponse>,
        repository: Arc<dyn MarketDataRepository>,
        trade_date: NaiveDate,
    ) {
        info!("市场行情引擎主循环开始运行");

        loop {
            tokio::select! {
                // 处理撮合引擎通知
                Some(match_notification) = match_notification_rx.recv() => {
                    Self::handle_match_notification(
                        match_notification,
                        &market_data_cache,
                        &market_data_notification_tx,
                        trade_date,
                    ).await;
                }
                
                // 处理市场数据请求
                Some(request) = market_data_request_rx.recv() => {
                    Self::handle_market_data_request(
                        request,
                        &market_data_cache,
                        &market_data_response_tx,
                        &repository,
                        trade_date,
                    ).await;
                }
                
                // 没有更多消息时，退出循环
                else => {
                    info!("所有通道都已关闭，退出主循环");
                    break;
                }
            }
        }

        info!("市场行情引擎主循环结束");
    }

    /// 处理撮合引擎通知
    #[instrument(skip_all)]
    async fn handle_match_notification(
        notification: MatchNotification,
        market_data_cache: &Arc<DashMap<Arc<str>, MarketData>>,
        market_data_notification_tx: &mpsc::Sender<MarketDataNotification>,
        trade_date: NaiveDate,
    ) {
        match notification {
            MatchNotification::TradeExecuted(trade_execution) => {
                Self::process_trade_execution(
                    trade_execution,
                    market_data_cache,
                    market_data_notification_tx,
                    trade_date,
                ).await;
            }
            MatchNotification::OrderCancelled { .. } => {
                // 订单取消通常不影响市场数据
                // 如果需要统计订单取消数量等信息，可以在这里处理
            }
            MatchNotification::OrderCancelRejected { .. } => {
                // 订单取消被拒绝通常不影响市场数据
            }
        }
    }

    /// 处理交易执行
    #[instrument(skip_all)]
    async fn process_trade_execution(
        trade_execution: TradeExecution,
        market_data_cache: &Arc<DashMap<Arc<str>, MarketData>>,
        market_data_notification_tx: &mpsc::Sender<MarketDataNotification>,
        trade_date: NaiveDate,
    ) {
        let trade = &trade_execution.trade;
        let stock_id = Arc::clone(&trade.stock_id);

        // 更新市场数据
        {
            let mut market_data = market_data_cache.entry(Arc::clone(&stock_id)).or_insert_with(|| {
                // 如果缓存中没有该股票数据，创建一个默认的
                warn!("股票 {} 的市场数据不在缓存中，创建默认数据", stock_id);
                let static_data = StaticMarketData {
                    stock_id: Arc::clone(&stock_id),
                    trade_date,
                    open_price: trade.price,
                    prev_close_price: trade.price,
                    limit_up_price: trade.price * dec!(1.1),
                    limit_down_price: trade.price * dec!(0.9),
                    created_at: Utc::now(),
                };
                MarketData::new(static_data)
            });

            // 更新动态数据
            market_data.dynamic_data.update_with_trade(
                trade.price,
                trade.quantity,
                trade.timestamp,
            );
        }

        // 发送通知
        let notification = MarketDataNotification::TradeProcessed {
            stock_id: Arc::clone(&stock_id),
            trade_id: trade.id.clone(),
            price: trade.price,
            quantity: trade.quantity,
            timestamp: trade.timestamp,
        };

        if let Err(e) = market_data_notification_tx.send(notification).await {
            error!("发送交易处理通知失败: {}", e);
        }

        // 发送市场数据更新通知
        if let Some(market_data_entry) = market_data_cache.get(&stock_id) {
            let update_notification = MarketDataNotification::MarketDataUpdated {
                stock_id: Arc::clone(&stock_id),
                market_data: market_data_entry.value().clone(),
                timestamp: Utc::now(),
            };

            if let Err(e) = market_data_notification_tx.send(update_notification).await {
                error!("发送市场数据更新通知失败: {}", e);
            }
        }
    }

    /// 处理市场数据请求
    #[instrument(skip_all)]
    async fn handle_market_data_request(
        request: MarketDataRequest,
        market_data_cache: &Arc<DashMap<Arc<str>, MarketData>>,
        market_data_response_tx: &mpsc::Sender<MarketDataResponse>,
        repository: &Arc<dyn MarketDataRepository>,
        trade_date: NaiveDate,
    ) {
        let response = match request {
            MarketDataRequest::GetMarketData { stock_id } => {
                match market_data_cache.get(&stock_id) {
                    Some(entry) => MarketDataResponse::MarketData(entry.value().clone()),
                    None => MarketDataResponse::Error {
                        error_message: format!("股票 {} 的市场数据未找到", stock_id),
                        timestamp: Utc::now(),
                    },
                }
            }
            
            MarketDataRequest::GetMultipleMarketData { stock_ids } => {
                let mut results = Vec::new();
                for stock_id in stock_ids {
                    if let Some(entry) = market_data_cache.get(&stock_id) {
                        results.push(entry.value().clone());
                    }
                }
                MarketDataResponse::MultipleMarketData(results)
            }
            
            MarketDataRequest::Subscribe { .. } => {
                // 订阅功能的具体实现可以根据需要扩展
                MarketDataResponse::Success {
                    message: "订阅功能暂未实现".to_string(),
                    timestamp: Utc::now(),
                }
            }
            
            MarketDataRequest::Unsubscribe { .. } => {
                // 取消订阅功能的具体实现可以根据需要扩展
                MarketDataResponse::Success {
                    message: "取消订阅功能暂未实现".to_string(),
                    timestamp: Utc::now(),
                }
            }
            
            MarketDataRequest::ReloadStaticData { stock_id, trade_date: reload_date } => {
                match Self::reload_static_data(
                    stock_id,
                    reload_date,
                    market_data_cache,
                    repository,
                ).await {
                    Ok(count) => MarketDataResponse::Success {
                        message: format!("成功重新加载了 {} 条静态数据", count),
                        timestamp: Utc::now(),
                    },
                    Err(e) => MarketDataResponse::Error {
                        error_message: format!("重新加载静态数据失败: {}", e),
                        timestamp: Utc::now(),
                    },
                }
            }
        };

        if let Err(e) = market_data_response_tx.send(response).await {
            error!("发送市场数据响应失败: {}", e);
        }
    }

    /// 重新加载静态数据
    async fn reload_static_data(
        stock_id: Option<Arc<str>>,
        trade_date: NaiveDate,
        market_data_cache: &Arc<DashMap<Arc<str>, MarketData>>,
        repository: &Arc<dyn MarketDataRepository>,
    ) -> Result<usize, DatabaseError> {
        match stock_id {
            Some(stock_id) => {
                // 重新加载单个股票的数据
                let static_data = repository
                    .get_static_market_data(stock_id.as_ref(), trade_date)
                    .await?;
                
                let market_data = MarketData::new(static_data);
                market_data_cache.insert(stock_id, market_data);
                Ok(1)
            }
            None => {
                // 重新加载所有股票的数据
                let static_data_list = repository
                    .get_all_static_market_data(trade_date)
                    .await?;

                market_data_cache.clear();
                
                for static_data in &static_data_list {
                    let market_data = MarketData::new(static_data.clone());
                    market_data_cache.insert(Arc::clone(&static_data.stock_id), market_data);
                }
                
                Ok(static_data_list.len())
            }
        }
    }
}

impl Drop for MarketDataEngine {
    fn drop(&mut self) {
        if self.is_running {
            warn!("MarketDataEngine is being dropped while still running. Ensure stop() was called before dropping.");
        }
    }
}

/// 市场行情引擎构建器
pub struct MarketDataEngineBuilder {
    config: MarketDataEngineConfig,
    repository: Option<Arc<dyn MarketDataRepository>>,
}

impl MarketDataEngineBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self {
            config: MarketDataEngineConfig::default(),
            repository: None,
        }
    }

    /// 设置配置
    pub fn with_config(mut self, config: MarketDataEngineConfig) -> Self {
        self.config = config;
        self
    }

    /// 设置交易日期
    pub fn with_trade_date(mut self, trade_date: NaiveDate) -> Self {
        self.config.trade_date = trade_date;
        self
    }

    /// 设置是否自动加载全市场数据
    pub fn with_auto_load_all_market_data(mut self, auto_load: bool) -> Self {
        self.config.auto_load_all_market_data = auto_load;
        self
    }

    /// 设置数据存储
    pub fn with_repository(mut self, repository: Arc<dyn MarketDataRepository>) -> Self {
        self.repository = Some(repository);
        self
    }

    /// 构建引擎
    pub fn build(
        self,
        match_notification_rx: mpsc::Receiver<MatchNotification>,
        market_data_notification_tx: mpsc::Sender<MarketDataNotification>,
        market_data_request_rx: mpsc::Receiver<MarketDataRequest>,
        market_data_response_tx: mpsc::Sender<MarketDataResponse>,
    ) -> Result<MarketDataEngine, MarketDataEngineError> {
        let repository = self.repository
            .ok_or(MarketDataEngineError::ConfigError("数据存储未设置".to_string()))?;

        Ok(MarketDataEngine::new(
            self.config,
            repository,
            match_notification_rx,
            market_data_notification_tx,
            market_data_request_rx,
            market_data_response_tx,
        ))
    }
}

impl Default for MarketDataEngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// 为 MarketDataEngine 实现 MarketDataService trait
#[async_trait::async_trait]
impl MarketDataService for MarketDataEngine {
    async fn get_market_data(&self, stock_id: &str) -> Option<MarketData> {
        self.market_data_cache.get(stock_id).map(|entry| entry.value().clone())
    }

    async fn get_multiple_market_data(&self, stock_ids: &[&str]) -> Vec<MarketData> {
        let mut results = Vec::new();
        for stock_id in stock_ids {
            if let Some(entry) = self.market_data_cache.get(*stock_id) {
                results.push(entry.value().clone());
            }
        }
        results
    }

    async fn subscribe_market_data(&self, _stock_id: &str) -> Result<(), String> {
        // 在当前实现中，所有股票的数据都会自动更新
        // 这里可以在未来扩展为真正的订阅机制
        Ok(())
    }

    async fn unsubscribe_market_data(&self, _stock_id: &str) -> Result<(), String> {
        // 预留接口，未来可以实现真正的取消订阅逻辑
        Ok(())
    }

    async fn health_check(&self) -> bool {
        self.is_running
    }

    async fn get_available_stocks(&self) -> Vec<Arc<str>> {
        self.market_data_cache.iter().map(|entry| entry.key().clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{MarketDataRepository, DatabaseError};
    use crate::data_types::StaticMarketData;
    use rust_decimal_macros::dec;
    use chrono::NaiveDate;
    use std::sync::Arc;

    // 模拟数据存储实现
    #[derive(Clone)]
    struct MockRepository {
        data: Arc<RwLock<HashMap<String, StaticMarketData>>>,
    }

    impl MockRepository {
        fn new() -> Self {
            Self {
                data: Arc::new(RwLock::new(HashMap::new())),
            }
        }

        async fn add_test_data(&self, stock_id: &str, trade_date: NaiveDate) {
            let static_data = StaticMarketData {
                stock_id: stock_id.into(),
                trade_date,
                open_price: dec!(10.00),
                prev_close_price: dec!(9.50),
                limit_up_price: dec!(10.45),
                limit_down_price: dec!(8.55),
                created_at: Utc::now(),
            };

            let mut data = self.data.write().await;
            data.insert(format!("{}_{}", stock_id, trade_date), static_data);
        }
    }

    #[async_trait::async_trait]
    impl MarketDataRepository for MockRepository {
        async fn get_static_market_data(
            &self,
            stock_id: &str,
            trade_date: NaiveDate,
        ) -> Result<StaticMarketData, DatabaseError> {
            let data = self.data.read().await;
            let key = format!("{}_{}", stock_id, trade_date);
            data.get(&key)
                .cloned()
                .ok_or(DatabaseError::StaticDataNotFound {
                    stock_id: stock_id.to_string(),
                    trade_date,
                })
        }

        async fn get_multiple_static_market_data(
            &self,
            stock_ids: &[&str],
            trade_date: NaiveDate,
        ) -> Result<Vec<StaticMarketData>, DatabaseError> {
            let mut results = Vec::new();
            for stock_id in stock_ids {
                if let Ok(data) = self.get_static_market_data(stock_id, trade_date).await {
                    results.push(data);
                }
            }
            Ok(results)
        }

        async fn get_all_static_market_data(
            &self,
            trade_date: NaiveDate,
        ) -> Result<Vec<StaticMarketData>, DatabaseError> {
            let data = self.data.read().await;
            let results: Vec<StaticMarketData> = data
                .values()
                .filter(|d| d.trade_date == trade_date)
                .cloned()
                .collect();
            Ok(results)
        }

        async fn get_complete_market_data(
            &self,
            stock_id: &str,
            trade_date: NaiveDate,
        ) -> Result<MarketData, DatabaseError> {
            let static_data = self.get_static_market_data(stock_id, trade_date).await?;
            Ok(MarketData::new(static_data))
        }

        async fn get_multiple_complete_market_data(
            &self,
            stock_ids: &[&str],
            trade_date: NaiveDate,
        ) -> Result<Vec<MarketData>, DatabaseError> {
            let static_data_list = self.get_multiple_static_market_data(stock_ids, trade_date).await?;
            let results = static_data_list.into_iter()
                .map(|static_data| MarketData::new(static_data))
                .collect();
            Ok(results)
        }

        async fn get_all_complete_market_data(
            &self,
            trade_date: NaiveDate,
        ) -> Result<Vec<MarketData>, DatabaseError> {
            let static_data_list = self.get_all_static_market_data(trade_date).await?;
            let results = static_data_list.into_iter()
                .map(|static_data| MarketData::new(static_data))
                .collect();
            Ok(results)
        }

        async fn save_static_market_data(
            &self,
            data: &StaticMarketData,
        ) -> Result<(), DatabaseError> {
            let mut storage = self.data.write().await;
            let key = format!("{}_{}", data.stock_id, data.trade_date);
            storage.insert(key, data.clone());
            Ok(())
        }

        async fn save_multiple_static_market_data(
            &self,
            data_list: &[StaticMarketData],
        ) -> Result<(), DatabaseError> {
            for data in data_list {
                self.save_static_market_data(data).await?;
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_market_data_engine_creation() {
        let (match_tx, match_rx) = mpsc::channel(10);
        let (notification_tx, _notification_rx) = mpsc::channel(10);
        let (request_tx, request_rx) = mpsc::channel(10);
        let (response_tx, _response_rx) = mpsc::channel(10);

        let repository = Arc::new(MockRepository::new());
        let config = MarketDataEngineConfig::default();

        let engine = MarketDataEngine::new(
            config,
            repository,
            match_rx,
            notification_tx,
            request_rx,
            response_tx,
        );

        assert!(!engine.is_running);
    }

    #[tokio::test]
    async fn test_market_data_engine_builder() {
        let (match_tx, match_rx) = mpsc::channel(10);
        let (notification_tx, _notification_rx) = mpsc::channel(10);
        let (request_tx, request_rx) = mpsc::channel(10);
        let (response_tx, _response_rx) = mpsc::channel(10);

        let repository = Arc::new(MockRepository::new());
        let trade_date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();

        let engine = MarketDataEngineBuilder::new()
            .with_trade_date(trade_date)
            .with_auto_load_all_market_data(false)
            .with_repository(repository)
            .build(match_rx, notification_tx, request_rx, response_tx)
            .unwrap();

        assert_eq!(engine.config.trade_date, trade_date);
        assert!(!engine.config.auto_load_all_market_data);
    }
} 