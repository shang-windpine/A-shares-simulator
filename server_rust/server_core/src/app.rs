use std::sync::Arc;
use tracing::{info, error};
use order_engine::{OrderEngine, OrderEngineConfig, OrderEngineFactory};
use matching_engine::engine::MatchingEngine;
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
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server_config: ServerConfig::default(),
            order_engine_config: OrderEngineConfig::default(),
        }
    }
}

/// 应用程序服务
pub struct AppServices {
    /// 订单引擎实例
    pub order_engine: Arc<OrderEngine>,
    /// 撮合引擎实例
    pub matching_engine: MatchingEngine,
    /// 订单引擎任务句柄
    pub order_engine_handle: Option<JoinHandle<Result<(), String>>>,
    /// 撮合引擎任务句柄
    pub matching_engine_handle: Option<JoinHandle<()>>,
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

        // 3. 存储服务引用
        self.services = Some(AppServices {
            order_engine: Arc::new(order_engine),
            matching_engine,
            order_engine_handle: None,
            matching_engine_handle: None,
        });

        info!("所有服务初始化完成");
        Ok(())
    }

    /// 启动所有服务
    pub async fn start_services(&mut self) -> Result<(), ConnectionError> {
        if let Some(services) = &mut self.services {
            info!("启动业务服务");

            // 启动撮合引擎
            let mut matching_engine = std::mem::replace(&mut services.matching_engine, 
                MatchingEngine::new(mpsc::channel(1).1, mpsc::channel(1).0)); // 临时替换
            
            let matching_engine_handle = tokio::spawn(async move {
                matching_engine.run().await;
            });
            services.matching_engine_handle = Some(matching_engine_handle);
            info!("撮合引擎已启动");

            // 注意：OrderEngine 不需要单独启动，它是通过方法调用来工作的
            // 但是如果需要启动清理任务等，可以在这里添加

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
            // 关闭撮合引擎
            if let Some(handle) = services.matching_engine_handle.take() {
                info!("关闭撮合引擎");
                handle.abort();
                match handle.await {
                    Ok(_) => info!("撮合引擎正常关闭"),
                    Err(e) if e.is_cancelled() => info!("撮合引擎被取消"),
                    Err(e) => error!("撮合引擎关闭时出错: {}", e),
                }
            }

            // 关闭订单引擎（如果有需要）
            if let Some(handle) = services.order_engine_handle.take() {
                info!("关闭订单引擎");
                handle.abort();
                match handle.await {
                    Ok(_) => info!("订单引擎正常关闭"),
                    Err(e) if e.is_cancelled() => info!("订单引擎被取消"),
                    Err(e) => error!("订单引擎关闭时出错: {}", e),
                }
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
        
        // 验证服务句柄存在
        let services = app.services().unwrap();
        assert!(services.matching_engine_handle.is_some());
        
        // 清理
        app.shutdown().await.unwrap();
    }
} 