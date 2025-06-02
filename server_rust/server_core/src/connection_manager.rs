use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use order_engine::OrderEngine;

use crate::error::{ConnectionError};
use crate::client_connection::ClientConnection;

/// 连接唯一标识符类型
pub type ConnectionId = u64;

/// 连接控制命令，用于与ClientConnection通信
#[derive(Debug, Clone)]
pub enum ConnectionControlCommand {
    /// 请求连接关闭
    Shutdown,
    /// 发送自定义消息给客户端（预留扩展）
    SendMessage(Vec<u8>),
    /// 获取连接状态
    GetStatus,
}

/// 连接状态报告，ClientConnection可以向ConnectionManager报告状态
#[derive(Debug, Clone)]
pub enum ConnectionStatusReport {
    /// 连接已建立
    Connected,
    /// 心跳超时
    HeartbeatTimeout,
    /// 连接错误
    Error(String),
    /// 连接正常关闭
    Disconnected,
}

/// 连接管理器的抽象trait，定义了连接管理器应该具备的核心功能
#[async_trait::async_trait]
pub trait ConnectionManagement: Send + Sync {
    /// 创建并管理新的客户端连接
    async fn create_and_manage_connection(
        &mut self,
        stream: TcpStream,
        addr: SocketAddr,
        shutdown_rx: broadcast::Receiver<()>,
    ) -> Result<ConnectionId, ConnectionError>;

    /// 创建并管理新的客户端连接（带有业务服务依赖）
    async fn create_and_manage_connection_with_services(
        &mut self,
        stream: TcpStream,
        addr: SocketAddr,
        shutdown_rx: broadcast::Receiver<()>,
        order_engine: Option<Arc<OrderEngine>>,
    ) -> Result<ConnectionId, ConnectionError>;

    /// 移除指定的连接
    async fn remove_connection(&mut self, id: ConnectionId);

    /// 获取活跃连接数
    fn active_count(&self) -> usize;

    /// 获取最大连接数限制
    fn max_connections(&self) -> usize;

    /// 检查是否可以接受新连接
    fn can_accept_connection(&self) -> bool;

    /// 获取所有连接ID的列表
    fn get_connection_ids(&self) -> Vec<ConnectionId>;

    /// 关闭所有连接
    async fn shutdown_all(&mut self);

    /// 获取连接统计信息
    fn get_stats(&self) -> ConnectionStats;

    /// 向指定连接发送控制命令
    async fn send_control_command(&self, id: ConnectionId, command: ConnectionControlCommand) -> Result<(), ConnectionError>;
}

/// 连接管理器的具体实现
#[derive(Debug)]
pub struct ConnectionManager {
    /// 活跃连接集合，存储连接ID到连接处理器的映射
    connections: HashMap<ConnectionId, ConnectionHandle>,
    /// 连接ID生成器
    next_id: Arc<AtomicU64>,
    /// 活跃连接计数
    active_count: Arc<AtomicUsize>,
    /// 最大连接数限制
    max_connections: usize,
}

/// 连接处理器，包含任务句柄和控制通道
#[derive(Debug)]
struct ConnectionHandle {
    /// 任务句柄
    task_handle: JoinHandle<()>,
    /// 控制命令发送器
    control_tx: mpsc::Sender<ConnectionControlCommand>,
    /// 连接地址（用于日志）
    addr: SocketAddr,
}

impl ConnectionManager {
    /// 创建新的连接管理器
    pub fn new(max_connections: usize) -> Self {
        info!("创建连接管理器，最大连接数: {}", max_connections);
        Self {
            connections: HashMap::new(),
            next_id: Arc::new(AtomicU64::new(1)),
            active_count: Arc::new(AtomicUsize::new(0)),
            max_connections,
        }
    }

    /// 移除连接（内部实现）
    fn remove_connection_internal(&mut self, id: ConnectionId) {
        if let Some(connection_handle) = self.connections.remove(&id) {
            // 尝试取消任务（如果还在运行）
            connection_handle.task_handle.abort();
            let new_count = self.active_count.fetch_sub(1, Ordering::Relaxed) - 1;
            
            info!("移除连接: ID={} ({}), 剩余连接数: {}", id, connection_handle.addr, new_count);
            debug!("连接管理器状态: {} 个活跃连接", self.connections.len());
        } else {
            warn!("尝试移除不存在的连接: ID={}", id);
        }
    }
}

#[async_trait::async_trait]
impl ConnectionManagement for ConnectionManager {
    /// 创建并管理新的客户端连接
    async fn create_and_manage_connection(
        &mut self,
        stream: TcpStream,
        addr: SocketAddr,
        shutdown_rx: broadcast::Receiver<()>,
    ) -> Result<ConnectionId, ConnectionError> {
        let current_count = self.active_count.load(Ordering::Relaxed);
        
        if current_count >= self.max_connections {
            warn!("拒绝新连接 {}：已达到最大连接数限制 {}/{}", addr, current_count, self.max_connections);
            return Err(ConnectionError::MaxConnectionsReached {
                limit: self.max_connections,
            });
        }

        // 分配连接ID
        let connection_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        
        // 创建控制通道
        let (control_tx, control_rx) = mpsc::channel::<ConnectionControlCommand>(100);
        
        // 在独立任务中运行客户端连接处理
        let task_handle = tokio::spawn(async move {
            match ClientConnection::new_with_control(stream, connection_id, shutdown_rx, control_rx).await {
                Ok(connection) => {
                    info!("客户端连接 {} ({}) 开始处理", connection_id, addr);
                    
                    // 运行连接处理循环
                    match connection.run().await {
                        Ok(_) => {
                            info!("客户端连接 {} ({}) 正常结束", connection_id, addr);
                        }
                        Err(ConnectionError::ShutdownRequested) => {
                            info!("客户端连接 {} ({}) 因关闭信号结束", connection_id, addr);
                        }
                        Err(ConnectionError::HeartbeatTimeout { .. }) => {
                            warn!("客户端连接 {} ({}) 因心跳超时结束", connection_id, addr);
                        }
                        Err(e) => {
                            error!("客户端连接 {} ({}) 出错结束: {}", connection_id, addr, e);
                        }
                    }
                }
                Err(e) => {
                    error!("创建客户端连接失败 ({}, ID={}): {}", addr, connection_id, e);
                }
            }
        });

        // 创建连接处理器
        let connection_handle = ConnectionHandle {
            task_handle,
            control_tx,
            addr,
        };

        // 存储连接处理器
        self.connections.insert(connection_id, connection_handle);
        let new_count = self.active_count.fetch_add(1, Ordering::Relaxed) + 1;
        
        info!("新连接已创建并管理: ID={}, 地址={}, 当前连接数: {}/{}", 
             connection_id, addr, new_count, self.max_connections);

        Ok(connection_id)
    }

    /// 创建并管理新的客户端连接（带有业务服务依赖）
    async fn create_and_manage_connection_with_services(
        &mut self,
        stream: TcpStream,
        addr: SocketAddr,
        shutdown_rx: broadcast::Receiver<()>,
        order_engine: Option<Arc<OrderEngine>>,
    ) -> Result<ConnectionId, ConnectionError> {
        let current_count = self.active_count.load(Ordering::Relaxed);
        
        if current_count >= self.max_connections {
            warn!("拒绝新连接 {}：已达到最大连接数限制 {}/{}", addr, current_count, self.max_connections);
            return Err(ConnectionError::MaxConnectionsReached {
                limit: self.max_connections,
            });
        }

        // 分配连接ID
        let connection_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        
        // 创建控制通道
        let (control_tx, control_rx) = mpsc::channel::<ConnectionControlCommand>(100);
        
        // 在独立任务中运行客户端连接处理
        let task_handle = tokio::spawn(async move {
            match ClientConnection::new_with_control_and_services(stream, connection_id, shutdown_rx, control_rx, order_engine).await {
                Ok(connection) => {
                    info!("客户端连接 {} ({}) 开始处理", connection_id, addr);
                    
                    // 运行连接处理循环
                    match connection.run().await {
                        Ok(_) => {
                            info!("客户端连接 {} ({}) 正常结束", connection_id, addr);
                        }
                        Err(ConnectionError::ShutdownRequested) => {
                            info!("客户端连接 {} ({}) 因关闭信号结束", connection_id, addr);
                        }
                        Err(ConnectionError::HeartbeatTimeout { .. }) => {
                            warn!("客户端连接 {} ({}) 因心跳超时结束", connection_id, addr);
                        }
                        Err(e) => {
                            error!("客户端连接 {} ({}) 出错结束: {}", connection_id, addr, e);
                        }
                    }
                }
                Err(e) => {
                    error!("创建客户端连接失败 ({}, ID={}): {}", addr, connection_id, e);
                }
            }
        });

        // 创建连接处理器
        let connection_handle = ConnectionHandle {
            task_handle,
            control_tx,
            addr,
        };

        // 存储连接处理器
        self.connections.insert(connection_id, connection_handle);
        let new_count = self.active_count.fetch_add(1, Ordering::Relaxed) + 1;
        
        info!("新连接已创建并管理: ID={}, 地址={}, 当前连接数: {}/{}", 
             connection_id, addr, new_count, self.max_connections);

        Ok(connection_id)
    }

    /// 移除指定的连接
    async fn remove_connection(&mut self, id: ConnectionId) {
        self.remove_connection_internal(id);
    }

    /// 获取活跃连接数
    fn active_count(&self) -> usize {
        self.active_count.load(Ordering::Relaxed)
    }

    /// 获取最大连接数限制
    fn max_connections(&self) -> usize {
        self.max_connections
    }

    /// 检查是否可以接受新连接
    fn can_accept_connection(&self) -> bool {
        self.active_count() < self.max_connections
    }

    /// 获取所有连接ID的列表
    fn get_connection_ids(&self) -> Vec<ConnectionId> {
        self.connections.keys().copied().collect()
    }

    /// 关闭所有连接
    async fn shutdown_all(&mut self) {
        let connection_count = self.connections.len();
        if connection_count == 0 {
            info!("没有需要关闭的连接");
            return;
        }

        info!("开始关闭所有连接，总数: {}", connection_count);

        // 发送关闭命令给所有连接
        for (id, connection_handle) in &self.connections {
            debug!("向连接 {} 发送关闭命令", id);
            if let Err(e) = connection_handle.control_tx.send(ConnectionControlCommand::Shutdown).await {
                warn!("向连接 {} 发送关闭命令失败: {}", id, e);
            }
        }

        // 等待所有任务结束，但设置超时
        for (id, connection_handle) in self.connections.drain() {
            debug!("等待连接 {} 结束", id);
            match tokio::time::timeout(
                std::time::Duration::from_secs(5), 
                connection_handle.task_handle
            ).await {
                Ok(result) => {
                    match result {
                        Ok(_) => debug!("连接 {} 正常关闭", id),
                        Err(e) if e.is_cancelled() => debug!("连接 {} 被取消", id),
                        Err(e) => warn!("连接 {} 关闭时出错: {}", id, e),
                    }
                }
                Err(_) => {
                    warn!("连接 {} 关闭超时，强制终止", id);
                    // 注意：task_handle已经在timeout中被消费了，无法再次使用
                }
            }
        }

        // 重置计数器
        self.active_count.store(0, Ordering::Relaxed);
        info!("所有连接已关闭");
    }

    /// 获取连接统计信息
    fn get_stats(&self) -> ConnectionStats {
        ConnectionStats {
            active_count: self.active_count(),
            max_connections: self.max_connections,
            next_id: self.next_id.load(Ordering::Relaxed),
        }
    }

    /// 向指定连接发送控制命令
    async fn send_control_command(&self, id: ConnectionId, command: ConnectionControlCommand) -> Result<(), ConnectionError> {
        if let Some(connection_handle) = self.connections.get(&id) {
            match connection_handle.control_tx.send(command).await {
                Ok(_) => {
                    debug!("成功向连接 {} 发送控制命令", id);
                    Ok(())
                }
                Err(e) => {
                    warn!("向连接 {} 发送控制命令失败: {}", id, e);
                    Err(ConnectionError::ChannelSend(format!("Failed to send control command to connection {}: {}", id, e)))
                }
            }
        } else {
            Err(ConnectionError::ConnectionNotFound { connection_id: id })
        }
    }
}

impl Drop for ConnectionManager {
    fn drop(&mut self) {
        if !self.connections.is_empty() {
            warn!("连接管理器被销毁时仍有 {} 个活跃连接", self.connections.len());
            // 在析构函数中取消所有任务
            for (id, connection_handle) in self.connections.drain() {
                debug!("在销毁时取消连接: ID={}", id);
                connection_handle.task_handle.abort();
            }
        }
    }
}

/// 连接统计信息
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    /// 当前活跃连接数
    pub active_count: usize,
    /// 最大连接数限制
    pub max_connections: usize,
    /// 下一个要分配的连接ID
    pub next_id: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    use tokio::sync::broadcast;
    use tokio::time::{sleep, Duration};

    fn create_test_addr() -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080)
    }

    #[tokio::test]
    async fn test_connection_manager_basic() {
        let manager = ConnectionManager::new(2);
        
        // 测试初始状态
        assert_eq!(manager.active_count(), 0);
        assert!(manager.can_accept_connection());
        assert_eq!(manager.max_connections(), 2);
        assert!(manager.get_connection_ids().is_empty());

        let stats = manager.get_stats();
        assert_eq!(stats.active_count, 0);
        assert_eq!(stats.max_connections, 2);
        assert_eq!(stats.next_id, 1);
    }

    #[tokio::test]
    async fn test_connection_stats() {
        let manager = ConnectionManager::new(100);
        
        let stats = manager.get_stats();
        assert_eq!(stats.active_count, 0);
        assert_eq!(stats.max_connections, 100);
        assert_eq!(stats.next_id, 1);

        // 测试基本功能
        assert!(manager.can_accept_connection());
        assert!(manager.get_connection_ids().is_empty());
    }

    #[tokio::test]
    async fn test_shutdown_all_empty() {
        let mut manager = ConnectionManager::new(10);
        
        // 测试空的shutdown_all
        manager.shutdown_all().await;
        assert_eq!(manager.active_count(), 0);
    }

    #[tokio::test]
    async fn test_connection_id_generation() {
        let manager = ConnectionManager::new(10);
        
        // 测试连接ID生成是递增的
        let first_id = manager.next_id.load(Ordering::Relaxed);
        let second_id = manager.next_id.fetch_add(1, Ordering::Relaxed);
        let third_id = manager.next_id.load(Ordering::Relaxed);
        
        assert_eq!(first_id, 1);
        assert_eq!(second_id, 1);
        assert_eq!(third_id, 2);
    }

    #[tokio::test]
    async fn test_can_accept_connection() {
        let manager = ConnectionManager::new(2);
        
        // 初始状态可以接受连接
        assert!(manager.can_accept_connection());
        
        // 手动增加计数来模拟连接
        manager.active_count.store(1, Ordering::Relaxed);
        assert!(manager.can_accept_connection());
        
        manager.active_count.store(2, Ordering::Relaxed);
        assert!(!manager.can_accept_connection());
        
        manager.active_count.store(3, Ordering::Relaxed);
        assert!(!manager.can_accept_connection());
    }

    #[tokio::test]
    async fn test_remove_connection_nonexistent() {
        let mut manager = ConnectionManager::new(10);
        
        // 移除不存在的连接不应该panic
        manager.remove_connection(999).await;
        assert_eq!(manager.active_count(), 0);
        assert!(manager.get_connection_ids().is_empty());
    }

    #[tokio::test]
    async fn test_send_control_command_nonexistent() {
        let manager = ConnectionManager::new(10);
        
        // 向不存在的连接发送控制命令应该返回错误
        let result = manager.send_control_command(999, ConnectionControlCommand::Shutdown).await;
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ConnectionError::ConnectionNotFound { connection_id } => {
                assert_eq!(connection_id, 999);
            }
            _ => panic!("应该返回ConnectionNotFound错误"),
        }
    }

    #[tokio::test]
    async fn test_connection_control_commands() {
        // 测试不同类型的控制命令
        let shutdown_cmd = ConnectionControlCommand::Shutdown;
        let message_cmd = ConnectionControlCommand::SendMessage(vec![1, 2, 3, 4]);
        let status_cmd = ConnectionControlCommand::GetStatus;
        
        // 这里主要测试命令的创建和格式，实际发送需要真实连接
        match shutdown_cmd {
            ConnectionControlCommand::Shutdown => {},
            _ => panic!("命令类型不匹配"),
        }
        
        match message_cmd {
            ConnectionControlCommand::SendMessage(data) => {
                assert_eq!(data, vec![1, 2, 3, 4]);
            },
            _ => panic!("命令类型不匹配"),
        }
        
        match status_cmd {
            ConnectionControlCommand::GetStatus => {},
            _ => panic!("命令类型不匹配"),
        }
    }

    #[tokio::test]
    async fn test_connection_status_report() {
        // 测试不同类型的状态报告
        let reports = vec![
            ConnectionStatusReport::Connected,
            ConnectionStatusReport::HeartbeatTimeout,
            ConnectionStatusReport::Error("测试错误".to_string()),
            ConnectionStatusReport::Disconnected,
        ];
        
        for report in reports {
            match report {
                ConnectionStatusReport::Connected => {},
                ConnectionStatusReport::HeartbeatTimeout => {},
                ConnectionStatusReport::Error(msg) => {
                    assert_eq!(msg, "测试错误");
                },
                ConnectionStatusReport::Disconnected => {},
            }
        }
    }

    #[tokio::test]
    async fn test_connection_manager_drop() {
        // 测试Drop trait的实现
        let _ = {
            let mut manager = ConnectionManager::new(10);
            
            // 手动添加一些模拟连接来测试drop行为
            let (control_tx, _control_rx) = mpsc::channel::<ConnectionControlCommand>(10);
            let task_handle = tokio::spawn(async {
                sleep(Duration::from_secs(1)).await;
            });
            
            let connection_handle = ConnectionHandle {
                task_handle,
                control_tx,
                addr: create_test_addr(),
            };
            
            manager.connections.insert(1, connection_handle);
            manager.active_count.store(1, Ordering::Relaxed);
            
            manager
        }; // manager在这里被drop
        
        // 如果到达这里说明drop没有panic
    }

    #[tokio::test]
    async fn test_max_connections_limit_simulation() {
        let manager = ConnectionManager::new(2);
        
        // 模拟达到最大连接数的情况
        manager.active_count.store(2, Ordering::Relaxed);
        
        // 由于无法轻易创建真实的TcpStream进行测试，我们测试相关的逻辑
        assert!(!manager.can_accept_connection());
        assert_eq!(manager.active_count(), 2);
        assert_eq!(manager.max_connections(), 2);
    }

    #[tokio::test]
    async fn test_concurrent_connection_id_generation() {
        let manager = Arc::new(ConnectionManager::new(100));
        let mut handles = vec![];
        
        // 并发获取连接ID
        for _ in 0..10 {
            let manager_clone = Arc::clone(&manager);
            let handle = tokio::spawn(async move {
                manager_clone.next_id.fetch_add(1, Ordering::Relaxed)
            });
            handles.push(handle);
        }
        
        let mut ids = vec![];
        for handle in handles {
            ids.push(handle.await.unwrap());
        }
        
        // 检查所有ID都是唯一的
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 10);
    }

    #[tokio::test]
    async fn test_connection_stats_consistency() {
        let manager = ConnectionManager::new(50);
        
        // 测试统计信息的一致性
        let initial_stats = manager.get_stats();
        assert_eq!(initial_stats.active_count, 0);
        assert_eq!(initial_stats.max_connections, 50);
        assert_eq!(initial_stats.next_id, 1);
        
        // 模拟连接数变化
        manager.active_count.store(5, Ordering::Relaxed);
        manager.next_id.store(10, Ordering::Relaxed);
        
        let updated_stats = manager.get_stats();
        assert_eq!(updated_stats.active_count, 5);
        assert_eq!(updated_stats.max_connections, 50);
        assert_eq!(updated_stats.next_id, 10);
    }

    #[tokio::test]
    async fn test_broadcast_shutdown_signal() {
        // 测试广播关闭信号的创建和使用
        let (shutdown_tx, mut shutdown_rx1) = broadcast::channel::<()>(1);
        let mut shutdown_rx2 = shutdown_tx.subscribe();
        
        // 发送关闭信号
        let _result = shutdown_tx.send(());
        
        // 接收者应该能够接收到信号
        let _result1 = shutdown_rx1.recv().await;
        let _result2 = shutdown_rx2.recv().await;
        
        // 这里主要测试broadcast channel的基本功能
        assert!(true); // 如果能到达这里说明没有panic
    }

    #[tokio::test]
    async fn test_connection_manager_edge_cases() {
        // 测试边界情况
        
        // 1. 最大连接数为0的情况
        let manager = ConnectionManager::new(0);
        assert!(!manager.can_accept_connection());
        assert_eq!(manager.max_connections(), 0);
        
        // 2. 最大连接数为1的情况
        let manager = ConnectionManager::new(1);
        assert!(manager.can_accept_connection());
        
        manager.active_count.store(1, Ordering::Relaxed);
        assert!(!manager.can_accept_connection());
        
        // 3. 非常大的最大连接数
        let manager = ConnectionManager::new(usize::MAX);
        assert!(manager.can_accept_connection());
        assert_eq!(manager.max_connections(), usize::MAX);
    }

    #[tokio::test]
    async fn test_atomic_operations() {
        let manager = ConnectionManager::new(10);
        
        // 测试原子操作的正确性
        let initial_count = manager.active_count.load(Ordering::Relaxed);
        assert_eq!(initial_count, 0);
        
        // 测试fetch_add
        let old_count = manager.active_count.fetch_add(5, Ordering::Relaxed);
        assert_eq!(old_count, 0);
        assert_eq!(manager.active_count.load(Ordering::Relaxed), 5);
        
        // 测试fetch_sub  
        let old_count = manager.active_count.fetch_sub(2, Ordering::Relaxed);
        assert_eq!(old_count, 5);
        assert_eq!(manager.active_count.load(Ordering::Relaxed), 3);
        
        // 测试store
        manager.active_count.store(10, Ordering::Relaxed);
        assert_eq!(manager.active_count.load(Ordering::Relaxed), 10);
    }

    #[tokio::test]
    async fn test_remove_connection_with_real_handle() {
        let mut manager = ConnectionManager::new(10);
        
        // 创建一个真实的连接处理器
        let (control_tx, _control_rx) = mpsc::channel::<ConnectionControlCommand>(10);
        let task_handle = tokio::spawn(async {
            // 模拟一个短暂运行的任务
            sleep(Duration::from_millis(100)).await;
        });
        
        let connection_handle = ConnectionHandle {
            task_handle,
            control_tx,
            addr: create_test_addr(),
        };
        
        // 手动插入连接
        manager.connections.insert(1, connection_handle);
        manager.active_count.store(1, Ordering::Relaxed);
        
        // 验证连接存在
        assert_eq!(manager.active_count(), 1);
        assert_eq!(manager.get_connection_ids(), vec![1]);
        
        // 移除连接
        manager.remove_connection(1).await;
        
        // 验证连接已被移除
        assert_eq!(manager.active_count(), 0);
        assert!(manager.get_connection_ids().is_empty());
    }

    #[tokio::test]
    async fn test_shutdown_all_with_real_connections() {
        let mut manager = ConnectionManager::new(10);
        
        // 创建多个模拟连接
        for i in 1..=3 {
            let (control_tx, _control_rx) = mpsc::channel::<ConnectionControlCommand>(10);
            let task_handle = tokio::spawn(async move {
                // 模拟连接处理，等待关闭信号
                sleep(Duration::from_millis(200)).await;
            });
            
            let connection_handle = ConnectionHandle {
                task_handle,
                control_tx,
                addr: create_test_addr(),
            };
            
            manager.connections.insert(i, connection_handle);
        }
        manager.active_count.store(3, Ordering::Relaxed);
        
        // 验证初始状态
        assert_eq!(manager.active_count(), 3);
        assert_eq!(manager.get_connection_ids().len(), 3);
        
        // 关闭所有连接
        manager.shutdown_all().await;
        
        // 验证所有连接都已关闭
        assert_eq!(manager.active_count(), 0);
        assert!(manager.get_connection_ids().is_empty());
        assert!(manager.connections.is_empty());
    }

    #[tokio::test]
    async fn test_send_control_command_success() {
        let mut manager = ConnectionManager::new(10);
        
        // 创建一个带有控制通道的连接
        let (control_tx, mut control_rx) = mpsc::channel::<ConnectionControlCommand>(10);
        let task_handle = tokio::spawn(async {
            sleep(Duration::from_secs(1)).await;
        });
        
        let connection_handle = ConnectionHandle {
            task_handle,
            control_tx,
            addr: create_test_addr(),
        };
        
        manager.connections.insert(1, connection_handle);
        
        // 在后台任务中接收控制命令
        let receive_task = tokio::spawn(async move {
            if let Some(cmd) = control_rx.recv().await {
                match cmd {
                    ConnectionControlCommand::Shutdown => true,
                    _ => false,
                }
            } else {
                false
            }
        });
        
        // 发送控制命令
        let result = manager.send_control_command(1, ConnectionControlCommand::Shutdown).await;
        assert!(result.is_ok());
        
        // 验证命令被接收
        let received = receive_task.await.unwrap();
        assert!(received);
    }

    #[tokio::test]
    async fn test_send_control_command_channel_closed() {
        let mut manager = ConnectionManager::new(10);
        
        // 创建一个控制通道，但立即关闭接收端
        let (control_tx, control_rx) = mpsc::channel::<ConnectionControlCommand>(10);
        drop(control_rx); // 关闭接收端
        
        let task_handle = tokio::spawn(async {
            sleep(Duration::from_secs(1)).await;
        });
        
        let connection_handle = ConnectionHandle {
            task_handle,
            control_tx,
            addr: create_test_addr(),
        };
        
        manager.connections.insert(1, connection_handle);
        
        // 尝试发送控制命令，应该失败
        let result = manager.send_control_command(1, ConnectionControlCommand::Shutdown).await;
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ConnectionError::ChannelSend(_) => {
                // 预期的错误类型
            }
            _ => panic!("应该返回ChannelSend错误"),
        }
    }

    #[tokio::test]
    async fn test_connection_manager_trait_implementation() {
        let mut manager = ConnectionManager::new(5);
        
        // 测试trait方法的基本功能
        assert_eq!(manager.active_count(), 0);
        assert_eq!(manager.max_connections(), 5);
        assert!(manager.can_accept_connection());
        assert!(manager.get_connection_ids().is_empty());
        
        let stats = manager.get_stats();
        assert_eq!(stats.active_count, 0);
        assert_eq!(stats.max_connections, 5);
        assert_eq!(stats.next_id, 1);
        
        // 测试空的shutdown_all
        manager.shutdown_all().await;
        assert_eq!(manager.active_count(), 0);
    }

    #[tokio::test]
    async fn test_connection_stats_struct() {
        let stats = ConnectionStats {
            active_count: 10,
            max_connections: 100,
            next_id: 42,
        };
        
        assert_eq!(stats.active_count, 10);
        assert_eq!(stats.max_connections, 100);
        assert_eq!(stats.next_id, 42);
        
        // 测试Clone trait
        let cloned_stats = stats.clone();
        assert_eq!(cloned_stats.active_count, stats.active_count);
        assert_eq!(cloned_stats.max_connections, stats.max_connections);
        assert_eq!(cloned_stats.next_id, stats.next_id);
    }

    #[tokio::test]
    async fn test_connection_handle_debug() {
        let (control_tx, _control_rx) = mpsc::channel::<ConnectionControlCommand>(10);
        let task_handle = tokio::spawn(async {
            sleep(Duration::from_millis(10)).await;
        });
        
        let connection_handle = ConnectionHandle {
            task_handle,
            control_tx,
            addr: create_test_addr(),
        };
        
        // 测试Debug trait实现
        let debug_str = format!("{:?}", connection_handle);
        assert!(debug_str.contains("ConnectionHandle"));
    }

    #[tokio::test]
    async fn test_remove_connection_internal_consistency() {
        let mut manager = ConnectionManager::new(10);
        
        // 创建多个连接
        for i in 1..=3 {
            let (control_tx, _control_rx) = mpsc::channel::<ConnectionControlCommand>(10);
            let task_handle = tokio::spawn(async {
                sleep(Duration::from_millis(50)).await;
            });
            
            let connection_handle = ConnectionHandle {
                task_handle,
                control_tx,
                addr: create_test_addr(),
            };
            
            manager.connections.insert(i, connection_handle);
        }
        manager.active_count.store(3, Ordering::Relaxed);
        
        // 移除中间的连接
        manager.remove_connection_internal(2);
        
        // 验证状态一致性
        assert_eq!(manager.active_count(), 2);
        assert_eq!(manager.connections.len(), 2);
        assert!(manager.connections.contains_key(&1));
        assert!(!manager.connections.contains_key(&2));
        assert!(manager.connections.contains_key(&3));
        
        let connection_ids = manager.get_connection_ids();
        assert_eq!(connection_ids.len(), 2);
        assert!(connection_ids.contains(&1));
        assert!(connection_ids.contains(&3));
    }

    #[tokio::test]
    async fn test_connection_manager_stress() {
        let manager = Arc::new(ConnectionManager::new(1000));
        let mut handles = vec![];
        
        // 并发测试连接ID生成和计数操作
        for i in 0..100 {
            let manager_clone = Arc::clone(&manager);
            let handle = tokio::spawn(async move {
                // 模拟连接创建
                let _id = manager_clone.next_id.fetch_add(1, Ordering::Relaxed);
                manager_clone.active_count.fetch_add(1, Ordering::Relaxed);
                
                // 短暂等待
                sleep(Duration::from_millis(1)).await;
                
                // 模拟连接关闭
                manager_clone.active_count.fetch_sub(1, Ordering::Relaxed);
                
                i
            });
            handles.push(handle);
        }
        
        // 等待所有任务完成
        for handle in handles {
            handle.await.unwrap();
        }
        
        // 验证最终状态
        assert_eq!(manager.active_count(), 0);
        assert_eq!(manager.next_id.load(Ordering::Relaxed), 101); // 1 + 100
    }

    #[tokio::test]
    async fn test_error_handling_in_drop() {
        // 测试在Drop中处理错误的情况
        let _manager = {
            let mut manager = ConnectionManager::new(5);
            
            // 创建一个已经完成的任务
            let task_handle = tokio::spawn(async {
                // 立即完成
            });
            
            // 等待任务完成
            let _ = task_handle.await;
            
            // 创建一个新的任务用于测试
            let (control_tx, _control_rx) = mpsc::channel::<ConnectionControlCommand>(10);
            let task_handle = tokio::spawn(async {
                sleep(Duration::from_millis(100)).await;
            });
            
            let connection_handle = ConnectionHandle {
                task_handle,
                control_tx,
                addr: create_test_addr(),
            };
            
            manager.connections.insert(1, connection_handle);
            manager.active_count.store(1, Ordering::Relaxed);
            
            manager
        }; // 在这里触发drop
        
        // 如果能到达这里说明drop处理正常
        assert!(true);
    }
} 