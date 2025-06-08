//! 市场行情引擎核心实现

use std::collections::HashMap;
use std::sync::Arc;
use chrono::{NaiveDate, Utc};
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tracing::{error, info, warn, instrument};
use dashmap::DashMap;

use crate::data_types::{
    MarketData, MarketDataNotification, MarketDataRequest, MarketDataResponse
};
use crate::database::{MarketDataRepository, DatabaseError};
use core_entities::{MatchNotification, TradeExecution};
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

/// 引擎内部可变状态
struct MarketDataEngineInner {
    /// 撮合引擎通知接收器
    match_notification_rx: Option<mpsc::Receiver<MatchNotification>>,
    /// 市场数据请求接收器
    market_data_request_rx: Option<mpsc::Receiver<MarketDataRequest>>,
    /// 主任务句柄
    main_task_handle: Option<JoinHandle<()>>,
    /// 是否正在运行
    is_running: bool,
}

/// 市场行情引擎
#[derive(Clone)]
pub struct MarketDataEngine {
    /// 配置
    config: Arc<MarketDataEngineConfig>,
    /// 数据存储
    repository: Arc<dyn MarketDataRepository>,
    /// 内存中的市场数据缓存
    market_data_cache: Arc<DashMap<Arc<str>, MarketData>>,
    /// 市场数据通知发送器
    market_data_notification_tx: mpsc::Sender<MarketDataNotification>,
    /// 市场数据响应发送器
    market_data_response_tx: mpsc::Sender<MarketDataResponse>,
    /// 内部可变状态
    inner: Arc<RwLock<MarketDataEngineInner>>,
}

impl MarketDataEngine {
    /// 创建新的市场行情引擎
    pub fn new(
        config: Arc<MarketDataEngineConfig>,
        repository: Arc<dyn MarketDataRepository>,
        match_notification_rx: mpsc::Receiver<MatchNotification>,
        market_data_notification_tx: mpsc::Sender<MarketDataNotification>,
        market_data_request_rx: mpsc::Receiver<MarketDataRequest>,
        market_data_response_tx: mpsc::Sender<MarketDataResponse>,
    ) -> Self {
        let inner = MarketDataEngineInner {
            match_notification_rx: Some(match_notification_rx),
            market_data_request_rx: Some(market_data_request_rx),
            main_task_handle: None,
            is_running: false,
        };

        Self {
            config,
            repository,
            market_data_cache: Arc::new(DashMap::new()),
            market_data_notification_tx,
            market_data_response_tx,
            inner: Arc::new(RwLock::new(inner)),
        }
    }

    /// 启动引擎
    #[instrument(skip(self))]
    pub async fn start(&self) -> Result<(), MarketDataEngineError> {
        info!("正在启动市场行情引擎...");
        
        let mut inner = self.inner.write().await;
        
        // 检查是否已经在运行
        if inner.is_running {
            warn!("市场行情引擎已经在运行");
            return Err(MarketDataEngineError::ConfigError("引擎已在运行".to_string()));
        }

        // 初始化市场数据
        if self.config.auto_load_all_market_data {
            self.load_all_market_data().await?;
        }

        // 启动主循环
        let match_notification_rx = inner.match_notification_rx.take()
            .ok_or(MarketDataEngineError::ConfigError("撮合通知接收器未初始化".to_string()))?;
        
        let market_data_request_rx = inner.market_data_request_rx.take()
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

        inner.main_task_handle = Some(handle);
        inner.is_running = true;

        info!("市场行情引擎启动完成");
        Ok(())
    }

    /// 停止引擎
    #[instrument(skip(self))]
    pub async fn stop(&self) -> Result<(), MarketDataEngineError> {
        let mut inner = self.inner.write().await;
        
        if !inner.is_running {
            return Ok(());
        }

        info!("正在停止市场行情引擎...");
        
        // 停止主任务
        if let Some(handle) = inner.main_task_handle.take() {
            handle.abort();
            match handle.await {
                Ok(_) => info!("市场数据引擎主任务正常停止"),
                Err(e) if e.is_cancelled() => info!("市场数据引擎主任务被取消"),
                Err(e) => warn!("市场数据引擎主任务停止时出错: {}", e),
            }
        }

        inner.is_running = false;
        info!("市场行情引擎已停止");
        Ok(())
    }

    /// 检查引擎是否正在运行
    pub async fn is_running(&self) -> bool {
        self.inner.read().await.is_running
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
            }
        }
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
            MatchNotification::OrderCancelled { .. } | MatchNotification::OrderCancelRejected { .. } => {
                // 市场数据引擎通常不直接处理这些通知
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
        let stock_id = trade_execution.trade.stock_id.clone();
        
        let mut market_data_entry = match market_data_cache.get_mut(&stock_id) {
            Some(entry) => entry,
            None => {
                warn!("收到股票 {} 的成交回报，但缓存中无此股票数据", stock_id);
                return;
            }
        };

        // 更新动态数据
        market_data_entry.dynamic_data.update_with_trade(
            trade_execution.trade.price,
            trade_execution.trade.quantity,
            trade_execution.trade.timestamp,
        );

        // 发送更新通知
        let notification = MarketDataNotification::MarketDataUpdated {
            stock_id: stock_id.clone(),
            market_data: market_data_entry.clone(),
            timestamp: Utc::now(),
        };

        if let Err(e) = market_data_notification_tx.send(notification).await {
            error!("发送市场数据更新通知失败: {}", e);
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
        let static_data_list = if let Some(sid) = stock_id {
            repository.get_multiple_static_market_data(&[sid.as_ref()], trade_date).await?
        } else {
            repository.get_all_static_market_data(trade_date).await?
        };

        let loaded_count = static_data_list.len();
        for static_data in static_data_list {
            let stock_id = static_data.stock_id.clone();
            market_data_cache.entry(stock_id).and_modify(|md| {
                md.static_data = static_data.clone();
            }).or_insert_with(|| {
                MarketData::new(static_data)
            });
        }
        Ok(loaded_count)
    }
}

impl Drop for MarketDataEngine {
    fn drop(&mut self) {
        if let Some(inner) = Arc::get_mut(&mut self.inner) {
            let inner = inner.get_mut();
            if inner.is_running {
                if let Some(handle) = &inner.main_task_handle {
                    info!("Dropping MarketDataEngine, aborting main task.");
                    handle.abort();
                }
            }
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

    /// 设置配置（Arc共享）
    pub fn with_shared_config(mut self, config: Arc<MarketDataEngineConfig>) -> Self {
        self.config = (*config).clone();
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
            Arc::new(self.config),
            repository,
            match_notification_rx,
            market_data_notification_tx,
            market_data_request_rx,
            market_data_response_tx,
        ))
    }

    /// 构建引擎（使用Arc共享配置）
    pub fn build_with_shared_config(
        self,
        config: Arc<MarketDataEngineConfig>,
        match_notification_rx: mpsc::Receiver<MatchNotification>,
        market_data_notification_tx: mpsc::Sender<MarketDataNotification>,
        market_data_request_rx: mpsc::Receiver<MarketDataRequest>,
        market_data_response_tx: mpsc::Sender<MarketDataResponse>,
    ) -> Result<MarketDataEngine, MarketDataEngineError> {
        let repository = self.repository
            .ok_or(MarketDataEngineError::ConfigError("数据存储未设置".to_string()))?;

        Ok(MarketDataEngine::new(
            config,
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
        // 在当前设计中，订阅是隐式的，通过直接查询获取。
        // 此接口为未来可能的推送模型预留。
        Ok(())
    }

    async fn unsubscribe_market_data(&self, _stock_id: &str) -> Result<(), String> {
        // 同上
        Ok(())
    }

    async fn health_check(&self) -> bool {
        self.is_running().await
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
    use chrono::Utc;
    use core_entities::{Trade, OrderStatusInTrade};

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
            let mut data = self.data.write().await;
            for item in data_list {
                data.insert(item.stock_id.to_string(), item.clone());
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_market_data_engine_creation() {
        let repository: Arc<dyn MarketDataRepository> = Arc::new(MockRepository::new());
        let (match_tx, match_rx) = mpsc::channel(100);
        let (md_notification_tx, mut md_notification_rx) = mpsc::channel(100);
        let (md_req_tx, md_req_rx) = mpsc::channel(100);
        let (md_resp_tx, mut md_resp_rx) = mpsc::channel(100);
        
        let config = Arc::new(MarketDataEngineConfig::default());
        
        let engine = MarketDataEngine::new(
            config.clone(),
            repository.clone(),
            match_rx,
            md_notification_tx,
            md_req_rx,
            md_resp_tx,
        );
        
        assert!(!engine.is_running().await);
    }

    #[tokio::test]
    async fn test_market_data_engine_builder() {
        let repository: Arc<dyn MarketDataRepository> = Arc::new(MockRepository::new());
        let (_match_tx, match_rx) = mpsc::channel(100);
        let (md_notification_tx, _md_notification_rx) = mpsc::channel(100);
        let (_md_req_tx, md_req_rx) = mpsc::channel(100);
        let (md_resp_tx, _md_resp_rx) = mpsc::channel(100);
        
        let engine = MarketDataEngineBuilder::new()
            .with_trade_date(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap())
            .with_repository(repository)
            .build(match_rx, md_notification_tx, md_req_rx, md_resp_tx)
            .unwrap();
        
        assert!(!engine.is_running().await);
        assert_eq!(engine.config.trade_date, NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
    }

    #[tokio::test]
    async fn test_engine_start_stop() {
        let repository: Arc<dyn MarketDataRepository> = Arc::new(MockRepository::new());
        let (_match_tx, match_rx) = mpsc::channel(100);
        let (md_notification_tx, _md_notification_rx) = mpsc::channel(100);
        let (_md_req_tx, md_req_rx) = mpsc::channel(100);
        let (md_resp_tx, _md_resp_rx) = mpsc::channel(100);
        
        let engine = MarketDataEngineBuilder::new()
            .with_repository(repository)
            .build(match_rx, md_notification_tx, md_req_rx, md_resp_tx)
            .unwrap();
        
        engine.start().await.unwrap();
        assert!(engine.is_running().await);
        
        engine.stop().await.unwrap();
        assert!(!engine.is_running().await);
    }

    #[tokio::test]
    async fn test_trade_processing() {
        let repository = Arc::new(MockRepository::new());
        let trade_date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        repository.add_test_data("SH600036", trade_date).await;
        
        let (match_tx, match_rx) = mpsc::channel(100);
        let (md_notification_tx, mut md_notification_rx) = mpsc::channel(100);
        let (_md_req_tx, md_req_rx) = mpsc::channel(100);
        let (md_resp_tx, _md_resp_rx) = mpsc::channel(100);
        
        let engine = MarketDataEngineBuilder::new()
            .with_repository(repository.clone())
            .with_trade_date(trade_date)
            .with_auto_load_all_market_data(true)
            .build(match_rx, md_notification_tx, md_req_rx, md_resp_tx)
            .unwrap();
        
        engine.start().await.unwrap();
        
        let initial_data = engine.get_market_data("SH600036").await.unwrap();
        assert_eq!(initial_data.dynamic_data.current_price, dec!(0));

        let trade = Trade {
            id: "trade1".to_string(),
            stock_id: "SH600036".into(),
            price: dec!(10.50),
            quantity: 100,
            aggressor_order_id: "order1".into(),
            resting_order_id: "order2".into(),
            buyer_order_id: "order_buy".into(),
            seller_order_id: "order_sell".into(),
            buyer_user_id: "user_buy".into(),
            seller_user_id: "user_sell".into(),
            timestamp: Utc::now(),
        };
        
        let buyer_status = OrderStatusInTrade {
            order_id: "order_buy".into(),
            filled_quantity_in_trade: 100,
            total_filled_quantity: 100,
            remaining_quantity: 0,
            is_fully_filled: true,
        };
        
        let seller_status = OrderStatusInTrade {
            order_id: "order_sell".into(),
            filled_quantity_in_trade: 100,
            total_filled_quantity: 200,
            remaining_quantity: 50,
            is_fully_filled: false,
        };

        let trade_execution = TradeExecution {
            trade,
            buyer_status,
            seller_status,
        };

        match_tx.send(MatchNotification::TradeExecuted(trade_execution)).await.unwrap();

        // 等待通知
        let notification = tokio::time::timeout(
            tokio::time::Duration::from_secs(1),
            md_notification_rx.recv()
        ).await.unwrap().unwrap();

        if let MarketDataNotification::MarketDataUpdated { stock_id, market_data, .. } = notification {
            assert_eq!(stock_id.as_ref(), "SH600036");
            assert_eq!(market_data.dynamic_data.current_price, dec!(10.50));
            assert_eq!(market_data.dynamic_data.volume, 100);
            assert_eq!(market_data.dynamic_data.high_price, dec!(10.50));
            assert_eq!(market_data.dynamic_data.low_price, dec!(10.50));
        } else {
            panic!("Expected MarketDataUpdated notification");
        }
        
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_market_data_request_handling() {
        let repository = Arc::new(MockRepository::new());
        let trade_date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        repository.add_test_data("SH600036", trade_date).await;
        
        let (_match_tx, match_rx) = mpsc::channel(100);
        let (md_notification_tx, _md_notification_rx) = mpsc::channel(100);
        let (md_req_tx, md_req_rx) = mpsc::channel(100);
        let (md_resp_tx, mut md_resp_rx) = mpsc::channel(100);
        
        let engine = MarketDataEngineBuilder::new()
            .with_repository(repository.clone())
            .with_trade_date(trade_date)
            .with_auto_load_all_market_data(true)
            .build(match_rx, md_notification_tx, md_req_rx, md_resp_tx)
            .unwrap();
            
        engine.start().await.unwrap();
        
        // 测试获取单个市场数据
        md_req_tx.send(MarketDataRequest::GetMarketData { stock_id: "SH600036".into() }).await.unwrap();
        
        let response = tokio::time::timeout(
            tokio::time::Duration::from_secs(1),
            md_resp_rx.recv()
        ).await.unwrap().unwrap();

        if let MarketDataResponse::MarketData(data) = response {
            assert_eq!(data.static_data.stock_id.as_ref(), "SH600036");
        } else {
            panic!("Expected MarketData response");
        }
        
        // 测试获取不存在的数据
        md_req_tx.send(MarketDataRequest::GetMarketData { stock_id: "SZ000001".into() }).await.unwrap();
        
        let response_err = tokio::time::timeout(
            tokio::time::Duration::from_secs(1),
            md_resp_rx.recv()
        ).await.unwrap().unwrap();

        if let MarketDataResponse::Error { error_message, .. } = response_err {
            assert!(error_message.contains("市场数据未找到"));
        } else {
            panic!("Expected Error response");
        }

        engine.stop().await.unwrap();
    }
} 