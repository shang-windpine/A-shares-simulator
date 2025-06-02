use std::time::{Duration, Instant};
use std::sync::Arc;
use tokio::io::{AsyncWriteExt, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, info, warn, instrument};
use trade_protocal_lite::{TradeMessage, WireMessage};
use order_engine::OrderEngine;

use crate::error::{ConnectionError};
use crate::frame_decoder::FrameDecoder;
use crate::message_dispatcher::MessageDispatcher;
use crate::connection_manager::{ConnectionId, ConnectionControlCommand};

/// 心跳超时时间（秒）
const HEARTBEAT_TIMEOUT_SECS: u64 = 30;
/// 心跳检查间隔（秒）
const HEARTBEAT_CHECK_INTERVAL_SECS: u64 = 10;

/// 客户端连接处理器，管理单个客户端连接的完整生命周期
pub struct ClientConnection {
    /// 连接唯一标识
    id: ConnectionId,
    /// 帧解码器，用于从TCP流读取数据
    frame_decoder: FrameDecoder,
    /// TCP写半部分，用于发送数据
    writer: WriteHalf<TcpStream>,
    /// 消息分发器
    message_dispatcher: MessageDispatcher,
    /// 关闭信号接收器
    shutdown_rx: broadcast::Receiver<()>,
    /// 控制命令接收器（可选）
    control_rx: Option<mpsc::Receiver<ConnectionControlCommand>>,
    /// 最后心跳时间
    last_heartbeat: Instant,
}

impl ClientConnection {
    /// 创建新的客户端连接
    #[instrument(level = "debug", skip(stream, shutdown_rx))]
    pub async fn new(
        stream: TcpStream, 
        id: ConnectionId, 
        shutdown_rx: broadcast::Receiver<()>
    ) -> Result<Self, ConnectionError> {
        info!("创建客户端连接: ID={}", id);

        let (reader, writer) = tokio::io::split(stream);
        let frame_decoder = FrameDecoder::new(reader, None);
        let message_dispatcher = MessageDispatcher::new_without_services(id);

        Ok(Self {
            id,
            frame_decoder,
            writer,
            message_dispatcher,
            shutdown_rx,
            control_rx: None,
            last_heartbeat: Instant::now(),
        })
    }

    /// 创建新的客户端连接（带控制通道）
    #[instrument(level = "debug", skip(stream, shutdown_rx, control_rx))]
    pub async fn new_with_control(
        stream: TcpStream, 
        id: ConnectionId, 
        shutdown_rx: broadcast::Receiver<()>,
        control_rx: mpsc::Receiver<ConnectionControlCommand>,
    ) -> Result<Self, ConnectionError> {
        info!("创建客户端连接（带控制通道）: ID={}", id);

        let (reader, writer) = tokio::io::split(stream);
        let frame_decoder = FrameDecoder::new(reader, None);
        let message_dispatcher = MessageDispatcher::new_without_services(id);

        Ok(Self {
            id,
            frame_decoder,
            writer,
            message_dispatcher,
            shutdown_rx,
            control_rx: Some(control_rx),
            last_heartbeat: Instant::now(),
        })
    }

    /// 创建新的客户端连接（带控制通道和业务服务依赖）
    #[instrument(level = "debug", skip(stream, shutdown_rx, control_rx, order_engine))]
    pub async fn new_with_control_and_services(
        stream: TcpStream, 
        id: ConnectionId, 
        shutdown_rx: broadcast::Receiver<()>,
        control_rx: mpsc::Receiver<ConnectionControlCommand>,
        order_engine: Option<Arc<OrderEngine>>,
    ) -> Result<Self, ConnectionError> {
        info!("创建客户端连接（带控制通道和服务）: ID={}", id);

        let (reader, writer) = tokio::io::split(stream);
        let frame_decoder = FrameDecoder::new(reader, None);
        
        let message_dispatcher = match order_engine {
            Some(engine) => MessageDispatcher::new(id, engine),
            None => MessageDispatcher::new_without_services(id),
        };

        Ok(Self {
            id,
            frame_decoder,
            writer,
            message_dispatcher,
            shutdown_rx,
            control_rx: Some(control_rx),
            last_heartbeat: Instant::now(),
        })
    }

    /// 主处理循环
    #[instrument(level = "debug", skip(self))]
    pub async fn run(mut self) -> Result<(), ConnectionError> {
        info!("启动客户端连接处理循环: ID={}", self.id);

        // 创建心跳检查任务
        let mut heartbeat_check_interval = tokio::time::interval(
            Duration::from_secs(HEARTBEAT_CHECK_INTERVAL_SECS)
        );

        loop {
            // 根据是否有控制通道来选择不同的分支
            if let Some(ref mut control_rx) = self.control_rx {
                tokio::select! {
                    // 处理来自客户端的消息
                    frame_result = self.frame_decoder.read_frame() => {
                        match frame_result {
                            Ok(Some(wire_msg)) => {
                                if let Err(e) = self.handle_wire_message(wire_msg).await {
                                    error!("连接 {} 处理消息时出错: {}", self.id, e);
                                    return Err(e);
                                }
                            }
                            Ok(None) => {
                                // 暂时没有完整的消息，继续等待
                                continue;
                            }
                            Err(e) => {
                                warn!("连接 {} 读取帧时出错: {}", self.id, e);
                                return Err(e);
                            }
                        }
                    }

                    // 心跳超时检查
                    _ = heartbeat_check_interval.tick() => {
                        if self.is_heartbeat_timeout() {
                            warn!("连接 {} 心跳超时", self.id);
                            return Err(ConnectionError::HeartbeatTimeout { 
                                connection_id: self.id 
                            });
                        }
                    }

                    // 处理控制命令
                    control_command = control_rx.recv() => {
                        match control_command {
                            Some(cmd) => {
                                if let Err(e) = self.handle_control_command(cmd).await {
                                    error!("连接 {} 处理控制命令时出错: {}", self.id, e);
                                    return Err(e);
                                }
                            }
                            None => {
                                info!("连接 {} 控制通道已关闭", self.id);
                                return Err(ConnectionError::ShutdownRequested);
                            }
                        }
                    }

                    // 接收关闭信号
                    _ = self.shutdown_rx.recv() => {
                        info!("连接 {} 收到关闭信号", self.id);
                        return Err(ConnectionError::ShutdownRequested);
                    }
                }
            } else {
                // 没有控制通道的情况（原有逻辑）
                tokio::select! {
                    // 处理来自客户端的消息
                    frame_result = self.frame_decoder.read_frame() => {
                        match frame_result {
                            Ok(Some(wire_msg)) => {
                                if let Err(e) = self.handle_wire_message(wire_msg).await {
                                    error!("连接 {} 处理消息时出错: {}", self.id, e);
                                    return Err(e);
                                }
                            }
                            Ok(None) => {
                                // 暂时没有完整的消息，继续等待
                                continue;
                            }
                            Err(e) => {
                                warn!("连接 {} 读取帧时出错: {}", self.id, e);
                                return Err(e);
                            }
                        }
                    }

                    // 心跳超时检查
                    _ = heartbeat_check_interval.tick() => {
                        if self.is_heartbeat_timeout() {
                            warn!("连接 {} 心跳超时", self.id);
                            return Err(ConnectionError::HeartbeatTimeout { 
                                connection_id: self.id 
                            });
                        }
                    }

                    // 接收关闭信号
                    _ = self.shutdown_rx.recv() => {
                        info!("连接 {} 收到关闭信号", self.id);
                        return Err(ConnectionError::ShutdownRequested);
                    }
                }
            }
        }
    }

    /// 处理控制命令
    #[instrument(level = "debug", skip(self))]
    async fn handle_control_command(&mut self, command: ConnectionControlCommand) -> Result<(), ConnectionError> {
        debug!("连接 {} 处理控制命令: {:?}", self.id, command);

        match command {
            ConnectionControlCommand::Shutdown => {
                info!("连接 {} 收到关闭控制命令", self.id);
                return Err(ConnectionError::ShutdownRequested);
            }
            ConnectionControlCommand::SendMessage(data) => {
                debug!("连接 {} 收到发送消息控制命令: {} 字节", self.id, data.len());
                match self.writer.write_all(&data).await {
                    Ok(_) => {
                        if let Err(e) = self.writer.flush().await {
                            error!("连接 {} 刷新写缓冲区失败: {}", self.id, e);
                            return Err(ConnectionError::Io(e));
                        }
                        debug!("连接 {} 成功发送控制消息: {} 字节", self.id, data.len());
                    }
                    Err(e) => {
                        error!("连接 {} 发送控制消息失败: {}", self.id, e);
                        return Err(ConnectionError::Io(e));
                    }
                }
            }
            ConnectionControlCommand::GetStatus => {
                debug!("连接 {} 收到状态查询控制命令", self.id);
                // 这里可以扩展返回状态信息给管理器
                // 目前只是记录日志
            }
        }

        Ok(())
    }

    /// 处理WireMessage
    #[instrument(level = "debug", skip(self, wire_msg))]
    async fn handle_wire_message(&mut self, wire_msg: WireMessage) -> Result<(), ConnectionError> {
        debug!("连接 {} 处理Wire消息: type={:?}", self.id, wire_msg.msg_type);

        // 将WireMessage转换为TradeMessage
        let trade_msg: TradeMessage = wire_msg.try_into()?;

        // 检查是否为心跳消息，如果是则更新心跳时间
        if matches!(trade_msg.body, trade_protocal_lite::ProtoBody::Heartbeat(_)) {
            self.last_heartbeat = Instant::now();
            debug!("连接 {} 更新心跳时间", self.id);
        }

        // 通过消息分发器处理消息
        let response = self.message_dispatcher.dispatch(trade_msg).await?;

        // 如果有响应，发送给客户端
        if let Some(response_msg) = response {
            self.send_response(response_msg).await?;
        }

        Ok(())
    }

    /// 发送响应消息给客户端
    #[instrument(level = "debug", skip(self, response))]
    async fn send_response(&mut self, response: TradeMessage) -> Result<(), ConnectionError> {
        debug!("连接 {} 发送响应: type={:?}", self.id, response.message_type());

        // 将TradeMessage转换为WireMessage
        let wire_msg: WireMessage = response.try_into()?;
        
        // 编码为字节并发送
        let data = wire_msg.encode_to_bytes();
        
        match self.writer.write_all(&data).await {
            Ok(_) => {
                if let Err(e) = self.writer.flush().await {
                    error!("连接 {} 刷新写缓冲区失败: {}", self.id, e);
                    return Err(ConnectionError::Io(e));
                }
                debug!("连接 {} 成功发送 {} 字节", self.id, data.len());
                Ok(())
            }
            Err(e) => {
                error!("连接 {} 发送数据失败: {}", self.id, e);
                Err(ConnectionError::Io(e))
            }
        }
    }

    /// 检查心跳是否超时
    fn is_heartbeat_timeout(&self) -> bool {
        let elapsed = self.last_heartbeat.elapsed();
        let timeout = Duration::from_secs(HEARTBEAT_TIMEOUT_SECS);
        
        if elapsed > timeout {
            warn!("连接 {} 心跳超时: 上次心跳 {:.2} 秒前", 
                 self.id, elapsed.as_secs_f64());
            true
        } else {
            false
        }
    }

    /// 获取连接ID
    pub fn connection_id(&self) -> ConnectionId {
        self.id
    }

    /// 获取连接统计信息
    pub fn get_connection_info(&self) -> ConnectionInfo {
        ConnectionInfo {
            id: self.id,
            last_heartbeat: self.last_heartbeat,
            heartbeat_timeout: Duration::from_secs(HEARTBEAT_TIMEOUT_SECS),
        }
    }
}

impl Drop for ClientConnection {
    fn drop(&mut self) {
        info!("客户端连接被销毁: ID={}", self.id);
    }
}

/// 连接信息结构
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// 连接ID
    pub id: ConnectionId,
    /// 最后心跳时间
    pub last_heartbeat: Instant,
    /// 心跳超时时间
    pub heartbeat_timeout: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::broadcast;
    use tokio::time::{sleep, Duration};

    async fn create_test_connection() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        
        let client_stream = TcpStream::connect(addr);
        let server_stream = listener.accept();
        
        let (client_result, server_result) = tokio::join!(client_stream, server_stream);
        let client = client_result.unwrap();
        let (server, _) = server_result.unwrap();
        (client, server)
    }

    #[tokio::test]
    async fn test_client_connection_creation() {
        let (_client, server) = create_test_connection().await;
        let (_, shutdown_rx) = broadcast::channel(1);
        
        let connection = ClientConnection::new(server, 1, shutdown_rx).await;
        assert!(connection.is_ok());
        
        let conn = connection.unwrap();
        assert_eq!(conn.connection_id(), 1);
    }

    #[tokio::test]
    async fn test_heartbeat_timeout_check() {
        let (_client, server) = create_test_connection().await;
        let (_, shutdown_rx) = broadcast::channel(1);
        
        let mut connection = ClientConnection::new(server, 1, shutdown_rx).await.unwrap();
        
        // 初始状态不应该超时
        assert!(!connection.is_heartbeat_timeout());
        
        // 手动设置旧的心跳时间
        connection.last_heartbeat = Instant::now() - Duration::from_secs(HEARTBEAT_TIMEOUT_SECS + 1);
        
        // 现在应该超时
        assert!(connection.is_heartbeat_timeout());
    }

    #[tokio::test]
    async fn test_connection_info() {
        let (_client, server) = create_test_connection().await;
        let (_, shutdown_rx) = broadcast::channel(1);
        
        let connection = ClientConnection::new(server, 42, shutdown_rx).await.unwrap();
        let info = connection.get_connection_info();
        
        assert_eq!(info.id, 42);
        assert_eq!(info.heartbeat_timeout, Duration::from_secs(HEARTBEAT_TIMEOUT_SECS));
    }

    #[tokio::test]
    async fn test_shutdown_signal() {
        let (_client, server) = create_test_connection().await;
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        
        let connection = ClientConnection::new(server, 1, shutdown_rx).await.unwrap();
        
        // 在另一个任务中运行连接处理
        let connection_handle = tokio::spawn(async move {
            connection.run().await
        });
        
        // 稍等一下然后发送关闭信号
        sleep(Duration::from_millis(10)).await;
        let _ = shutdown_tx.send(());
        
        // 连接应该因为关闭信号而结束
        let result = connection_handle.await.unwrap();
        assert!(matches!(result, Err(ConnectionError::ShutdownRequested)));
    }

    #[tokio::test]
    async fn test_control_command_shutdown() {
        let (_client, server) = create_test_connection().await;
        let (_, shutdown_rx) = broadcast::channel(1);
        let (control_tx, control_rx) = mpsc::channel(10);
        
        let connection = ClientConnection::new_with_control(server, 1, shutdown_rx, control_rx).await.unwrap();
        
        // 在另一个任务中运行连接处理
        let connection_handle = tokio::spawn(async move {
            connection.run().await
        });
        
        // 稍等一下然后发送控制命令
        sleep(Duration::from_millis(10)).await;
        let _ = control_tx.send(ConnectionControlCommand::Shutdown).await;
        
        // 连接应该因为控制命令而结束
        let result = connection_handle.await.unwrap();
        assert!(matches!(result, Err(ConnectionError::ShutdownRequested)));
    }
} 