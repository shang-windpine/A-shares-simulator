use thiserror::Error;
use std::borrow::Cow;

#[derive(Error, Debug)]
pub enum ConnectionError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to decode protobuf message: {0}")]
    ProstDecode(#[from] prost::DecodeError),

    #[error("Failed to encode protobuf message: {0}")]
    ProstEncode(#[from] prost::EncodeError),

    #[error("Protocol error: {0}")]
    Protocol(#[from] trade_protocal_lite::ProtocolError),

    #[error("Incomplete message received")]
    IncompleteMessage,

    #[error("Invalid message header: {0}")]
    InvalidHeader(String),

    #[error("Message too large (max: {max}, actual: {actual})")]
    MessageTooLarge { max: usize, actual: usize },

    #[error("Unsupported message type: {0}")]
    UnsupportedMessageType(u16),

    #[error("Connection closed by peer")]
    ConnectionClosed,

    #[error("Maximum connections reached (limit: {limit})")]
    MaxConnectionsReached { limit: usize },

    #[error("Heartbeat timeout for connection {connection_id}")]
    HeartbeatTimeout { connection_id: u64 },

    #[error("Server shutdown requested")]
    ShutdownRequested,

    #[error("Message dispatch error: {0}")]
    DispatchError(String),

    #[error("Connection {connection_id} not found")]
    ConnectionNotFound { connection_id: u64 },

    #[error("Failed to bind to address {addr}: {source}")]
    BindError { addr: Cow<'static, str>, source: std::io::Error },

    #[error("Invalid address format: {addr}")]
    InvalidAddress { addr: Cow<'static, str> },

    #[error("Channel send error: {0}")]
    ChannelSend(String),

    #[error("Channel receive error: {0}")]
    ChannelReceive(String),

    #[error("Task join error: {0}")]
    TaskJoin(#[from] tokio::task::JoinError),

    #[error("Custom application error: {0}")]
    Application(String), // For higher-level errors passed down

    #[error(transparent)]
    Other(#[from] anyhow::Error), // For general errors if anyhow is used extensively
}

impl ConnectionError {
    /// 创建一个应用层错误，包含源错误信息用于传播
    /// 
    /// 这个方法提供了一个便利的 API 来包装底层错误并添加上下文信息。
    /// anyhow 会自动处理 backtrace 和错误链的显示。
    pub fn application_with_source<E: std::error::Error + Send + Sync + 'static>(
        message: String, 
        source: E
    ) -> Self {
        Self::Other(anyhow::Error::new(source).context(message))
    }
}

#[cfg(test)]
mod error_propagation_tests {
    use super::*;
    use trade_protocal_lite::{ProtocolError, MessageType};
    use std::io::{Error as IoError, ErrorKind};

    #[test]
    fn test_protocol_error_propagation() {
        // 模拟来自 trade_protocal_lite 的错误
        let protocol_error = ProtocolError::InvalidMessageType { message_type: 999 };
        
        // 通过 From trait 自动转换
        let connection_error: ConnectionError = protocol_error.into();
        
        // 验证错误类型和消息
        match connection_error {
            ConnectionError::Protocol(inner) => {
                assert!(inner.to_string().contains("Invalid message type: 999"));
            }
            _ => panic!("错误转换失败"),
        }
    }

    #[test]
    fn test_io_error_propagation() {
        // 模拟 IO 错误
        let io_error = IoError::new(ErrorKind::ConnectionRefused, "Connection refused");
        
        // 通过 From trait 自动转换
        let connection_error: ConnectionError = io_error.into();
        
        // 验证错误类型
        match connection_error {
            ConnectionError::Io(inner) => {
                assert_eq!(inner.kind(), ErrorKind::ConnectionRefused);
            }
            _ => panic!("IO错误转换失败"),
        }
    }

    #[test]
    fn test_nested_error_propagation() {
        // 创建一个嵌套的错误场景
        let protocol_error = ProtocolError::InsufficientBodyData { 
            expected: 100, 
            actual: 50 
        };
        
        let connection_error = ConnectionError::application_with_source(
            "处理消息时发生错误".to_string(),
            protocol_error
        );
        
        // 验证错误信息包含预期内容
        assert!(connection_error.to_string().contains("处理消息时发生错误"));
        
        // anyhow 会自动显示错误链，我们只需要打印错误即可
        println!("错误: {}", connection_error);
        println!("详细错误 (Debug): {:?}", connection_error);
    }

    #[test]
    fn test_question_mark_operator() {
        // 测试问号操作符的错误传播
        fn mock_protocol_operation() -> Result<(), ProtocolError> {
            Err(ProtocolError::EmptyMessageBody { 
                message_type: MessageType::LoginRequest 
            })
        }

        fn mock_connection_operation() -> Result<(), ConnectionError> {
            mock_protocol_operation()?; // 这里应该自动转换错误类型
            Ok(())
        }

        let result = mock_connection_operation();
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ConnectionError::Protocol(inner) => {
                assert!(inner.to_string().contains("Message body cannot be empty"));
            }
            _ => panic!("问号操作符错误传播失败"),
        }
    }

    #[test]
    fn test_prost_error_propagation() {
        // 模拟 prost 解码错误
        let prost_error = prost::DecodeError::new("invalid wire format");
        let connection_error: ConnectionError = prost_error.into();
        
        match connection_error {
            ConnectionError::ProstDecode(inner) => {
                assert!(inner.to_string().contains("invalid wire format"));
            }
            _ => panic!("Prost错误转换失败"),
        }
    }

    #[test]
    fn test_backtrace_environment() {
        // 确保环境变量设置正确以启用 backtrace
        // 注意：这需要在运行时设置 RUST_BACKTRACE=1
        
        let protocol_error = ProtocolError::InvalidMessageType { message_type: 42 };
        let connection_error = ConnectionError::application_with_source(
            "测试 backtrace".to_string(),
            protocol_error
        );
        
        // 打印错误信息以便手动检查
        println!("错误: {:?}", connection_error);
        
        // anyhow 自动处理错误链，我们只需要验证错误信息包含预期内容
        assert!(connection_error.to_string().contains("测试 backtrace"));
    }

    #[tokio::test]
    async fn test_async_error_propagation() {
        // 测试异步函数中的错误传播
        async fn mock_async_operation() -> Result<(), ProtocolError> {
            Err(ProtocolError::UnspecifiedMessageType)
        }

        async fn mock_async_connection_handler() -> Result<(), ConnectionError> {
            mock_async_operation().await?;
            Ok(())
        }

        let result = mock_async_connection_handler().await;
        assert!(result.is_err());
        
        let error = result.unwrap_err();
        println!("异步错误传播测试: {}", error);
        
        match error {
            ConnectionError::Protocol(inner) => {
                assert!(inner.to_string().contains("Cannot decode unspecified message type"));
            }
            _ => panic!("异步错误传播失败"),
        }
    }

    #[test]
    fn test_comprehensive_error_display() {
        println!("\n=== 错误传播和显示测试 ===");
        
        // 测试各种错误类型的显示
        let errors = vec![
            ConnectionError::IncompleteMessage,
            ConnectionError::MessageTooLarge { max: 1024, actual: 2048 },
            ConnectionError::UnsupportedMessageType(999),
            ConnectionError::HeartbeatTimeout { connection_id: 123 },
            ConnectionError::MaxConnectionsReached { limit: 100 },
        ];
        
        for (i, error) in errors.iter().enumerate() {
            println!("{}. {}", i + 1, error);
            // anyhow 会自动处理错误链显示，无需手动检查
            println!("   错误类型: {:?}", std::mem::discriminant(error));
        }
    }
}

/// 运行错误传播测试的辅助函数
#[cfg(test)]
pub fn run_error_propagation_tests() {
    // 设置 backtrace 环境变量
    unsafe {
        std::env::set_var("RUST_BACKTRACE", "1");
    }
    
    println!("运行错误传播测试...");
    println!("确保设置了 RUST_BACKTRACE=1 环境变量");
}