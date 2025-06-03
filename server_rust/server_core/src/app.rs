use std::sync::Arc;
use tracing::{info, error};
use order_engine::{OrderEngine, OrderEngineConfig, OrderEngineFactory};
use matching_engine::engine::MatchingEngine;
use market_data_engine::engine::{MarketDataEngine, MarketDataEngineConfig};
use market_data_engine::data_types::{MarketDataNotification, MarketDataRequest, MarketDataResponse};
use market_data_engine::database::{MySqlMarketDataRepository, DatabaseConfig};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::{Server, ServerConfig};
use crate::error::ConnectionError;

/// 应用程序配置
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// 服务器配置
    pub server_config: ServerConfig,
    /// 订单引擎配置
    pub order_engine_config: OrderEngineConfig,
    /// 市场数据引擎配置
    pub market_data_engine_config: MarketDataEngineConfig,
    /// 数据库配置
    pub database_config: DatabaseConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server_config: ServerConfig::default(),
            order_engine_config: OrderEngineConfig::default(),
            market_data_engine_config: MarketDataEngineConfig::default(),
            database_config: DatabaseConfig::default(),
        }
    }
}

/// 应用程序服务
pub struct AppServices {
    /// 订单引擎实例
    pub order_engine: Arc<OrderEngine>,
    /// 撮合引擎实例
    pub matching_engine: MatchingEngine,
    /// 市场数据引擎实例
    pub market_data_engine: MarketDataEngine,
}

/// 应用程序主入口，负责管理各种服务的生命周期
pub struct App {
    /// 应用配置
    config: AppConfig,
    /// 已初始化的服务
    services: Option<AppServices>,
}

impl App {
    /// 创建新的应用程序实例
    pub fn new(config: AppConfig) -> Self {
        Self {
            config,
            services: None,
        }
    }

    /// 使用默认配置创建应用程序
    pub fn with_defaults() -> Self {
        Self::new(AppConfig::default())
    }

    /// 初始化所有服务
    pub async fn initialize_services(&mut self) -> Result<(), ConnectionError> {
        info!("开始初始化应用服务");

        // 1. 使用工厂方法创建订单引擎和相关channels
        let (order_engine, order_notification_rx, match_notification_tx) = 
            OrderEngineFactory::create_with_channels(None, self.config.order_engine_config.clone());
        
        info!("订单引擎初始化完成");

        // 2. 创建撮合引擎
        let matching_engine = MatchingEngine::new(order_notification_rx, match_notification_tx);
        info!("撮合引擎初始化完成");

        // 3. 创建市场数据引擎相关的channels
        let (market_data_notification_tx, _market_data_notification_rx) = 
            mpsc::channel::<MarketDataNotification>(self.config.market_data_engine_config.notification_buffer_size);
        let (market_data_request_tx, market_data_request_rx) = 
            mpsc::channel::<MarketDataRequest>(self.config.market_data_engine_config.request_buffer_size);
        let (market_data_response_tx, _market_data_response_rx) = 
            mpsc::channel::<MarketDataResponse>(self.config.market_data_engine_config.request_buffer_size);

        // 4. 创建市场数据引擎的MySQL仓库
        let repository = Arc::new(MySqlMarketDataRepository::new(
            self.config.database_config.clone()
        ).await.map_err(|e| ConnectionError::application_with_source(
            "创建MySQL仓库失败".to_string(), 
            e
        ))?);

        // 5. 创建市场数据引擎
        let market_data_engine = MarketDataEngine::new(
            self.config.market_data_engine_config.clone(),
            repository,
            mpsc::channel(100).1, // 临时的match_notification_rx，后续需要连接到真实的撮合引擎
            market_data_notification_tx,
            market_data_request_rx,
            market_data_response_tx,
        );
        info!("市场数据引擎初始化完成");

        // 6. 存储服务引用
        self.services = Some(AppServices {
            order_engine: Arc::new(order_engine),
            matching_engine,
            market_data_engine,
        });

        info!("所有服务初始化完成");
        Ok(())
    }

    /// 启动所有服务
    pub async fn start_services(&mut self) -> Result<(), ConnectionError> {
        if let Some(services) = &mut self.services {
            info!("启动业务服务");

            // 启动订单引擎
            services.order_engine.start().await
                .map_err(|e| ConnectionError::Application(format!("订单引擎启动失败: {}", e)))?;
            info!("订单引擎已启动");

            // 启动撮合引擎
            services.matching_engine.start().await
                .map_err(|e| ConnectionError::Application(format!("撮合引擎启动失败: {}", e)))?;
            info!("撮合引擎已启动");

            // 启动市场数据引擎
            services.market_data_engine.start().await
                .map_err(|e| ConnectionError::Application(format!("市场数据引擎启动失败: {}", e)))?;
            info!("市场数据引擎已启动");

            info!("所有业务服务启动完成");
        } else {
            return Err(ConnectionError::Application("服务未初始化".to_string()));
        }
        
        Ok(())
    }

    /// 启动应用程序
    pub async fn run(&mut self) -> Result<(), ConnectionError> {
        // 确保服务已初始化
        if self.services.is_none() {
            self.initialize_services().await?;
        }

        // 启动业务服务
        self.start_services().await?;

        let services = self.services.as_ref().unwrap();

        // 创建并启动服务器
        info!("启动网络服务器");
        let mut server = Server::new_with_services(
            self.config.server_config.clone(),
            services.order_engine.clone(),
        ).await?;

        // 运行服务器主循环
        server.run().await
    }

    /// 获取服务引用（用于测试或外部访问）
    pub fn services(&self) -> Option<&AppServices> {
        self.services.as_ref()
    }

    /// 优雅关闭
    pub async fn shutdown(&mut self) -> Result<(), ConnectionError> {
        info!("开始应用程序优雅关闭");

        if let Some(services) = &mut self.services {
            // 关闭市场数据引擎
            if let Err(e) = services.market_data_engine.stop().await {
                error!("市场数据引擎关闭失败: {}", e);
            } else {
                info!("市场数据引擎正常关闭");
            }

            // 关闭撮合引擎
            if let Err(e) = services.matching_engine.stop().await {
                error!("撮合引擎关闭失败: {}", e);
            } else {
                info!("撮合引擎正常关闭");
            }

            // 关闭订单引擎
            if let Err(e) = services.order_engine.stop().await {
                error!("订单引擎关闭失败: {}", e);
            } else {
                info!("订单引擎正常关闭");
            }

            info!("清理业务服务完成");
        }

        info!("应用程序关闭完成");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_app_creation() {
        let mut app = App::with_defaults();
        let result = app.initialize_services().await;
        assert!(result.is_ok());
        assert!(app.services().is_some());
    }

    #[tokio::test]
    async fn test_app_services() {
        let mut app = App::with_defaults();
        app.initialize_services().await.unwrap();
        
        let services = app.services().unwrap();
        // 测试服务是否正确初始化
        assert!(Arc::strong_count(&services.order_engine) >= 1);
    }

    #[tokio::test]
    async fn test_app_service_startup() {
        let mut app = App::with_defaults();
        app.initialize_services().await.unwrap();
        
        // 测试服务启动
        let result = app.start_services().await;
        assert!(result.is_ok());
        
        // 验证服务运行状态
        let services = app.services().unwrap();
        assert!(services.matching_engine.is_running());
        assert!(services.order_engine.is_running().await);
        
        // 清理
        app.shutdown().await.unwrap();
    }
}

// 临时的模拟市场数据仓库实现
use market_data_engine::database::{MarketDataRepository, DatabaseError};
use market_data_engine::data_types::{StaticMarketData, MarketData};
use chrono::NaiveDate;

struct MockMarketDataRepository;

impl MockMarketDataRepository {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl MarketDataRepository for MockMarketDataRepository {
    async fn get_static_market_data(
        &self,
        stock_id: &str,
        trade_date: NaiveDate,
    ) -> Result<StaticMarketData, DatabaseError> {
        Err(DatabaseError::StaticDataNotFound {
            stock_id: stock_id.to_string(),
            trade_date,
        })
    }

    async fn get_multiple_static_market_data(
        &self,
        _stock_ids: &[&str],
        _trade_date: NaiveDate,
    ) -> Result<Vec<StaticMarketData>, DatabaseError> {
        Ok(vec![])
    }

    async fn get_all_static_market_data(
        &self,
        _trade_date: NaiveDate,
    ) -> Result<Vec<StaticMarketData>, DatabaseError> {
        Ok(vec![])
    }

    async fn get_complete_market_data(
        &self,
        stock_id: &str,
        trade_date: NaiveDate,
    ) -> Result<MarketData, DatabaseError> {
        Err(DatabaseError::StaticDataNotFound {
            stock_id: stock_id.to_string(),
            trade_date,
        })
    }

    async fn get_multiple_complete_market_data(
        &self,
        _stock_ids: &[&str],
        _trade_date: NaiveDate,
    ) -> Result<Vec<MarketData>, DatabaseError> {
        Ok(vec![])
    }

    async fn get_all_complete_market_data(
        &self,
        _trade_date: NaiveDate,
    ) -> Result<Vec<MarketData>, DatabaseError> {
        Ok(vec![])
    }

    async fn save_static_market_data(
        &self,
        _data: &StaticMarketData,
    ) -> Result<(), DatabaseError> {
        Ok(())
    }

    async fn save_multiple_static_market_data(
        &self,
        _data_list: &[StaticMarketData],
    ) -> Result<(), DatabaseError> {
        Ok(())
    }
} 