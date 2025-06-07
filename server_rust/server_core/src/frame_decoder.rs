use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, ReadHalf};
use bytes::{BytesMut, Buf};
use trade_protocal_lite::{HEADER_SIZE, ProtocolError, WireMessage};
use crate::error::ConnectionError;

/// 用于处理TCP字节流并提取完整消息帧的解码器。
/// 
/// 该解码器管理TCP连接和内部缓冲区，以处理TCP流固有的数据包分片
/// （粘包和不完整的数据包）。
#[derive(Debug)]
pub struct FrameDecoder {
    /// 用于读取数据的底层TCP流读取器
    reader: ReadHalf<TcpStream>,
    /// 用于累积多次读取数据的内部缓冲区
    buffer: BytesMut,
    /// 为防止内存耗尽攻击而设置的最大允许消息大小
    max_message_size: usize,
}

const DEFAULT_MAX_MESSAGE_SIZE: usize = 1024 * 1024; // 1MB
const DEFAULT_BUFFER_SIZE: usize = 8192; // 8KB

impl FrameDecoder {
    /// 使用给定的TCP流读取器创建一个新的FrameDecoder。
    /// 
    /// # 参数
    /// * `reader` - 要从中读取的TCP流读取器
    /// * `max_message_size` - 最大允许消息大小（字节）（默认：1MB）
    pub fn new(reader: ReadHalf<TcpStream>, max_message_size: Option<usize>) -> Self {
        Self {
            reader,
            buffer: BytesMut::with_capacity(DEFAULT_BUFFER_SIZE), // 从8KB缓冲区开始
            max_message_size: max_message_size.unwrap_or(DEFAULT_MAX_MESSAGE_SIZE), // 默认1MB
        }
    }

    /// 尝试从TCP流中读取完整的消息帧。
    /// 
    /// 此方法处理TCP流的复杂性：
    /// - 从TCP流读取数据到内部缓冲区
    /// - 解析消息头以确定完整消息的边界
    /// - 通过缓冲处理不完整的消息直到完整
    /// - 当可用时返回完整的WireMessage实例
    /// 
    /// # 返回值
    /// - `Ok(Some(WireMessage))` - 成功解码完整消息
    /// - `Ok(None)` - 连接正常关闭（EOF）
    /// - `Err(ConnectionError)` - 读取或解析过程中发生错误
    pub async fn read_frame(&mut self) -> Result<Option<WireMessage>, ConnectionError> {
        loop {
            // 首先，尝试从现有缓冲区解析完整消息
            if let Some(wire_message) = self.try_parse_message()? {
                return Ok(Some(wire_message));
            }

            // 如果无法解析完整消息，从流中读取更多数据
            let bytes_read = self.reader.read_buf(&mut self.buffer).await?;
            
            // 如果读取返回0字节，表示连接已关闭
            if bytes_read == 0 {
                // 检查缓冲区中是否还有剩余数据
                if self.buffer.is_empty() {
                    return Ok(None); // 干净的EOF
                } else {
                    return Err(ConnectionError::IncompleteMessage);
                }
            }

            // 检查潜在的内存耗尽攻击
            if self.buffer.len() > self.max_message_size {
                return Err(ConnectionError::MessageTooLarge {
                    max: self.max_message_size,
                    actual: self.buffer.len(),
                });
            }
        }
    }

    /// 尝试从当前缓冲区解析完整消息。
    /// 
    /// # 返回值
    /// - `Ok(Some(WireMessage))` - 成功解析完整消息
    /// - `Ok(None)` - 缓冲区中还没有完整消息
    /// - `Err(ConnectionError)` - 解析过程中发生错误
    fn try_parse_message(&mut self) -> Result<Option<WireMessage>, ConnectionError> {
        // WireMessage至少需要HEADER_SIZE字节来解析头部
        
        // total_length(4) + msg_type(2) + proto_body_length(4)
        if self.buffer.len() < HEADER_SIZE as usize {
            // 没有足够的数据用于完整头部
            return Ok(None);
        }

        // 我们至少有头部，让我们查看total_length以确定是否有完整消息
        let total_length = {
            let mut peek_buf = self.buffer.clone();
            peek_buf.get_u32() // 从真实缓冲区读取total_length而不消耗它
        };

        // 验证total_length
        if total_length < HEADER_SIZE {
            return Err(ConnectionError::InvalidHeader(
                format!("无效的total_length: {}小于头部大小{}", total_length, HEADER_SIZE)
            ));
        }

        if total_length as usize > self.max_message_size {
            return Err(ConnectionError::MessageTooLarge {
                max: self.max_message_size,
                actual: total_length as usize,
            });
        }

        // 检查缓冲区中是否有完整消息
        if self.buffer.len() < total_length as usize {
            // 我们还没有完整消息
            return Ok(None);
        }

        // 我们有完整消息！从缓冲区中提取它
        let message_bytes = self.buffer.split_to(total_length as usize);
        let message_bytes = message_bytes.freeze();

        // 使用WireMessage的decode_from_bytes解析完整消息
        match WireMessage::decode_from_bytes(message_bytes) {
            Ok(wire_message) => Ok(Some(wire_message)),
            Err(ProtocolError::InsufficientHeaderData { .. }) => {
                // 这不应该发生，因为我们已经检查了缓冲区大小
                Err(ConnectionError::InvalidHeader(
                    "内部错误：大小检查后头部数据不足".to_string()
                ))
            }
            Err(ProtocolError::InsufficientBodyData { expected, actual }) => {
                // 这不应该发生，因为我们已经检查了总长度
                Err(ConnectionError::InvalidHeader(
                    format!("内部错误：消息体数据不足 - 期望：{}，实际：{}", expected, actual)
                ))
            }
            Err(ProtocolError::InvalidMessageType { message_type }) => {
                Err(ConnectionError::UnsupportedMessageType(message_type))
            }
            Err(other_error) => {
                Err(ConnectionError::Application(
                    format!("解码WireMessage失败：{}", other_error)
                ))
            }
        }
    }

    /// 返回内部缓冲区的当前大小。
    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }

    /// 返回内部缓冲区的容量。
    pub fn buffer_capacity(&self) -> usize {
        self.buffer.capacity()
    }

    /// 消费FrameDecoder并返回底层的ReadHalf。
    /// 这在需要升级连接或以不同方式处理连接时很有用。
    pub fn into_reader(self) -> ReadHalf<TcpStream> {
        self.reader
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::{TcpListener, TcpStream};
    use tokio::io::AsyncWriteExt;
    use trade_protocal_lite::{TradeMessage, ProtoBody, LoginRequest};

    #[tokio::test]
    async fn test_frame_decoder_complete_message() {
        // 创建测试消息
        let login_request = LoginRequest {
            user_id: "test_user".to_string(),
            password: "test_password".to_string(),
        };
        let trade_msg = TradeMessage::new(ProtoBody::LoginRequest(login_request));
        let wire_msg: WireMessage = trade_msg.try_into().unwrap();
        let wire_msg_bytes = wire_msg.encode_to_bytes();

        // 设置测试用的TCP连接
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // 生成任务发送测试数据
        tokio::spawn(async move {
            let mut stream = TcpStream::connect(addr).await.unwrap();
            stream.write_all(&wire_msg_bytes).await.unwrap();
            stream.shutdown().await.unwrap();
        });

        // 接受连接并测试解码器
        let (stream, _) = listener.accept().await.unwrap();
        let (reader, _writer) = tokio::io::split(stream);
        let mut decoder = FrameDecoder::new(reader, None);

        // 尝试读取帧
        let result = decoder.read_frame().await.unwrap();
        assert!(result.is_some());

        let received_wire_msg = result.unwrap();
        assert_eq!(received_wire_msg.msg_type, trade_protocal_lite::MessageType::LoginRequest);

        // 再次尝试读取，应该返回None（EOF）
        let result = decoder.read_frame().await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_frame_decoder_fragmented_message() {
        // 创建测试消息
        let login_request = LoginRequest {
            user_id: "test_user_with_long_name".to_string(),
            password: "test_password_with_long_content".to_string(),
        };
        let trade_msg = TradeMessage::new(ProtoBody::LoginRequest(login_request));
        let wire_msg: WireMessage = trade_msg.try_into().unwrap();
        let wire_msg_bytes = wire_msg.encode_to_bytes();

        // 设置测试用的TCP连接
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Spawn a task to send the test data in fragments
        let wire_msg_bytes_clone = wire_msg_bytes.clone();
        tokio::spawn(async move {
            let mut stream = TcpStream::connect(addr).await.unwrap();
            
            // Send the message in two parts to simulate fragmentation
            let mid_point = wire_msg_bytes_clone.len() / 2;
            let (first_half, second_half) = wire_msg_bytes_clone.split_at(mid_point);
            
            stream.write_all(first_half).await.unwrap();
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            stream.write_all(second_half).await.unwrap();
            stream.shutdown().await.unwrap();
        });

        // Accept the connection and test the decoder
        let (stream, _) = listener.accept().await.unwrap();
        let (reader, _writer) = tokio::io::split(stream);
        let mut decoder = FrameDecoder::new(reader, None);

        // Try to read the frame - should handle fragmentation correctly
        let result = decoder.read_frame().await.unwrap();
        assert!(result.is_some());

        let received_wire_msg = result.unwrap();
        assert_eq!(received_wire_msg.msg_type, trade_protocal_lite::MessageType::LoginRequest);
    }

    #[tokio::test]
    async fn test_frame_decoder_oversized_message() {
        // Create a test message
        let login_request = LoginRequest {
            user_id: "test_user".to_string(),
            password: "test_password".to_string(),
        };
        let trade_msg = TradeMessage::new(ProtoBody::LoginRequest(login_request));
        let wire_msg: WireMessage = trade_msg.try_into().unwrap();
        let wire_msg_bytes = wire_msg.encode_to_bytes();

        // Set up TCP connection for testing
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Spawn a task to send the test data
        tokio::spawn(async move {
            let mut stream = TcpStream::connect(addr).await.unwrap();
            stream.write_all(&wire_msg_bytes).await.unwrap();
            stream.shutdown().await.unwrap();
        });

        // Accept the connection and test the decoder with a small max_message_size
        let (stream, _) = listener.accept().await.unwrap();
        let (reader, _writer) = tokio::io::split(stream);
        let mut decoder = FrameDecoder::new(reader, Some(10)); // Set max_message_size to 10 bytes

        // Try to read the frame - should return MessageTooLarge error
        let result = decoder.read_frame().await;
        assert!(result.is_err());
        match result {
            Err(ConnectionError::MessageTooLarge { max, actual }) => {
                assert_eq!(max, 10);
                assert!(actual > max);
            }
            _ => panic!("Expected MessageTooLarge error"),
        }
    }
}