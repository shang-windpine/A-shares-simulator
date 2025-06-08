pub mod protos_generated;
pub mod errors;

use bytes::{Bytes, BytesMut, Buf, BufMut};
use prost::Message;
use std::convert::TryFrom;

pub use protos_generated::trade_protocal_lite as protocol;
pub use protocol::*;

pub use errors::{ProtocolError, ProtocolResult};

#[derive(Debug, Clone, PartialEq)]
pub enum ProtoBody {
    LoginRequest(LoginRequest),
    LoginResponse(LoginResponse),
    NewOrderRequest(NewOrderRequest),
    OrderUpdateResponse(OrderUpdateResponse),
    CancelOrderRequest(CancelOrderRequest),
    MarketDataRequest(MarketDataRequest),
    MarketDataSnapshot(MarketDataSnapshot),
    AccountQueryRequest(AccountQueryRequest),
    AccountInfoResponse(AccountInfoResponse),
    Heartbeat(Heartbeat),
    HeartbeatResponse(HeartbeatResponse),
    ErrorResponse(ErrorResponse),
}

impl ProtoBody {
    /// Returns the message type for this proto body.
    pub fn message_type(&self) -> MessageType {
        match self {
            ProtoBody::LoginRequest(_) => MessageType::LoginRequest,
            ProtoBody::LoginResponse(_) => MessageType::LoginResponse,
            ProtoBody::NewOrderRequest(_) => MessageType::NewOrderRequest,
            ProtoBody::OrderUpdateResponse(_) => MessageType::OrderUpdateResponse,
            ProtoBody::CancelOrderRequest(_) => MessageType::CancelOrderRequest,
            ProtoBody::MarketDataRequest(_) => MessageType::MarketDataRequest,
            ProtoBody::MarketDataSnapshot(_) => MessageType::MarketDataSnapshot,
            ProtoBody::AccountQueryRequest(_) => MessageType::AccountQueryRequest,
            ProtoBody::AccountInfoResponse(_) => MessageType::AccountInfoResponse,
            ProtoBody::Heartbeat(_) => MessageType::Heartbeat,
            ProtoBody::HeartbeatResponse(_) => MessageType::HeartbeatResponse,
            ProtoBody::ErrorResponse(_) => MessageType::ErrorResponse,
        }
    }

    /// Encodes the proto body to bytes using prost's efficient encoding.
    pub fn encode_to_bytes(&self) -> ProtocolResult<Bytes> {
        let mut buf = BytesMut::new();
        match self {
            ProtoBody::LoginRequest(msg) => msg.encode(&mut buf)?,
            ProtoBody::LoginResponse(msg) => msg.encode(&mut buf)?,
            ProtoBody::NewOrderRequest(msg) => msg.encode(&mut buf)?,
            ProtoBody::OrderUpdateResponse(msg) => msg.encode(&mut buf)?,
            ProtoBody::CancelOrderRequest(msg) => msg.encode(&mut buf)?,
            ProtoBody::MarketDataRequest(msg) => msg.encode(&mut buf)?,
            ProtoBody::MarketDataSnapshot(msg) => msg.encode(&mut buf)?,
            ProtoBody::AccountQueryRequest(msg) => msg.encode(&mut buf)?,
            ProtoBody::AccountInfoResponse(msg) => msg.encode(&mut buf)?,
            ProtoBody::Heartbeat(msg) => msg.encode(&mut buf)?,
            ProtoBody::HeartbeatResponse(msg) => msg.encode(&mut buf)?,
            ProtoBody::ErrorResponse(msg) => msg.encode(&mut buf)?,
        }
        Ok(buf.freeze())
    }

    /// Decodes a proto body from bytes based on the message type.
    pub fn decode_from_bytes(msg_type: MessageType, data: Bytes) -> ProtocolResult<Self> {
        match msg_type {
            MessageType::LoginRequest => {
                let msg = LoginRequest::decode(data)?;
                Ok(ProtoBody::LoginRequest(msg))
            }
            MessageType::LoginResponse => {
                let msg = LoginResponse::decode(data)?;
                Ok(ProtoBody::LoginResponse(msg))
            }
            MessageType::NewOrderRequest => {
                let msg = NewOrderRequest::decode(data)?;
                Ok(ProtoBody::NewOrderRequest(msg))
            }
            MessageType::OrderUpdateResponse => {
                let msg = OrderUpdateResponse::decode(data)?;
                Ok(ProtoBody::OrderUpdateResponse(msg))
            }
            MessageType::CancelOrderRequest => {
                let msg = CancelOrderRequest::decode(data)?;
                Ok(ProtoBody::CancelOrderRequest(msg))
            }
            MessageType::MarketDataRequest => {
                let msg = MarketDataRequest::decode(data)?;
                Ok(ProtoBody::MarketDataRequest(msg))
            }
            MessageType::MarketDataSnapshot => {
                let msg = MarketDataSnapshot::decode(data)?;
                Ok(ProtoBody::MarketDataSnapshot(msg))
            }
            MessageType::AccountQueryRequest => {
                let msg = AccountQueryRequest::decode(data)?;
                Ok(ProtoBody::AccountQueryRequest(msg))
            }
            MessageType::AccountInfoResponse => {
                let msg = AccountInfoResponse::decode(data)?;
                Ok(ProtoBody::AccountInfoResponse(msg))
            }
            MessageType::Heartbeat => {
                let msg = Heartbeat::decode(data)?;
                Ok(ProtoBody::Heartbeat(msg))
            }
            MessageType::HeartbeatResponse => {
                let msg = HeartbeatResponse::decode(data)?;
                Ok(ProtoBody::HeartbeatResponse(msg))
            }
            MessageType::ErrorResponse => {
                let msg = ErrorResponse::decode(data)?;
                Ok(ProtoBody::ErrorResponse(msg))
            }
            MessageType::Unspecified => {
                Err(ProtocolError::UnspecifiedMessageType)
            }
        }
    }

    /// Returns the encoded length of this proto body.
    pub fn encoded_len(&self) -> usize {
        match self {
            ProtoBody::LoginRequest(msg) => msg.encoded_len(),
            ProtoBody::LoginResponse(msg) => msg.encoded_len(),
            ProtoBody::NewOrderRequest(msg) => msg.encoded_len(),
            ProtoBody::OrderUpdateResponse(msg) => msg.encoded_len(),
            ProtoBody::CancelOrderRequest(msg) => msg.encoded_len(),
            ProtoBody::MarketDataRequest(msg) => msg.encoded_len(),
            ProtoBody::MarketDataSnapshot(msg) => msg.encoded_len(),
            ProtoBody::AccountQueryRequest(msg) => msg.encoded_len(),
            ProtoBody::AccountInfoResponse(msg) => msg.encoded_len(),
            ProtoBody::Heartbeat(msg) => msg.encoded_len(),
            ProtoBody::HeartbeatResponse(msg) => msg.encoded_len(),
            ProtoBody::ErrorResponse(msg) => msg.encoded_len(),
        }
    }
}

/// Message type enumeration for protocol identification.
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageType {
    Unspecified = 0,
    LoginRequest = 1,
    LoginResponse = 2,
    NewOrderRequest = 3,
    OrderUpdateResponse = 4,
    CancelOrderRequest = 5,
    MarketDataRequest = 6,
    MarketDataSnapshot = 7,
    AccountQueryRequest = 8,
    AccountInfoResponse = 9,
    Heartbeat = 10,
    HeartbeatResponse = 11,
    ErrorResponse = 12,
}

impl TryFrom<u16> for MessageType {
    type Error = ProtocolError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(MessageType::Unspecified),
            1 => Ok(MessageType::LoginRequest),
            2 => Ok(MessageType::LoginResponse),
            3 => Ok(MessageType::NewOrderRequest),
            4 => Ok(MessageType::OrderUpdateResponse),
            5 => Ok(MessageType::CancelOrderRequest),
            6 => Ok(MessageType::MarketDataRequest),
            7 => Ok(MessageType::MarketDataSnapshot),
            8 => Ok(MessageType::AccountQueryRequest),
            9 => Ok(MessageType::AccountInfoResponse),
            10 => Ok(MessageType::Heartbeat),
            11 => Ok(MessageType::HeartbeatResponse),
            12 => Ok(MessageType::ErrorResponse),
            _ => Err(ProtocolError::InvalidMessageType { message_type: value }),
        }
    }
}

impl From<MessageType> for u16 {
    fn from(msg_type: MessageType) -> u16 {
        msg_type as u16
    }
}

/// Application-layer message structure that provides type safety and ease of use.
/// This is the primary structure used by application code.
#[derive(Debug, Clone, PartialEq)]
pub struct ProtoMessage {
    /// The protobuf message body wrapped in a type-safe enum.
    pub body: ProtoBody,
}

impl ProtoMessage {
    /// Creates a new TradeMessage with the given protobuf body.
    pub fn new(body: ProtoBody) -> Self {
        Self { body }
    }

    /// Returns the message type of this trade message.
    pub fn message_type(&self) -> MessageType {
        self.body.message_type()
    }

    /// Convenience constructor for LoginRequest.
    pub fn new_login_request(user_id: String, password: String) -> Self {
        let body = ProtoBody::LoginRequest(LoginRequest { user_id, password });
        Self::new(body)
    }

    /// Convenience constructor for NewOrderRequest.
    pub fn new_order_request(
        account_id: String,
        client_order_id: String,
        stock_code: String,
        side: OrderSide,
        order_type: OrderType,
        quantity: i64,
        price: f64,
    ) -> Self {
        let body = ProtoBody::NewOrderRequest(NewOrderRequest {
            account_id,
            client_order_id,
            stock_code,
            side: side as i32,
            r#type: order_type as i32,
            quantity,
            price,
        });
        Self::new(body)
    }

    /// Convenience constructor for Heartbeat.
    pub fn new_heartbeat(client_timestamp_utc: String) -> Self {
        let body = ProtoBody::Heartbeat(Heartbeat { client_timestamp_utc });
        Self::new(body)
    }

    /// Encodes this TradeMessage to bytes (protobuf body only, no header).
    pub fn encode_to_bytes(&self) -> ProtocolResult<Bytes> {
        self.body.encode_to_bytes()
    }

    /// Decodes a TradeMessage from bytes and message type.
    pub fn decode_from_bytes(msg_type: MessageType, data: Bytes) -> ProtocolResult<Self> {
        let body = ProtoBody::decode_from_bytes(msg_type, data)?;
        Ok(ProtoMessage::new(body))
    }
}

// Header size constants
pub const HEADER_SIZE: u32 = 10; // total_length(4) + msg_type(2) + proto_body_length(4)

/// Network wire format message with raw bytes for efficient transmission.
/// Used internally for serialization/deserialization over the network.
#[derive(Debug, Clone, PartialEq)]
pub struct WireMessage {
    /// Total length of the entire message frame (header + protobuf body) in bytes.
    pub total_length: u32,
    /// Message type identifier.
    pub msg_type: MessageType,
    /// Length of the protobuf body in bytes.
    pub proto_body_length: u32,
    /// The serialized protobuf message body.
    pub proto_body: Bytes,
}

impl WireMessage {
    /// Creates a new WireMessage with the given parameters.
    pub fn new(msg_type: MessageType, proto_body: Bytes) -> Self {
        let proto_body_length = proto_body.len() as u32;
        let total_length = HEADER_SIZE + proto_body_length;
        
        Self {
            total_length,
            msg_type,
            proto_body_length,
            proto_body,
        }
    }

    /// Encodes this wire message to bytes for network transmission.
    pub fn encode_to_bytes(&self) -> Bytes {
        let mut buffer = BytesMut::with_capacity(self.total_length as usize);
        
        buffer.put_u32(self.total_length);
        buffer.put_u16(self.msg_type.into());
        buffer.put_u32(self.proto_body_length);
        buffer.put(self.proto_body.clone());

        buffer.freeze()
    }

    /// Decodes a WireMessage from raw bytes.
    pub fn decode_from_bytes(mut data: Bytes) -> ProtocolResult<Self> {
        if data.len() < HEADER_SIZE as usize {
            return Err(ProtocolError::InsufficientHeaderData {
                expected: HEADER_SIZE as usize,
                actual: data.len(),
            });
        }

        let total_length = data.get_u32();
        let msg_type_raw = data.get_u16();
        let proto_body_length = data.get_u32();

        if data.len() < proto_body_length as usize {
            return Err(ProtocolError::InsufficientBodyData {
                expected: proto_body_length,
                actual: data.len(),
            });
        }

        let msg_type = MessageType::try_from(msg_type_raw)?;
        let proto_body = data.slice(0..proto_body_length as usize);

        Ok(WireMessage {
            total_length,
            msg_type,
            proto_body_length,
            proto_body,
        })
    }
}

// Conversion between TradeMessage and WireMessage
impl TryFrom<ProtoMessage> for WireMessage {
    type Error = ProtocolError;

    fn try_from(trade_msg: ProtoMessage) -> Result<Self, Self::Error> {
        let msg_type = trade_msg.message_type();
        let proto_body = trade_msg.body.encode_to_bytes()?;
        
        Ok(WireMessage::new(msg_type, proto_body))
    }
}

impl TryFrom<WireMessage> for ProtoMessage {
    type Error = ProtocolError;

    fn try_from(wire_msg: WireMessage) -> Result<Self, Self::Error> {
        let body = ProtoBody::decode_from_bytes(wire_msg.msg_type, wire_msg.proto_body)?;
        Ok(ProtoMessage::new(body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_login_request_encoding_decoding() {
        // Create a login request message
        let original_msg = ProtoMessage::new_login_request(
            "test_user".to_string(),
            "test_password".to_string()
        );

        // Convert to WireMessage for transmission
        let wire_msg: WireMessage = original_msg.clone().try_into().unwrap();
        let wire_bytes = wire_msg.encode_to_bytes();

        // Decode back to WireMessage then TradeMessage
        let decoded_wire_msg = WireMessage::decode_from_bytes(wire_bytes).unwrap();
        let decoded_msg: ProtoMessage = decoded_wire_msg.try_into().unwrap();

        // Verify the roundtrip
        assert_eq!(original_msg, decoded_msg);
        assert_eq!(original_msg.message_type(), MessageType::LoginRequest);
    }

    #[test]
    fn test_heartbeat_message() {
        let timestamp = "2024-01-01T12:00:00Z".to_string();
        let heartbeat_msg = ProtoMessage::new_heartbeat(timestamp.clone());

        // Convert to WireMessage for transmission
        let wire_msg: WireMessage = heartbeat_msg.clone().try_into().unwrap();
        let wire_bytes = wire_msg.encode_to_bytes();
        
        // Decode back to TradeMessage
        let decoded_wire_msg = WireMessage::decode_from_bytes(wire_bytes).unwrap();
        let decoded_msg: ProtoMessage = decoded_wire_msg.try_into().unwrap();

        assert_eq!(heartbeat_msg, decoded_msg);
        assert_eq!(decoded_msg.message_type(), MessageType::Heartbeat);

        // Extract the heartbeat and verify timestamp
        if let ProtoBody::Heartbeat(heartbeat) = decoded_msg.body {
            assert_eq!(heartbeat.client_timestamp_utc, timestamp);
        } else {
            panic!("Expected heartbeat message");
        }
    }

    #[test]
    fn test_wire_message_conversion() {
        let trade_msg = ProtoMessage::new_login_request(
            "user".to_string(),
            "pass".to_string()
        );

        // Convert to wire message and back
        let wire_msg: WireMessage = trade_msg.clone().try_into().unwrap();
        let converted_back: ProtoMessage = wire_msg.try_into().unwrap();

        assert_eq!(trade_msg, converted_back);
    }

    #[test]
    fn test_message_type_conversion() {
        assert_eq!(MessageType::try_from(1u16).unwrap(), MessageType::LoginRequest);
        assert_eq!(u16::from(MessageType::LoginRequest), 1u16);
        
        assert!(MessageType::try_from(999u16).is_err());
    }

    #[test]
    fn test_wire_message_decode_insufficient_header_data() {
        let data = Bytes::from_static(&[0, 0, 0]); // Not enough for header
        let result = WireMessage::decode_from_bytes(data);
        assert!(matches!(result, Err(ProtocolError::InsufficientHeaderData { .. })));
    }

    #[test]
    fn test_wire_message_decode_insufficient_body_data() {
        let mut full_data_buffer = BytesMut::new();
        full_data_buffer.put_u32(HEADER_SIZE + 5); // total_length (header + 5 body bytes)
        full_data_buffer.put_u16(MessageType::LoginRequest as u16); // msg_type
        full_data_buffer.put_u32(10); // proto_body_length (claims 10 bytes)
        full_data_buffer.put_slice(&[1, 2, 3, 4, 5]); // actual body is only 5 bytes

        let data = full_data_buffer.freeze();
        let result = WireMessage::decode_from_bytes(data);
        
        assert!(matches!(result, Err(ProtocolError::InsufficientBodyData { expected, actual }) if expected == 10 && actual == 5));
    }

    #[test]
    fn test_wire_message_decode_invalid_message_type() {
        let mut buffer = BytesMut::new();
        buffer.put_u32(HEADER_SIZE + 5); // total_length
        buffer.put_u16(999); // invalid msg_type
        buffer.put_u32(5); // proto_body_length
        buffer.put_slice(&[0; 5]); // 5 bytes of body
        
        let data = buffer.freeze();
        let result = WireMessage::decode_from_bytes(data);
        assert!(matches!(result, Err(ProtocolError::InvalidMessageType { .. })));
    }

    #[test]
    fn test_decode_proto_body_unspecified_message_type() {
        let dummy_bytes = Bytes::new();
        let result = ProtoBody::decode_from_bytes(MessageType::Unspecified, dummy_bytes);
        assert!(matches!(result, Err(ProtocolError::UnspecifiedMessageType)));
    }
} 