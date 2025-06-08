use thiserror::Error;
use super::MessageType;

/// Unified error type for handling various protocol-related error cases
#[derive(Error, Debug)]
pub enum ProtocolError {
    /// Protobuf encoding error
    #[error("Failed to encode protobuf message: {0}")]
    ProtobufEncode(#[from] prost::EncodeError),

    /// Protobuf decoding error
    #[error("Failed to decode protobuf message: {0}")]
    ProtobufDecode(#[from] prost::DecodeError),

    /// Invalid message type
    #[error("Invalid message type: {message_type}")]
    InvalidMessageType { message_type: u16 },

    /// Insufficient data for message header
    #[error("Insufficient data for message header, expected at least {expected} bytes, got {actual} bytes")]
    InsufficientHeaderData { expected: usize, actual: usize },

    /// Insufficient data for message body
    #[error("Insufficient data for message body, expected {expected} bytes, got {actual} bytes")]
    InsufficientBodyData { expected: u32, actual: usize },

    /// Message length mismatch
    #[error("Message length mismatch: total_length={total_length}, header_size={header_size}, body_length={body_length}")]
    MessageLengthMismatch {
        total_length: u32,
        header_size: u32,
        body_length: u32,
    },

    /// Cannot decode unspecified message type
    #[error("Cannot decode unspecified message type")]
    UnspecifiedMessageType,

    /// Message body cannot be empty
    #[error("Message body cannot be empty for message type: {message_type:?}")]
    EmptyMessageBody { message_type: MessageType },
}

/// Type alias for protocol operation results
pub type ProtocolResult<T> = Result<T, ProtocolError>; 