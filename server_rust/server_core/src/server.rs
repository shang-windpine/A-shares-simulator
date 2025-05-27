use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};
use tracing::{debug, error, info, warn, instrument};

use crate::connection_manager::{ConnectionManager, ConnectionManagement, ConnectionStats};
use crate::error::{ConnectionError};

/// 默认最大连接数
const DEFAULT_MAX_CONNECTIONS: usize = 10_000;
/// 默认监听地址
const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:8080";

/// 服务器配置
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// 监听地址
    pub listen_addr: String,
    /// 最大连接数
    pub max_connections: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: DEFAULT_LISTEN_ADDR.to_string(),
            max_connections: DEFAULT_MAX_CONNECTIONS,
        }
    }
}

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
}

impl Server {
    /// 创建新的服务器实例
    #[instrument(level = "info")]
    pub async fn new(config: ServerConfig) -> Result<Self, ConnectionError> {
        info!("创建服务器实例，配置: {:?}", config);

        // 绑定TCP监听器
        let listener = match TcpListener::bind(&config.listen_addr).await {
            Ok(listener) => {
                info!("服务器成功绑定到地址: {}", config.listen_addr);
                listener
            }
            Err(e) => {
                error!("绑定地址失败: {} - {}", config.listen_addr, e);
                return Err(ConnectionError::BindError {
                    addr: config.listen_addr.clone(),
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
        })
    }

    /// 使用默认配置创建服务器
    pub async fn new_with_defaults() -> Result<Self, ConnectionError> {
        Self::new(ServerConfig::default()).await
    }

    /// 使用指定地址创建服务器
    pub async fn new_with_addr(addr: &str) -> Result<Self, ConnectionError> {
        let config = ServerConfig {
            listen_addr: addr.to_string(),
            ..Default::default()
        };
        Self::new(config).await
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
            let connection_id = manager.create_and_manage_connection(stream, addr, shutdown_rx).await?;
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
            listen_addr: self.config.listen_addr.clone(),
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
    pub listen_addr: String,
    /// 连接统计
    pub connection_stats: ConnectionStats,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_server_creation() {
        let config = ServerConfig {
            listen_addr: "127.0.0.1:0".to_string(), // 使用端口0让系统自动分配
            max_connections: 100,
        };
        
        let server = Server::new(config).await;
        assert!(server.is_ok());
        
        let server = server.unwrap();
        assert!(server.local_addr().is_ok());
    }

    #[tokio::test]
    async fn test_server_with_defaults() {
        // 注意：这个测试可能会失败如果默认端口被占用
        // 在实际测试中，应该使用随机端口
        let server = Server::new_with_addr("127.0.0.1:0").await;
        assert!(server.is_ok());
    }

    #[tokio::test]
    async fn test_server_shutdown() {
        let mut server = Server::new_with_addr("127.0.0.1:0").await.unwrap();
        
        // 在另一个任务中运行服务器
        let server_handle = {
            let shutdown_trigger = server.shutdown_tx.clone();
            tokio::spawn(async move {
                // 稍等一下然后触发关闭
                sleep(Duration::from_millis(10)).await;
                let _ = shutdown_trigger.send(());
            })
        };
        
        // 运行服务器（应该很快结束）
        let result = tokio::time::timeout(Duration::from_secs(1), server.run()).await;
        assert!(result.is_ok());
        
        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_server_stats() {
        let server = Server::new_with_addr("127.0.0.1:0").await.unwrap();
        let stats = server.get_stats().await;
        
        assert_eq!(stats.connection_stats.active_count, 0);
        assert!(stats.connection_stats.max_connections > 0);
    }
} 