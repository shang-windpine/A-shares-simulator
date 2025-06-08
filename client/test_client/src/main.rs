use trade_protocal_lite::{
    ProtoMessage, ProtoBody, LoginRequest, LoginResponse, NewOrderRequest, 
    CancelOrderRequest, MarketDataRequest, MarketDataSnapshot, Heartbeat, 
    HeartbeatResponse, WireMessage, OrderSide, OrderType, ErrorResponse,
    market_data_request::SubscriptionAction
};
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{sleep, Duration, timeout, interval};
use bytes::{Bytes, BytesMut, BufMut, Buf};
use tracing::{info, warn, error, debug};
use anyhow::{Result, anyhow};
use uuid::Uuid;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::Mutex;

/// 测试客户端配置
#[derive(Debug, Clone)]
struct TestClientConfig {
    /// 服务器地址
    server_addr: String,
    /// 用户ID
    user_id: String,
    /// 心跳间隔（秒）
    heartbeat_interval_secs: u64,
    /// 消息读取超时（秒）
    message_timeout_secs: u64,
}

impl Default for TestClientConfig {
    fn default() -> Self {
        Self {
            server_addr: "127.0.0.1:8080".to_string(),
            user_id: "test_user_001".to_string(),
            heartbeat_interval_secs: 20, // 小于服务端30秒超时
            message_timeout_secs: 10,
        }
    }
}

/// 测试客户端
struct TestClient {
    config: TestClientConfig,
    stream: Arc<Mutex<TcpStream>>,
    session_id: Option<String>,
}

impl TestClient {
    /// 创建新的测试客户端
    async fn new(config: TestClientConfig) -> Result<Self> {
        info!("正在连接服务器: {}", config.server_addr);
        let stream = TcpStream::connect(&config.server_addr).await?;
        info!("成功连接到服务器");
        
        Ok(Self {
            config,
            stream: Arc::new(Mutex::new(stream)),
            session_id: None,
        })
    }

    /// 运行完整的测试流程
    async fn run(&mut self) -> Result<()> {
        info!("开始运行测试客户端流程");

        // 1. 登录
        self.login().await?;
        
        // 2. 启动心跳任务
        let heartbeat_handle = self.start_heartbeat().await;

        // 3. 查询行情数据
        let market_data = self.query_market_data().await?;
        
        // 4. 根据行情数据下单
        let order_id = self.submit_order(&market_data).await?;
        
        // 5. 等待一段时间，然后尝试撤单
        sleep(Duration::from_secs(2)).await;
        self.cancel_order(&order_id).await?;

        // 6. 再等待一段时间观察
        sleep(Duration::from_secs(5)).await;

        // 停止心跳任务
        heartbeat_handle.abort();
        
        info!("测试流程完成");
        Ok(())
    }

    /// 登录流程
    async fn login(&mut self) -> Result<()> {
        info!("开始登录流程");
        
        let login_request = LoginRequest {
            user_id: self.config.user_id.clone(),
            password: "test_password".to_string(),
        };
        
        let request_msg = ProtoMessage::new(ProtoBody::LoginRequest(login_request));
        self.send_message(request_msg).await?;
        
        // 等待登录响应
        let response = self.receive_message().await?;
        match response.body {
            ProtoBody::LoginResponse(login_resp) => {
                if login_resp.success {
                    self.session_id = Some(login_resp.session_id.clone());
                    info!("登录成功，会话ID: {}", login_resp.session_id);
                } else {
                    return Err(anyhow!("登录失败: {}", login_resp.message));
                }
            }
            ProtoBody::ErrorResponse(err) => {
                return Err(anyhow!("登录错误: {}", err.error_message));
            }
            _ => {
                return Err(anyhow!("收到意外的登录响应类型"));
            }
        }
        
        Ok(())
    }

    /// 启动心跳任务
    async fn start_heartbeat(&self) -> tokio::task::JoinHandle<()> {
        let stream = Arc::clone(&self.stream);
        let interval_secs = self.config.heartbeat_interval_secs;
        
        tokio::spawn(async move {
            let mut heartbeat_interval = interval(Duration::from_secs(interval_secs));
            
            loop {
                heartbeat_interval.tick().await;
                
                let heartbeat = Heartbeat {
                    client_timestamp_utc: Utc::now().to_rfc3339(),
                };
                
                let heartbeat_msg = ProtoMessage::new(ProtoBody::Heartbeat(heartbeat));
                
                if let Err(e) = Self::send_message_static(&stream, heartbeat_msg).await {
                    error!("发送心跳失败: {}", e);
                    break;
                }
                
                debug!("发送心跳成功");
            }
        })
    }

    /// 查询行情数据
    async fn query_market_data(&self) -> Result<MarketDataSnapshot> {
        info!("查询 000001.SZ 的行情数据");
        
        let market_request = MarketDataRequest {
            action: SubscriptionAction::Subscribe as i32,
            stock_codes: vec!["000001.SZ".to_string()],
        };
        
        let request_msg = ProtoMessage::new(ProtoBody::MarketDataRequest(market_request));
        self.send_message(request_msg).await?;
        
        // 等待行情响应
        let response = self.receive_message().await?;
        match response.body {
            ProtoBody::MarketDataSnapshot(snapshot) => {
                info!("收到行情数据: 股票={}, 最新价={}, 前收盘价={}", 
                      snapshot.stock_code, snapshot.last_price, snapshot.prev_close_price);
                Ok(snapshot)
            }
            ProtoBody::ErrorResponse(err) => {
                Err(anyhow!("查询行情失败: {}", err.error_message))
            }
            _ => {
                Err(anyhow!("收到意外的行情响应类型"))
            }
        }
    }

    /// 提交订单
    async fn submit_order(&self, market_data: &MarketDataSnapshot) -> Result<String> {
        info!("根据行情数据提交订单");
        
        // 使用前收盘价作为基准价格，稍微调整作为限价
        let base_price = if market_data.prev_close_price > 0.0 {
            market_data.prev_close_price
        } else if market_data.last_price > 0.0 {
            market_data.last_price
        } else {
            10.0 // 默认价格
        };
        
        // 生成客户端订单ID
        let client_order_id = format!("CLIENT_ORDER_{}", Uuid::new_v4().simple());
        
        // 提交一个买单，价格稍高于基准价格以便可能成交
        let order_price = (base_price * 1.01 * 100.0).round() / 100.0; // 高1%，保留两位小数
        
        let new_order = NewOrderRequest {
            account_id: self.config.user_id.clone(),
            client_order_id: client_order_id.clone(),
            stock_code: "000001.SZ".to_string(),
            side: OrderSide::Buy as i32,
            r#type: OrderType::Limit as i32,
            quantity: 100, // 100股
            price: order_price,
        };
        
        info!("提交买单: 股票={}, 数量={}, 价格={:.2}", 
              new_order.stock_code, new_order.quantity, new_order.price);
        
        let request_msg = ProtoMessage::new(ProtoBody::NewOrderRequest(new_order));
        self.send_message(request_msg).await?;
        
        // 等待订单响应
        let response = self.receive_message().await?;

        // ############ DEBUGGING LOG ############
        info!("在 cancel_order 中收到的响应: {:?}", response);
        // ############ END DEBUGGING LOG ############
        
        match response.body {
            ProtoBody::OrderUpdateResponse(order_resp) => {
                info!("订单提交成功: 服务器订单ID={}, 状态={:?}", 
                      order_resp.server_order_id, order_resp.status);
                Ok(order_resp.server_order_id)
            }
            ProtoBody::ErrorResponse(err) => {
                Err(anyhow!("订单提交失败: {}", err.error_message))
            }
            _ => {
                Err(anyhow!("收到意外的订单响应类型"))
            }
        }
    }

    /// 撤销订单
    async fn cancel_order(&self, server_order_id: &str) -> Result<()> {
        info!("尝试撤销订单: {}", server_order_id);
        
        let cancel_request = CancelOrderRequest {
            account_id: self.config.user_id.clone(),
            server_order_id_to_cancel: server_order_id.to_string(),
        };
        
        let request_msg = ProtoMessage::new(ProtoBody::CancelOrderRequest(cancel_request));
        self.send_message(request_msg).await?;
        
        // 等待撤单响应
        let response = self.receive_message().await?;
        match response.body {
            ProtoBody::OrderUpdateResponse(order_resp) => {
                info!("撤单响应: 订单ID={}, 状态={:?}, 消息={}", 
                      order_resp.server_order_id, order_resp.status, order_resp.reject_message);
                Ok(())
            }
            ProtoBody::ErrorResponse(err) => {
                warn!("撤单失败: {}", err.error_message);
                Ok(()) // 撤单失败是正常的，比如订单已经成交
            }
            _ => {
                Err(anyhow!("收到意外的撤单响应类型"))
            }
        }
    }

    /// 发送消息
    async fn send_message(&self, message: ProtoMessage) -> Result<()> {
        Self::send_message_static(&self.stream, message).await
    }

    /// 静态方法：发送消息（用于心跳任务）
    async fn send_message_static(stream: &Arc<Mutex<TcpStream>>, message: ProtoMessage) -> Result<()> {
        let wire_message: WireMessage = message.try_into()?;
        let encoded = wire_message.encode_to_bytes();
        
        let mut stream_guard = stream.lock().await;
        stream_guard.write_all(&encoded).await?;
        stream_guard.flush().await?;
        
        Ok(())
    }

    /// 接收消息
    async fn receive_message(&self) -> Result<ProtoMessage> {
        let timeout_duration = Duration::from_secs(self.config.message_timeout_secs);
        
        timeout(timeout_duration, async {
            loop {
                let message = self.receive_message_internal().await?;
                // 检查是否为心跳响应
                if let ProtoBody::HeartbeatResponse(_) = &message.body {
                    debug!("收到并忽略心跳响应，继续等待业务响应");
                    continue; // 忽略并继续循环
                }
                // 如果不是心跳响应，则认为是期望的业务响应
                return Ok(message);
            }
        }).await.map_err(|_| anyhow!("接收业务消息超时"))?
    }

    /// 内部接收消息逻辑
    async fn receive_message_internal(&self) -> Result<ProtoMessage> {
        let mut stream_guard = self.stream.lock().await;
        
        // 读取消息头部 (10字节)
        let mut header_buf = [0u8; 10];
        stream_guard.read_exact(&mut header_buf).await?;
        
        // 解析头部
        let mut header_bytes = Bytes::copy_from_slice(&header_buf);
        let total_length = header_bytes.get_u32();
        let msg_type_raw = header_bytes.get_u16();
        let proto_body_length = header_bytes.get_u32();
        
        // 验证长度
        if proto_body_length as usize > total_length as usize - 10 {
            return Err(anyhow!("消息长度不匹配"));
        }
        
        // 读取消息体
        let mut body_buf = vec![0u8; proto_body_length as usize];
        stream_guard.read_exact(&mut body_buf).await?;
        
        // 构造WireMessage并转换为TradeMessage
        let wire_message = WireMessage {
            total_length,
            msg_type: msg_type_raw.try_into()?,
            proto_body_length,
            proto_body: Bytes::from(body_buf),
        };
        
        let trade_message: ProtoMessage = wire_message.try_into()?;
        Ok(trade_message)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter("debug")
        .init();

    info!("启动A股交易系统测试客户端");

    // 创建并运行测试客户端
    let config = TestClientConfig::default();
    let mut client = TestClient::new(config).await?;
    
    if let Err(e) = client.run().await {
        error!("测试客户端运行出错: {}", e);
        return Err(e);
    }

    info!("测试客户端正常结束");
    Ok(())
}