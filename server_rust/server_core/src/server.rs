use std::borrow::Cow;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};
use tracing::{debug, error, info, warn, instrument};
use order_engine::OrderEngine;
use core_entities::app_config::{ServerConfig};
use market_data_engine::MarketDataService;

use crate::connection_manager::{ConnectionManager, ConnectionManagement, ConnectionStats};
use crate::error::{ConnectionError};

/// 默认最大连接数
const DEFAULT_MAX_CONNECTIONS: usize = 10_000;
/// 默认监听地址
const DEFAULT_LISTEN_ADDR: &'static str = "127.0.0.1:8080";

/// 服务器主组件，管理整个服务器的生命周期
pub struct Server {
    /// TCP监听器
    listener: TcpListener,
    /// 连接管理器
    connection_manager: Arc<Mutex<dyn ConnectionManagement>>,
    /// 优雅关闭信号发送器
    shutdown_tx: broadcast::Sender<()>,
    /// 优雅关闭信号接收器
    shutdown_rx: broadcast::Receiver<()>,
    /// 服务器配置
    config: ServerConfig,
    /// 订单引擎引用
    order_engine: Arc<OrderEngine>,
    /// 市场数据引擎引用
    market_data_engine: Arc<dyn MarketDataService>,
}

impl Server {
    /// 创建新的服务器实例
    #[instrument(level = "info", skip(config, order_engine, market_data_engine))]
    pub async fn new(config: ServerConfig, order_engine: Arc<OrderEngine>, market_data_engine: Arc<dyn MarketDataService>) -> Result<Self, ConnectionError> {
        info!("创建服务器实例，配置: {:?}", config);

        // 绑定TCP监听器
        let listener = match TcpListener::bind(config.listen_addr.as_ref()).await {
            Ok(listener) => {
                info!("服务器成功绑定到地址: {}", config.listen_addr);
                listener
            }
            Err(e) => {
                error!("绑定地址失败: {} - {}", config.listen_addr, e);
                return Err(ConnectionError::BindError {
                    addr: Cow::Owned(config.listen_addr.to_string()),
                    source: e,
                });
            }
        };

        // 创建关闭信号通道
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        // 创建连接管理器
        let connection_manager: Arc<Mutex<dyn ConnectionManagement>> = Arc::new(Mutex::new(
            ConnectionManager::new(config.max_connections)
        ));

        Ok(Self {
            listener,
            connection_manager,
            shutdown_tx,
            shutdown_rx,
            config,
            order_engine,
            market_data_engine,
        })
    }

    /// 主服务循环
    #[instrument(level = "info", skip(self))]
    pub async fn run(&mut self) -> Result<(), ConnectionError> {
        info!("启动服务器主循环，监听地址: {}", self.config.listen_addr);

        loop {
            tokio::select! {
                // 接受新的TCP连接
                accept_result = self.listener.accept() => {
                    match accept_result {
                        Ok((stream, addr)) => {
                            debug!("接受新连接: {}", addr);
                            if let Err(e) = self.handle_new_connection(stream, addr).await {
                                warn!("处理新连接失败: {} - {}", addr, e);
                                // 继续处理其他连接，不因单个连接失败而停止服务器
                            }
                        }
                        Err(e) => {
                            error!("接受连接时出错: {}", e);
                            // 根据错误类型决定是否继续
                            if self.should_continue_on_accept_error(&e) {
                                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                                continue;
                            } else {
                                return Err(ConnectionError::Io(e));
                            }
                        }
                    }
                }

                // 接收关闭信号
                _ = self.shutdown_rx.recv() => {
                    info!("服务器收到关闭信号，开始优雅关闭");
                    break;
                }
            }
        }

        // 执行优雅关闭
        self.graceful_shutdown().await?;
        Ok(())
    }

    /// 处理新连接
    #[instrument(level = "debug", skip(self, stream))]
    async fn handle_new_connection(&mut self, stream: TcpStream, addr: SocketAddr) -> Result<(), ConnectionError> {
        // 创建关闭信号接收器
        let shutdown_rx = self.shutdown_tx.subscribe();
        
        // 使用连接管理器创建并管理新连接
        {
            let mut manager = self.connection_manager.lock().await;
            let connection_id = manager.create_and_manage_connection(
                stream, 
                addr, 
                shutdown_rx,
                self.order_engine.clone(),
                self.market_data_engine.clone()
            ).await?;
            info!("新连接已创建: ID={}, 地址={}", connection_id, addr);
        }

        Ok(())
    }

    /// 判断在接受连接出错时是否应该继续运行
    fn should_continue_on_accept_error(&self, error: &std::io::Error) -> bool {
        use std::io::ErrorKind;
        
        match error.kind() {
            // 这些错误通常是临时的，可以继续运行
            ErrorKind::WouldBlock |
            ErrorKind::Interrupted |
            ErrorKind::TimedOut => true,
            
            // 这些错误通常是严重的，应该停止服务器
            ErrorKind::PermissionDenied |
            ErrorKind::AddrInUse |
            ErrorKind::AddrNotAvailable => false,
            
            // 其他错误，保守起见选择继续运行
            _ => {
                warn!("遇到未知的接受连接错误: {}", error);
                true
            }
        }
    }

    /// 执行优雅关闭
    #[instrument(level = "info", skip(self))]
    async fn graceful_shutdown(&mut self) -> Result<(), ConnectionError> {
        info!("开始执行优雅关闭流程");

        // 广播关闭信号给所有连接
        let _ = self.shutdown_tx.send(());
        
        // 等待所有连接关闭
        {
            let mut manager = self.connection_manager.lock().await;
            manager.shutdown_all().await;
        }

        info!("优雅关闭完成");
        Ok(())
    }

    /// 触发优雅关闭
    pub async fn shutdown(&self) {
        info!("触发服务器关闭");
        let _ = self.shutdown_tx.send(());
    }

    /// 获取服务器统计信息
    pub async fn get_stats(&self) -> ServerStats {
        let manager = self.connection_manager.lock().await;
        let connection_stats = manager.get_stats();
        
        ServerStats {
            listen_addr: Cow::Owned(self.config.listen_addr.to_string()),
            connection_stats,
        }
    }

    /// 获取监听地址
    pub fn listen_addr(&self) -> &str {
        &self.config.listen_addr
    }

    /// 获取本地地址（实际绑定的地址）
    pub fn local_addr(&self) -> Result<SocketAddr, std::io::Error> {
        self.listener.local_addr()
    }
}

/// 服务器统计信息
#[derive(Debug, Clone)]
pub struct ServerStats {
    /// 监听地址
    pub listen_addr: Cow<'static, str>,
    /// 连接统计
    pub connection_stats: ConnectionStats,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};
    use core_entities::app_config::*;

    
} 