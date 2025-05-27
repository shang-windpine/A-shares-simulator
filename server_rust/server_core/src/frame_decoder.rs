use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, ReadHalf};
use bytes::{BytesMut, Buf};
use trade_protocal_lite::{HEADER_SIZE, ProtocolError, WireMessage};
use crate::error::ConnectionError;

/// Frame decoder for handling TCP byte streams and extracting complete message frames.
/// 
/// This decoder manages a TCP connection and internal buffer to handle packet fragmentation
/// (sticky packets and incomplete packets) that are inherent to TCP streams.
#[derive(Debug)]
pub struct FrameDecoder {
    /// The underlying TCP stream reader for reading data
    reader: ReadHalf<TcpStream>,
    /// Internal buffer to accumulate data from multiple reads
    buffer: BytesMut,
    /// Maximum allowed message size to prevent memory exhaustion attacks
    max_message_size: usize,
}

const DEFAULT_MAX_MESSAGE_SIZE: usize = 1024 * 1024; // 1MB
const DEFAULT_BUFFER_SIZE: usize = 8192; // 8KB

impl FrameDecoder {
    /// Creates a new FrameDecoder with the given TCP stream reader.
    /// 
    /// # Arguments
    /// * `reader` - The TCP stream reader to read from
    /// * `max_message_size` - Maximum allowed message size in bytes (default: 1MB)
    pub fn new(reader: ReadHalf<TcpStream>, max_message_size: Option<usize>) -> Self {
        Self {
            reader,
            buffer: BytesMut::with_capacity(DEFAULT_BUFFER_SIZE), // Start with 8KB buffer
            max_message_size: max_message_size.unwrap_or(DEFAULT_MAX_MESSAGE_SIZE), // Default 1MB
        }
    }

    /// Attempts to read a complete message frame from the TCP stream.
    /// 
    /// This method handles the complexity of TCP streaming:
    /// - Reads data from the TCP stream into an internal buffer
    /// - Parses message headers to determine complete message boundaries
    /// - Handles incomplete messages by buffering until complete
    /// - Returns complete WireMessage instances when available
    /// 
    /// # Returns
    /// - `Ok(Some(WireMessage))` - A complete message was successfully decoded
    /// - `Ok(None)` - Connection was gracefully closed (EOF)
    /// - `Err(ConnectionError)` - An error occurred during reading or parsing
    pub async fn read_frame(&mut self) -> Result<Option<WireMessage>, ConnectionError> {
        loop {
            // First, try to parse a complete message from the existing buffer
            if let Some(wire_message) = self.try_parse_message()? {
                return Ok(Some(wire_message));
            }

            // If we can't parse a complete message, read more data from the stream
            let bytes_read = self.reader.read_buf(&mut self.buffer).await?;
            
            // If read returns 0 bytes, the connection was closed
            if bytes_read == 0 {
                // Check if we have any remaining data in the buffer
                if self.buffer.is_empty() {
                    return Ok(None); // Clean EOF
                } else {
                    return Err(ConnectionError::IncompleteMessage);
                }
            }

            // Check for potential memory exhaustion attack
            if self.buffer.len() > self.max_message_size {
                return Err(ConnectionError::MessageTooLarge {
                    max: self.max_message_size,
                    actual: self.buffer.len(),
                });
            }
        }
    }

    /// Attempts to parse a complete message from the current buffer.
    /// 
    /// # Returns
    /// - `Ok(Some(WireMessage))` - Successfully parsed a complete message
    /// - `Ok(None)` - Buffer doesn't contain a complete message yet
    /// - `Err(ConnectionError)` - Parsing error occurred
    fn try_parse_message(&mut self) -> Result<Option<WireMessage>, ConnectionError> {
        // WireMessage needs at least HEADER_SIZE bytes to parse the header
        
        // total_length(4) + msg_type(2) + proto_body_length(4)
        if self.buffer.len() < HEADER_SIZE as usize {
            // Not enough data for a complete header
            return Ok(None);
        }

        // We have at least a header, let's peek at the total_length to see if we have a complete message
        let total_length = {
            let mut peek_buf = self.buffer.clone();
            peek_buf.get_u32() // Read total_length without consuming from the real buffer
        };

        // Validate total_length
        if total_length < HEADER_SIZE {
            return Err(ConnectionError::InvalidHeader(
                format!("Invalid total_length: {} is less than header size {}", total_length, HEADER_SIZE)
            ));
        }

        if total_length as usize > self.max_message_size {
            return Err(ConnectionError::MessageTooLarge {
                max: self.max_message_size,
                actual: total_length as usize,
            });
        }

        // Check if we have the complete message in the buffer
        if self.buffer.len() < total_length as usize {
            // We don't have the complete message yet
            return Ok(None);
        }

        // We have a complete message! Extract it from the buffer
        let message_bytes = self.buffer.split_to(total_length as usize);
        let message_bytes = message_bytes.freeze();

        // Use WireMessage's decode_from_bytes to parse the complete message
        match WireMessage::decode_from_bytes(message_bytes) {
            Ok(wire_message) => Ok(Some(wire_message)),
            Err(ProtocolError::InsufficientHeaderData { .. }) => {
                // This should not happen since we already checked the buffer size
                Err(ConnectionError::InvalidHeader(
                    "Internal error: insufficient header data after size check".to_string()
                ))
            }
            Err(ProtocolError::InsufficientBodyData { expected, actual }) => {
                // This should not happen since we already checked the total length
                Err(ConnectionError::InvalidHeader(
                    format!("Internal error: insufficient body data - expected: {}, actual: {}", expected, actual)
                ))
            }
            Err(ProtocolError::InvalidMessageType { message_type }) => {
                Err(ConnectionError::UnsupportedMessageType(message_type))
            }
            Err(other_error) => {
                Err(ConnectionError::Application(
                    format!("Failed to decode WireMessage: {}", other_error)
                ))
            }
        }
    }

    /// Returns the current size of the internal buffer.
    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }

    /// Returns the capacity of the internal buffer.
    pub fn buffer_capacity(&self) -> usize {
        self.buffer.capacity()
    }

    /// Consumes the FrameDecoder and returns the underlying ReadHalf.
    /// This is useful when you need to upgrade the connection or handle it differently.
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

        // Accept the connection and test the decoder
        let (stream, _) = listener.accept().await.unwrap();
        let (reader, _writer) = tokio::io::split(stream);
        let mut decoder = FrameDecoder::new(reader, None);

        // Try to read the frame
        let result = decoder.read_frame().await.unwrap();
        assert!(result.is_some());

        let received_wire_msg = result.unwrap();
        assert_eq!(received_wire_msg.msg_type, trade_protocal_lite::MessageType::LoginRequest);

        // Try to read again, should return None (EOF)
        let result = decoder.read_frame().await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_frame_decoder_fragmented_message() {
        // Create a test message
        let login_request = LoginRequest {
            user_id: "test_user_with_long_name".to_string(),
            password: "test_password_with_long_content".to_string(),
        };
        let trade_msg = TradeMessage::new(ProtoBody::LoginRequest(login_request));
        let wire_msg: WireMessage = trade_msg.try_into().unwrap();
        let wire_msg_bytes = wire_msg.encode_to_bytes();

        // Set up TCP connection for testing
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