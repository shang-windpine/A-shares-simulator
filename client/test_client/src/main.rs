use trade_protocal_lite::{TradeMessage, MessageType};
use trade_protocal_lite::protocol::{OrderSide, OrderType};

fn main() {
    println!("测试客户端启动！");
    
    // 创建登录请求消息
    let login_msg = TradeMessage::new_login_request(
        "test_user".to_string(),
        "test_password".to_string()
    );
    
    println!("创建登录消息: {:?}", login_msg);
    println!("消息类型: {:?}", login_msg.message_type());
    
    // 编码消息到字节
    match login_msg.encode_to_bytes() {
        Ok(bytes) => {
            println!("消息编码成功，长度: {} 字节", bytes.len());
            
            // 解码回消息
            match TradeMessage::decode_from_bytes(MessageType::LoginRequest, bytes) {
                Ok(decoded_msg) => {
                    println!("消息解码成功: {:?}", decoded_msg);
                }
                Err(e) => {
                    println!("解码失败: {}", e);
                }
            }
        }
        Err(e) => {
            println!("编码失败: {}", e);
        }
    }
    
    // 创建新订单请求消息
    let order_msg = TradeMessage::new_order_request(
        "account_001".to_string(),
        "client_order_123".to_string(),
        "000001.SZ".to_string(),  // 平安银行
        OrderSide::Buy,
        OrderType::Limit,
        100,    // 100股
        10.50   // 10.50元
    );
    
    println!("创建订单消息: {:?}", order_msg);
    
    // 创建心跳消息
    let heartbeat_msg = TradeMessage::new_heartbeat("2024-01-01T12:00:00Z".to_string());
    println!("创建心跳消息: {:?}", heartbeat_msg);
}
