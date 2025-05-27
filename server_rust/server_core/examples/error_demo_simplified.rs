use server_core::{ConnectionError};
use trade_protocal_lite::{ProtocolError, WireMessage, TradeMessage, ProtoBody, LoginRequest};
use std::io::{Error as IoError, ErrorKind};

/// 模拟底层协议操作
fn simulate_protocol_error() -> Result<(), ProtocolError> {
    Err(ProtocolError::InvalidMessageType { message_type: 999 })
}

/// 模拟 IO 操作
fn simulate_io_error() -> Result<(), IoError> {
    Err(IoError::new(ErrorKind::ConnectionRefused, "模拟连接被拒绝"))
}

/// 模拟复合错误场景 - 协议错误
fn handle_protocol_message() -> Result<(), ConnectionError> {
    // 这里的 ? 操作符会自动将 ProtocolError 转换为 ConnectionError
    simulate_protocol_error()?;
    Ok(())
}

/// 模拟嵌套错误传播
fn simulate_nested_error_propagation() -> Result<(), ConnectionError> {
    // 创建一个底层的协议错误
    let protocol_error = ProtocolError::InsufficientBodyData { 
        expected: 256, 
        actual: 128 
    };
    
    // 使用我们的辅助方法创建包含源错误的应用层错误
    Err(ConnectionError::application_with_source(
        "在处理客户端消息时发生错误".to_string(),
        protocol_error
    ))
}

/// 演示复杂的错误传播链
async fn complex_error_chain() -> Result<(), ConnectionError> {
    // 模拟一系列可能失败的操作
    handle_protocol_message().map_err(|e| {
        ConnectionError::application_with_source(
            "第一层：协议处理失败".to_string(),
            e
        )
    })?;
    
    Ok(())
}

fn main() {
    println!("=== Server Core 错误处理演示 (简化版) ===");
    println!("请通过环境变量 RUST_LIB_BACKTRACE=1 启用 backtrace");
    println!("当前 RUST_LIB_BACKTRACE: {:?}", std::env::var("RUST_LIB_BACKTRACE"));
    println!("当前 RUST_BACKTRACE: {:?}\n", std::env::var("RUST_BACKTRACE"));
    
    // 测试 1: 直接的协议错误传播
    println!("1. 协议错误传播测试:");
    match handle_protocol_message() {
        Ok(_) => println!("   操作成功"),
        Err(e) => {
            println!("   错误: {}", e);
            println!("   详细信息: {:#}", e); // 使用 anyhow 的格式化显示
        }
    }
    
    println!();
    
    // 测试 2: 嵌套错误传播 - 显示 anyhow 的自动错误链
    println!("2. 嵌套错误传播测试 (anyhow 自动错误链):");
    match simulate_nested_error_propagation() {
        Ok(_) => println!("   操作成功"),
        Err(e) => {
            println!("   简单格式: {}", e);
            println!("\n   详细格式 (包含错误链):");
            println!("   {:#}", e);
            
            println!("\n   Debug 格式 (包含 backtrace):");
            println!("   {:?}", e);
        }
    }
    
    println!();
    
    // 测试 3: 复杂错误链
    println!("3. 复杂错误链测试:");
    let rt = tokio::runtime::Runtime::new().unwrap();
    match rt.block_on(complex_error_chain()) {
        Ok(_) => println!("   操作成功"),
        Err(e) => {
            println!("   错误: {}", e);
            println!("\n   完整错误链:");
            println!("   {:#}", e);
        }
    }
    
    println!();
    
    // 测试 4: 真实的 WireMessage 错误场景
    println!("4. 真实 WireMessage 错误场景:");
    
    // 创建一个有效的消息
    let login_request = LoginRequest {
        user_id: "test_user".to_string(),
        password: "test_password".to_string(),
    };
    let trade_msg = TradeMessage::new(ProtoBody::LoginRequest(login_request));
    
    match trade_msg.try_into() {
        Ok(wire_msg) => {
            let wire_msg: WireMessage = wire_msg;
            println!("   成功创建 WireMessage: type={:?}, body_len={}", 
                    wire_msg.msg_type, wire_msg.proto_body.len());
            
            // 模拟解码错误
            let corrupted_data = vec![0x00, 0x01, 0x02]; // 无效数据
            match WireMessage::decode_from_bytes(corrupted_data.into()) {
                Ok(_) => println!("   意外成功解码损坏的数据"),
                Err(protocol_err) => {
                    let connection_err: ConnectionError = protocol_err.into();
                    println!("   解码错误: {}", connection_err);
                    println!("   详细信息: {:#}", connection_err);
                }
            }
        }
        Err(e) => {
            let connection_err: ConnectionError = e.into();
            println!("   创建 WireMessage 失败: {}", connection_err);
        }
    }
    
    println!("\n=== 演示完成 ===");
    println!("注意事项:");
    println!("1. anyhow 在 Rust ≥ 1.65 中会自动捕获和显示 backtrace");
    println!("2. 使用 RUST_LIB_BACKTRACE=1 只显示错误的 backtrace（不包括 panic）");
    println!("3. 使用 RUST_BACKTRACE=1 会同时显示错误和 panic 的 backtrace");
    println!("4. 错误链通过 anyhow 的 '{{:#}}' 格式自动显示");
} 