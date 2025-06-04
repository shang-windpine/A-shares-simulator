/// 传输层抽象模块
/// 
/// 支持多种传输协议：
/// 1. 原生TCP + Protobuf（低延迟）
/// 2. HTTP/2 gRPC（标准化，运维友好）
/// 3. WebSocket（浏览器支持）

// TODO: 后续实现具体的传输层
// pub mod tcp;
// pub mod grpc;
// pub mod websocket;

use async_trait::async_trait;
use bytes::Bytes;
use std::net::SocketAddr;
use tokio::sync::mpsc;

/// 传输协议类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransportType {
    /// 原生TCP（最低延迟）
    NativeTcp,
    /// HTTP/2 gRPC（标准化）
    Grpc,
    /// WebSocket（浏览器兼容）
    WebSocket,
}

/// 连接事件
#[derive(Debug)]
pub enum ConnectionEvent {
    /// 新连接建立
    Connected { addr: SocketAddr, transport: TransportType },
    /// 连接断开
    Disconnected { addr: SocketAddr, reason: String },
    /// 接收到消息
    MessageReceived { addr: SocketAddr, data: Bytes },
}

/// 传输层trait，定义统一的传输接口
#[async_trait]
pub trait Transport: Send + Sync {
    /// 启动传输层服务
    async fn start(&mut self, bind_addr: SocketAddr) -> Result<(), TransportError>;
    
    /// 停止传输层服务
    async fn stop(&mut self) -> Result<(), TransportError>;
    
    /// 发送消息到指定地址
    async fn send_to(&self, addr: SocketAddr, data: Bytes) -> Result<(), TransportError>;
    
    /// 广播消息到所有连接
    async fn broadcast(&self, data: Bytes) -> Result<(), TransportError>;
    
    /// 获取连接事件接收器
    fn event_receiver(&self) -> mpsc::Receiver<ConnectionEvent>;
    
    /// 获取传输类型
    fn transport_type(&self) -> TransportType;
}

/// 传输层错误
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("绑定地址失败: {0}")]
    BindFailed(std::io::Error),
    
    #[error("发送消息失败: {0}")]
    SendFailed(String),
    
    #[error("连接不存在: {addr}")]
    ConnectionNotFound { addr: SocketAddr },
    
    #[error("传输层未启动")]
    NotStarted,
    
    #[error("协议错误: {0}")]
    ProtocolError(String),
}

/// 多传输层管理器
pub struct MultiTransportManager {
    transports: Vec<Box<dyn Transport>>,
    event_tx: mpsc::Sender<ConnectionEvent>,
    event_rx: mpsc::Receiver<ConnectionEvent>,
}

impl MultiTransportManager {
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel(1000);
        Self {
            transports: Vec::new(),
            event_tx,
            event_rx,
        }
    }
    
    /// 添加传输层
    pub fn add_transport(&mut self, transport: Box<dyn Transport>) {
        self.transports.push(transport);
    }
    
    /// 启动所有传输层
    pub async fn start_all(&mut self, base_port: u16) -> Result<(), TransportError> {
        for (i, transport) in self.transports.iter_mut().enumerate() {
            let addr = SocketAddr::from(([127, 0, 0, 1], base_port + i as u16));
            transport.start(addr).await?;
        }
        Ok(())
    }
    
    /// 根据延迟要求选择最佳传输方式
    pub fn select_transport_for_latency(&self, max_latency_ms: u32) -> Option<TransportType> {
        match max_latency_ms {
            0..=1 => Some(TransportType::NativeTcp),    // 超低延迟
            2..=10 => Some(TransportType::Grpc),        // 低延迟 + 标准化
            _ => Some(TransportType::WebSocket),         // 兼容性优先
        }
    }
} 