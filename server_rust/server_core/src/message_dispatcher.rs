use std::sync::Arc;
use std::str::FromStr;
use tracing::{debug, info, warn};
use trade_protocal_lite::{TradeMessage, ProtoBody, protocol::*};
use order_engine::{OrderEngine, Order, OrderSide as EngineOrderSide, OrderEngineFactory, OrderEngineConfig};
use market_data_engine::{MarketDataService, MarketData};
use rust_decimal::Decimal;

use crate::error::{ConnectionError};
use crate::connection_manager::ConnectionId;

/// 消息分发器，负责根据消息类型将请求路由到相应的业务处理器
pub struct MessageDispatcher {
    /// 关联的连接ID
    connection_id: ConnectionId,
    /// 订单引擎引用
    order_engine: Arc<OrderEngine>,
    /// 市场数据服务引用
    market_data_service: Option<Arc<dyn MarketDataService>>,
}

impl MessageDispatcher {
    /// 创建新的消息分发器（带有订单引擎依赖）
    pub fn new(connection_id: ConnectionId, order_engine: Arc<OrderEngine>) -> Self {
        debug!("为连接 {} 创建消息分发器（带订单引擎）", connection_id);
        Self { 
            connection_id,
            order_engine,
            market_data_service: None,
        }
    }

    /// 创建新的消息分发器（带有完整的服务依赖）
    pub fn new_with_services(
        connection_id: ConnectionId, 
        order_engine: Arc<OrderEngine>,
        market_data_service: Arc<dyn MarketDataService>,
    ) -> Self {
        debug!("为连接 {} 创建消息分发器（带完整服务）", connection_id);
        Self { 
            connection_id,
            order_engine,
            market_data_service: Some(market_data_service),
        }
    }

    /// 创建新的消息分发器（无业务服务依赖，用于向后兼容）
    pub fn new_without_services(connection_id: ConnectionId) -> Self {
        debug!("为连接 {} 创建消息分发器（无业务服务）", connection_id);
        // 创建一个默认的 OrderEngine 实例用于向后兼容
        let (order_engine, _order_rx, _match_tx) = 
            OrderEngineFactory::create_with_channels(None, OrderEngineConfig::default());
        Self { 
            connection_id,
            order_engine: Arc::new(order_engine),
            market_data_service: None,
        }
    }

    /// 将 MarketData 转换为 protobuf MarketDataSnapshot
    fn convert_market_data_to_protobuf(&self, market_data: &MarketData) -> MarketDataSnapshot {
        let static_data = &market_data.static_data;
        let dynamic_data = &market_data.dynamic_data;

        MarketDataSnapshot {
            stock_code: static_data.stock_id.to_string(),
            last_price: dynamic_data.current_price.try_into().unwrap_or(0.0),
            open_price: static_data.open_price.try_into().unwrap_or(0.0),
            high_price: dynamic_data.high_price.try_into().unwrap_or(0.0),
            low_price: if dynamic_data.low_price == Decimal::MAX {
                0.0 // 如果还没有交易，最低价设为0
            } else {
                dynamic_data.low_price.try_into().unwrap_or(0.0)
            },
            prev_close_price: static_data.prev_close_price.try_into().unwrap_or(0.0),
            total_volume: dynamic_data.volume as i64,
            total_turnover: dynamic_data.turnover.try_into().unwrap_or(0.0),
            server_timestamp_utc: dynamic_data.last_updated.to_rfc3339(),
            
            // 买卖盘数据 - 暂时使用基于当前价格的估算值
            // 在未来版本中，这些数据会来自订单簿深度
            bid_price_1: if dynamic_data.current_price > Decimal::ZERO {
                (dynamic_data.current_price * Decimal::from_str("0.999").unwrap_or(Decimal::ONE))
                    .try_into().unwrap_or(0.0)
            } else {
                0.0
            },
            bid_volume_1: 1000, // 模拟买一量
            ask_price_1: if dynamic_data.current_price > Decimal::ZERO {
                (dynamic_data.current_price * Decimal::from_str("1.001").unwrap_or(Decimal::ONE))
                    .try_into().unwrap_or(0.0)
            } else {
                0.0
            },
            ask_volume_1: 1000, // 模拟卖一量
        }
    }

    /// 分发消息到相应的业务处理器
    pub async fn dispatch(&self, message: TradeMessage) -> Result<Option<TradeMessage>, ConnectionError> {
        debug!("连接 {} 开始分发消息: {:?}", self.connection_id, message);

        let response_body = match message.body {
            ProtoBody::Heartbeat(heartbeat) => {
                debug!("连接 {} 处理心跳消息", self.connection_id);
                Some(self.handle_heartbeat(heartbeat).await?)
            }

            ProtoBody::LoginRequest(req) => {
                info!("连接 {} 处理登录请求", self.connection_id);
                Some(self.handle_login_request(req).await?)
            }

            ProtoBody::NewOrderRequest(req) => {
                info!("连接 {} 处理新订单请求", self.connection_id);
                Some(self.handle_new_order_request(req).await?)
            }

            ProtoBody::CancelOrderRequest(req) => {
                info!("连接 {} 处理撤单请求", self.connection_id);
                Some(self.handle_cancel_order_request(req).await?)
            }

            ProtoBody::MarketDataRequest(req) => {
                info!("连接 {} 处理市场数据请求", self.connection_id);
                Some(self.handle_market_data_request(req).await?)
            }

            ProtoBody::AccountQueryRequest(req) => {
                info!("连接 {} 处理账户查询请求", self.connection_id);
                Some(self.handle_account_query_request(req).await?)
            }

            // 响应消息通常不需要进一步处理
            ProtoBody::LoginResponse(_) |
            ProtoBody::OrderUpdateResponse(_) |
            ProtoBody::MarketDataSnapshot(_) |
            ProtoBody::AccountInfoResponse(_) |
            ProtoBody::HeartbeatResponse(_) |
            ProtoBody::ErrorResponse(_) => {
                warn!("连接 {} 收到响应消息，可能是协议错误", self.connection_id);
                None
            }
        };

        let response = response_body.map(|body| TradeMessage::new(body));

        if response.is_some() {
            debug!("连接 {} 消息分发完成，有响应", self.connection_id);
        } else {
            debug!("连接 {} 消息分发完成，无响应", self.connection_id);
        }

        Ok(response)
    }

    /// 处理心跳消息
    async fn handle_heartbeat(&self, heartbeat: Heartbeat) -> Result<ProtoBody, ConnectionError> {
        debug!("连接 {} 处理心跳: timestamp={}", self.connection_id, heartbeat.client_timestamp_utc);
        
        // 心跳响应：返回服务器时间戳
        let response = HeartbeatResponse {
            server_timestamp_utc: chrono::Utc::now().to_rfc3339(),
        };

        Ok(ProtoBody::HeartbeatResponse(response))
    }

    /// 处理登录请求
    async fn handle_login_request(&self, req: LoginRequest) -> Result<ProtoBody, ConnectionError> {
        info!("连接 {} 处理登录请求: user_id={}", self.connection_id, req.user_id);
        
        // TODO: 实现实际的登录逻辑
        // 这里先返回一个成功的响应
        let account_info = AccountInfoResponse {
            account_id: format!("account_{}", self.connection_id),
            total_assets: 100000.0,
            available_funds: 100000.0,
            frozen_funds: 0.0,
            total_market_value: 0.0,
            holdings: vec![], // 空持仓
            server_timestamp_utc: chrono::Utc::now().to_rfc3339(),
            message: "账户信息获取成功".to_string(),
        };

        let response = LoginResponse {
            success: true,
            message: "登录成功".to_string(),
            session_id: format!("session_{}", self.connection_id),
            initial_account_info: Some(account_info),
        };

        Ok(ProtoBody::LoginResponse(response))
    }

    /// 处理新订单请求
    async fn handle_new_order_request(&self, req: NewOrderRequest) -> Result<ProtoBody, ConnectionError> {
        info!("连接 {} 处理新订单请求: symbol={}, side={:?}, quantity={}", 
              self.connection_id, req.stock_code, req.side, req.quantity);
        
        // 转换协议类型到业务类型
        let engine_side = match req.side {
            x if x == trade_protocal_lite::OrderSide::Buy as i32 => EngineOrderSide::Buy,
            x if x == trade_protocal_lite::OrderSide::Sell as i32 => EngineOrderSide::Sell,
            _ => {
                let error_response = ErrorResponse {
                    error_code: 400,
                    error_message: "无效的订单方向".to_string(),
                    original_request_id: req.client_order_id.clone(),
                };
                return Ok(ProtoBody::ErrorResponse(error_response));
            }
        };

        // MVP阶段只支持限价单
        if req.r#type != trade_protocal_lite::OrderType::Limit as i32 {
            let error_response = ErrorResponse {
                error_code: 400,
                error_message: "当前只支持限价单".to_string(),
                original_request_id: req.client_order_id.clone(),
            };
            return Ok(ProtoBody::ErrorResponse(error_response));
        }

        // 创建限价订单
        let price = Decimal::from_f64_retain(req.price).unwrap_or_else(|| Decimal::ZERO);
        let order = Order::new_limit_order(
            req.client_order_id.clone(),
            req.stock_code.clone(),
            req.account_id.clone(),
            engine_side,
            price,
            req.quantity as u64,
            chrono::Utc::now(),
        );

        // 提交订单到引擎
        match self.order_engine.submit_order(order).await {
            Ok(server_order_id) => {
                info!("订单提交成功: client_id={}, server_id={}", req.client_order_id, server_order_id);
                
                let response = OrderUpdateResponse {
                    account_id: req.account_id,
                    server_order_id,
                    client_order_id: req.client_order_id,
                    stock_code: req.stock_code,
                    side: req.side,
                    r#type: req.r#type,
                    status: trade_protocal_lite::OrderStatus::New as i32,
                    filled_quantity_this_event: 0,
                    avg_filled_price_this_event: 0.0,
                    cumulative_filled_quantity: 0,
                    avg_cumulative_filled_price: 0.0,
                    leaves_quantity: req.quantity,
                    server_timestamp_utc: chrono::Utc::now().to_rfc3339(),
                    rejection_reason: trade_protocal_lite::RejectionReason::ReasonUnspecified as i32,
                    reject_message: "".to_string(),
                    commission: 0.0,
                };

                Ok(ProtoBody::OrderUpdateResponse(response))
            }
            Err(e) => {
                warn!("订单提交失败: client_id={}, error={}", req.client_order_id, e);
                
                let response = OrderUpdateResponse {
                    account_id: req.account_id,
                    server_order_id: "".to_string(),
                    client_order_id: req.client_order_id,
                    stock_code: req.stock_code,
                    side: req.side,
                    r#type: req.r#type,
                    status: trade_protocal_lite::OrderStatus::Rejected as i32,
                    filled_quantity_this_event: 0,
                    avg_filled_price_this_event: 0.0,
                    cumulative_filled_quantity: 0,
                    avg_cumulative_filled_price: 0.0,
                    leaves_quantity: 0,
                    server_timestamp_utc: chrono::Utc::now().to_rfc3339(),
                    rejection_reason: trade_protocal_lite::RejectionReason::Other as i32,
                    reject_message: e,
                    commission: 0.0,
                };

                Ok(ProtoBody::OrderUpdateResponse(response))
            }
        }
    }

    /// 处理撤单请求
    async fn handle_cancel_order_request(&self, req: CancelOrderRequest) -> Result<ProtoBody, ConnectionError> {
        info!("连接 {} 处理撤单请求: order_id={}", self.connection_id, req.server_order_id_to_cancel);
        
        // 首先查询订单是否存在
        let order = match self.order_engine.get_order(&req.server_order_id_to_cancel) {
            Some(order) => order,
            None => {
                let response = OrderUpdateResponse {
                    account_id: req.account_id,
                    server_order_id: req.server_order_id_to_cancel,
                    client_order_id: "".to_string(),
                    stock_code: "UNKNOWN".to_string(),
                    side: trade_protocal_lite::OrderSide::Unspecified as i32,
                    r#type: trade_protocal_lite::OrderType::Unspecified as i32,
                    status: trade_protocal_lite::OrderStatus::Rejected as i32,
                    filled_quantity_this_event: 0,
                    avg_filled_price_this_event: 0.0,
                    cumulative_filled_quantity: 0,
                    avg_cumulative_filled_price: 0.0,
                    leaves_quantity: 0,
                    server_timestamp_utc: chrono::Utc::now().to_rfc3339(),
                    rejection_reason: trade_protocal_lite::RejectionReason::OrderNotFound as i32,
                    reject_message: "订单不存在".to_string(),
                    commission: 0.0,
                };
                return Ok(ProtoBody::OrderUpdateResponse(response));
            }
        };

        // 提交撤单请求到引擎
        match self.order_engine.cancel_order(&req.server_order_id_to_cancel, &order.stock_id).await {
            Ok(()) => {
                info!("撤单请求提交成功: order_id={}", req.server_order_id_to_cancel);
                
                let response = OrderUpdateResponse {
                    account_id: req.account_id,
                    server_order_id: req.server_order_id_to_cancel,
                    client_order_id: order.order_id.to_string(),
                    stock_code: order.stock_id.to_string(),
                    side: match order.side {
                        EngineOrderSide::Buy => trade_protocal_lite::OrderSide::Buy as i32,
                        EngineOrderSide::Sell => trade_protocal_lite::OrderSide::Sell as i32,
                    },
                    r#type: trade_protocal_lite::OrderType::Limit as i32, // MVP阶段只有限价单
                    status: trade_protocal_lite::OrderStatus::Canceled as i32,
                    filled_quantity_this_event: 0,
                    avg_filled_price_this_event: 0.0,
                    cumulative_filled_quantity: order.filled_quantity() as i64,
                    avg_cumulative_filled_price: 0.0, // TODO: 计算平均成交价格
                    leaves_quantity: order.unfilled_quantity as i64,
                    server_timestamp_utc: chrono::Utc::now().to_rfc3339(),
                    rejection_reason: trade_protocal_lite::RejectionReason::ReasonUnspecified as i32,
                    reject_message: "撤单请求已提交".to_string(),
                    commission: 0.0,
                };

                Ok(ProtoBody::OrderUpdateResponse(response))
            }
            Err(e) => {
                warn!("撤单请求失败: order_id={}, error={}", req.server_order_id_to_cancel, e);
                
                let response = OrderUpdateResponse {
                    account_id: req.account_id,
                    server_order_id: req.server_order_id_to_cancel,
                    client_order_id: order.order_id.to_string(),
                    stock_code: order.stock_id.to_string(),
                    side: match order.side {
                        EngineOrderSide::Buy => trade_protocal_lite::OrderSide::Buy as i32,
                        EngineOrderSide::Sell => trade_protocal_lite::OrderSide::Sell as i32,
                    },
                    r#type: trade_protocal_lite::OrderType::Limit as i32,
                    status: trade_protocal_lite::OrderStatus::Rejected as i32,
                    filled_quantity_this_event: 0,
                    avg_filled_price_this_event: 0.0,
                    cumulative_filled_quantity: order.filled_quantity() as i64,
                    avg_cumulative_filled_price: 0.0,
                    leaves_quantity: order.unfilled_quantity as i64,
                    server_timestamp_utc: chrono::Utc::now().to_rfc3339(),
                    rejection_reason: trade_protocal_lite::RejectionReason::Other as i32,
                    reject_message: e,
                    commission: 0.0,
                };

                Ok(ProtoBody::OrderUpdateResponse(response))
            }
        }
    }

    /// 处理市场数据请求
    async fn handle_market_data_request(&self, req: MarketDataRequest) -> Result<ProtoBody, ConnectionError> {
        info!("连接 {} 处理市场数据请求: action={:?}, symbols={:?}", 
              self.connection_id, req.action, req.stock_codes);
        
        // 检查是否有市场数据服务可用
        let market_data_service = match &self.market_data_service {
            Some(service) => service,
            None => {
                // 如果没有市场数据服务，返回模拟数据（保持向后兼容）
                warn!("连接 {} 没有市场数据服务，返回模拟数据", self.connection_id);
                return self.handle_market_data_request_fallback(req).await;
            }
        };

        // 处理订阅/取消订阅请求
        match req.action {
            1 => { // Subscribe
                info!("连接 {} 订阅市场数据", self.connection_id);
                // 在实际实现中，这里可以管理订阅列表
                // 目前先返回第一个股票的数据作为响应
            }
            2 => { // Unsubscribe
                info!("连接 {} 取消订阅市场数据", self.connection_id);
                // 在实际实现中，这里可以从订阅列表中移除
                return Ok(ProtoBody::MarketDataSnapshot(MarketDataSnapshot {
                    stock_code: req.stock_codes.first().unwrap_or(&"UNKNOWN".to_string()).clone(),
                    server_timestamp_utc: chrono::Utc::now().to_rfc3339(),
                    last_price: 0.0,
                    bid_price_1: 0.0,
                    bid_volume_1: 0,
                    ask_price_1: 0.0,
                    ask_volume_1: 0,
                    open_price: 0.0,
                    high_price: 0.0,
                    low_price: 0.0,
                    prev_close_price: 0.0,
                    total_volume: 0,
                    total_turnover: 0.0,
                }));
            }
            _ => {
                warn!("连接 {} 市场数据请求操作未指定", self.connection_id);
            }
        }
        
        // 获取并返回市场数据
        if !req.stock_codes.is_empty() {
            let stock_code = &req.stock_codes[0];
            
            match market_data_service.get_market_data(stock_code).await {
                Some(market_data) => {
                    info!("连接 {} 成功获取股票 {} 的市场数据", self.connection_id, stock_code);
                    let proto_snapshot = self.convert_market_data_to_protobuf(&market_data);
                    Ok(ProtoBody::MarketDataSnapshot(proto_snapshot))
                }
                None => {
                    warn!("连接 {} 未找到股票 {} 的市场数据", self.connection_id, stock_code);
                    let error_response = ErrorResponse {
                        error_code: 404,
                        error_message: format!("未找到股票 {} 的市场数据", stock_code),
                        original_request_id: "".to_string(),
                    };
                    Ok(ProtoBody::ErrorResponse(error_response))
                }
            }
        } else {
            // 返回错误响应
            let error_response = ErrorResponse {
                error_code: 400,
                error_message: "股票代码列表为空".to_string(),
                original_request_id: "".to_string(),
            };
            Ok(ProtoBody::ErrorResponse(error_response))
        }
    }

    /// 处理市场数据请求的回退方法（模拟数据，用于向后兼容）
    async fn handle_market_data_request_fallback(&self, req: MarketDataRequest) -> Result<ProtoBody, ConnectionError> {
        info!("连接 {} 使用模拟市场数据", self.connection_id);
        
        if !req.stock_codes.is_empty() {
            let stock_code = &req.stock_codes[0];
            let response = MarketDataSnapshot {
                stock_code: stock_code.clone(),
                last_price: 100.0 + (self.connection_id as f64 % 10.0), // 模拟价格
                bid_price_1: 99.5,
                bid_volume_1: 500,
                ask_price_1: 100.5,
                ask_volume_1: 600,
                server_timestamp_utc: chrono::Utc::now().to_rfc3339(),
                open_price: 98.0,
                high_price: 102.0,
                low_price: 97.0,
                prev_close_price: 99.0,
                total_volume: 10000,
                total_turnover: 1000000.0,
            };

            Ok(ProtoBody::MarketDataSnapshot(response))
        } else {
            // 返回错误响应
            let error_response = ErrorResponse {
                error_code: 400,
                error_message: "股票代码列表为空".to_string(),
                original_request_id: "".to_string(),
            };
            Ok(ProtoBody::ErrorResponse(error_response))
        }
    }

    /// 处理账户查询请求
    async fn handle_account_query_request(&self, req: AccountQueryRequest) -> Result<ProtoBody, ConnectionError> {
        info!("连接 {} 处理账户查询请求: account_id={}", self.connection_id, req.account_id);
        
        // TODO: 实现实际的账户查询逻辑
        // 这里先返回一个模拟的账户信息
        let response = AccountInfoResponse {
            account_id: req.account_id,
            total_assets: 100000.0,
            available_funds: 80000.0,
            frozen_funds: 5000.0,
            total_market_value: 15000.0,
            holdings: vec![
                trade_protocal_lite::Holding {
                    stock_code: "000001".to_string(),
                    stock_name: "平安银行".to_string(),
                    quantity: 1000,
                    cost_price_avg: 15.0,
                    last_price: 15.5,
                    market_value: 15500.0,
                    unrealized_pnl: 500.0,
                    unrealized_pnl_ratio: 0.033,
                    available_sell_quantity: 1000,
                },
            ],
            server_timestamp_utc: chrono::Utc::now().to_rfc3339(),
            message: "账户信息查询成功".to_string(),
        };

        Ok(ProtoBody::AccountInfoResponse(response))
    }

    /// 获取连接ID
    pub fn connection_id(&self) -> ConnectionId {
        self.connection_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_heartbeat_dispatch() {
        let (order_engine, _order_rx, _match_tx) = 
            OrderEngineFactory::create_with_channels(None, OrderEngineConfig::default());
        let dispatcher = MessageDispatcher::new(1, Arc::new(order_engine));
        let heartbeat = Heartbeat {
            client_timestamp_utc: chrono::Utc::now().to_rfc3339(),
        };
        
        let trade_msg = TradeMessage::new(ProtoBody::Heartbeat(heartbeat.clone()));
        let result = dispatcher.dispatch(trade_msg).await;
        assert!(result.is_ok());
        
        let response = result.unwrap();
        assert!(response.is_some());
        
        if let Some(msg) = response {
            if let ProtoBody::HeartbeatResponse(_) = msg.body {
                // 测试通过
            } else {
                panic!("期望心跳响应");
            }
        }
    }

    #[tokio::test]
    async fn test_login_dispatch() {
        let (order_engine, _order_rx, _match_tx) = 
            OrderEngineFactory::create_with_channels(None, OrderEngineConfig::default());
        let dispatcher = MessageDispatcher::new(1, Arc::new(order_engine));
        let login_req = LoginRequest {
            user_id: "test_user".to_string(),
            password: "password".to_string(),
        };
        
        let trade_msg = TradeMessage::new(ProtoBody::LoginRequest(login_req));
        let result = dispatcher.dispatch(trade_msg).await;
        assert!(result.is_ok());
        
        let response = result.unwrap();
        assert!(response.is_some());
        
        if let Some(msg) = response {
            if let ProtoBody::LoginResponse(resp) = msg.body {
                assert!(resp.success);
                assert_eq!(resp.session_id, "session_1");
            } else {
                panic!("期望登录响应");
            }
        }
    }

    #[tokio::test]
    async fn test_new_order_dispatch() {
        let (order_engine, _order_rx, _match_tx) = 
            OrderEngineFactory::create_with_channels(None, OrderEngineConfig::default());
        let dispatcher = MessageDispatcher::new(1, Arc::new(order_engine));
        let order_req = NewOrderRequest {
            account_id: "test_account".to_string(),
            client_order_id: "client_order_1".to_string(),
            stock_code: "000001".to_string(),
            side: OrderSide::Buy as i32,
            r#type: OrderType::Limit as i32,
            quantity: 100,
            price: 15.0,
        };
        
        let trade_msg = TradeMessage::new(ProtoBody::NewOrderRequest(order_req.clone()));
        let result = dispatcher.dispatch(trade_msg).await;
        assert!(result.is_ok());
        
        let response = result.unwrap();
        assert!(response.is_some());
        
        if let Some(msg) = response {
            if let ProtoBody::OrderUpdateResponse(resp) = msg.body {
                assert_eq!(resp.account_id, order_req.account_id);
                assert_eq!(resp.client_order_id, order_req.client_order_id);
                assert_eq!(resp.stock_code, order_req.stock_code);
                assert_eq!(resp.status, OrderStatus::New as i32);
            } else {
                panic!("期望订单更新响应");
            }
        }
    }
} 