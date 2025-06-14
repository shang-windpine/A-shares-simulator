// This file is @generated by prost-build.
/// Login
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LoginRequest {
    /// MVP阶段为单一默认虚拟账户，此字段可预留
    #[prost(string, tag = "1")]
    pub user_id: ::prost::alloc::string::String,
    /// 密码，MVP阶段可忽略
    #[prost(string, tag = "2")]
    pub password: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LoginResponse {
    #[prost(bool, tag = "1")]
    pub success: bool,
    /// 例如 "Login successful" 或错误信息
    #[prost(string, tag = "2")]
    pub message: ::prost::alloc::string::String,
    /// 会话ID，MVP阶段可选
    #[prost(string, tag = "3")]
    pub session_id: ::prost::alloc::string::String,
    /// 可选：登录成功后直接返回初始账户信息
    #[prost(message, optional, tag = "4")]
    pub initial_account_info: ::core::option::Option<AccountInfoResponse>,
}
/// New Order
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NewOrderRequest {
    /// 账户ID
    #[prost(string, tag = "1")]
    pub account_id: ::prost::alloc::string::String,
    /// 客户端生成的订单ID，用于跟踪
    #[prost(string, tag = "2")]
    pub client_order_id: ::prost::alloc::string::String,
    /// 股票代码
    #[prost(string, tag = "3")]
    pub stock_code: ::prost::alloc::string::String,
    /// 买卖方向
    #[prost(enumeration = "OrderSide", tag = "4")]
    pub side: i32,
    /// 订单类型 (MVP为限价单)
    #[prost(enumeration = "OrderType", tag = "5")]
    pub r#type: i32,
    /// 数量 (股)
    #[prost(int64, tag = "6")]
    pub quantity: i64,
    /// 价格 (限价单的价格)
    #[prost(double, tag = "7")]
    pub price: f64,
}
/// Order Update (用于订单确认、成交回报、撤单确认、拒绝等)
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OrderUpdateResponse {
    #[prost(string, tag = "1")]
    pub account_id: ::prost::alloc::string::String,
    /// 服务器生成的唯一订单ID
    #[prost(string, tag = "2")]
    pub server_order_id: ::prost::alloc::string::String,
    /// 回显客户端订单ID
    #[prost(string, tag = "3")]
    pub client_order_id: ::prost::alloc::string::String,
    #[prost(string, tag = "4")]
    pub stock_code: ::prost::alloc::string::String,
    #[prost(enumeration = "OrderSide", tag = "5")]
    pub side: i32,
    #[prost(enumeration = "OrderType", tag = "6")]
    pub r#type: i32,
    /// 当前订单状态
    #[prost(enumeration = "OrderStatus", tag = "7")]
    pub status: i32,
    /// 本次事件相关的成交数量 (例如，部分成交时，本次成交了多少)
    #[prost(int64, tag = "8")]
    pub filled_quantity_this_event: i64,
    /// 本次事件相关的成交均价
    #[prost(double, tag = "9")]
    pub avg_filled_price_this_event: f64,
    /// 累计成交数量
    #[prost(int64, tag = "10")]
    pub cumulative_filled_quantity: i64,
    /// 累计成交均价 (如果适用)
    #[prost(double, tag = "11")]
    pub avg_cumulative_filled_price: f64,
    /// 剩余未成交数量
    #[prost(int64, tag = "12")]
    pub leaves_quantity: i64,
    /// 服务器处理此更新的时间戳 (UTC)
    #[prost(string, tag = "13")]
    pub server_timestamp_utc: ::prost::alloc::string::String,
    /// 如果 status 是 REJECTED
    #[prost(enumeration = "RejectionReason", tag = "14")]
    pub rejection_reason: i32,
    /// 拒绝的详细文本信息
    #[prost(string, tag = "15")]
    pub reject_message: ::prost::alloc::string::String,
    /// 本次成交产生的手续费 (如果适用)
    #[prost(double, tag = "16")]
    pub commission: f64,
}
/// Cancel Order
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CancelOrderRequest {
    #[prost(string, tag = "1")]
    pub account_id: ::prost::alloc::string::String,
    /// 要取消的服务器订单ID
    #[prost(string, tag = "2")]
    pub server_order_id_to_cancel: ::prost::alloc::string::String,
}
/// Market Data Subscription
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MarketDataRequest {
    #[prost(enumeration = "market_data_request::SubscriptionAction", tag = "1")]
    pub action: i32,
    /// 需要订阅/取消订阅的股票代码列表
    #[prost(string, repeated, tag = "2")]
    pub stock_codes: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
/// Nested message and enum types in `MarketDataRequest`.
pub mod market_data_request {
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum SubscriptionAction {
        ActionUnspecified = 0,
        Subscribe = 1,
        Unsubscribe = 2,
    }
    impl SubscriptionAction {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Self::ActionUnspecified => "ACTION_UNSPECIFIED",
                Self::Subscribe => "SUBSCRIBE",
                Self::Unsubscribe => "UNSUBSCRIBE",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "ACTION_UNSPECIFIED" => Some(Self::ActionUnspecified),
                "SUBSCRIBE" => Some(Self::Subscribe),
                "UNSUBSCRIBE" => Some(Self::Unsubscribe),
                _ => None,
            }
        }
    }
}
/// Market Data Snapshot (行情快照推送)
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MarketDataSnapshot {
    #[prost(string, tag = "1")]
    pub stock_code: ::prost::alloc::string::String,
    /// 最新价
    #[prost(double, tag = "2")]
    pub last_price: f64,
    /// 买一价
    #[prost(double, tag = "3")]
    pub bid_price_1: f64,
    /// 买一量
    #[prost(int64, tag = "4")]
    pub bid_volume_1: i64,
    /// 卖一价
    #[prost(double, tag = "5")]
    pub ask_price_1: f64,
    /// 卖一量
    #[prost(int64, tag = "6")]
    pub ask_volume_1: i64,
    /// MVP中提到"买卖盘深度等"，如果需要五档，可以扩展 bid_price_2-5, ask_price_2-5 等
    ///
    /// 行情时间戳 (UTC)
    #[prost(string, tag = "7")]
    pub server_timestamp_utc: ::prost::alloc::string::String,
    /// 可选的日内统计数据 (根据MVP沙盒行情生成器决定是否提供)
    ///
    /// 今日开盘价
    #[prost(double, tag = "8")]
    pub open_price: f64,
    /// 今日最高价
    #[prost(double, tag = "9")]
    pub high_price: f64,
    /// 今日最低价
    #[prost(double, tag = "10")]
    pub low_price: f64,
    /// 昨日收盘价 (用于计算涨跌幅)
    #[prost(double, tag = "11")]
    pub prev_close_price: f64,
    /// 总成交量 (当日累计)
    #[prost(int64, tag = "12")]
    pub total_volume: i64,
    /// 总成交额 (当日累计)
    #[prost(double, tag = "13")]
    pub total_turnover: f64,
}
/// Account Information
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountQueryRequest {
    /// 要查询的账户ID
    #[prost(string, tag = "1")]
    pub account_id: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Holding {
    /// 股票代码
    #[prost(string, tag = "1")]
    pub stock_code: ::prost::alloc::string::String,
    /// 股票名称 (方便显示)
    #[prost(string, tag = "2")]
    pub stock_name: ::prost::alloc::string::String,
    /// 持有数量
    #[prost(int64, tag = "3")]
    pub quantity: i64,
    /// 平均持仓成本
    #[prost(double, tag = "4")]
    pub cost_price_avg: f64,
    /// 当前市价 (来自 MarketDataSnapshot)
    #[prost(double, tag = "5")]
    pub last_price: f64,
    /// 持仓市值 (quantity * last_price)
    #[prost(double, tag = "6")]
    pub market_value: f64,
    /// 浮动盈亏
    #[prost(double, tag = "7")]
    pub unrealized_pnl: f64,
    /// 浮动盈亏比例
    #[prost(double, tag = "8")]
    pub unrealized_pnl_ratio: f64,
    /// 可卖数量 (考虑T+1)
    #[prost(int64, tag = "9")]
    pub available_sell_quantity: i64,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccountInfoResponse {
    #[prost(string, tag = "1")]
    pub account_id: ::prost::alloc::string::String,
    /// 总资产 (持仓市值 + 可用资金 + 冻结资金)
    #[prost(double, tag = "2")]
    pub total_assets: f64,
    /// 可用资金
    #[prost(double, tag = "3")]
    pub available_funds: f64,
    /// 冻结资金 (例如挂单未成交占用的资金)
    #[prost(double, tag = "4")]
    pub frozen_funds: f64,
    /// 总持仓市值
    #[prost(double, tag = "5")]
    pub total_market_value: f64,
    /// 持仓列表
    #[prost(message, repeated, tag = "6")]
    pub holdings: ::prost::alloc::vec::Vec<Holding>,
    /// 数据生成时间戳
    #[prost(string, tag = "7")]
    pub server_timestamp_utc: ::prost::alloc::string::String,
    /// 可选的附加信息
    #[prost(string, tag = "8")]
    pub message: ::prost::alloc::string::String,
}
/// Heartbeat
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Heartbeat {
    /// 客户端发送心跳的时间
    #[prost(string, tag = "1")]
    pub client_timestamp_utc: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct HeartbeatResponse {
    /// 服务器响应心跳的时间
    #[prost(string, tag = "1")]
    pub server_timestamp_utc: ::prost::alloc::string::String,
}
/// Generic Error Response (可用于不适合特定响应消息的错误场景)
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ErrorResponse {
    /// 内部错误码
    #[prost(int32, tag = "1")]
    pub error_code: i32,
    /// 错误描述
    #[prost(string, tag = "2")]
    pub error_message: ::prost::alloc::string::String,
    /// 可选，关联到哪个请求出错
    #[prost(string, tag = "3")]
    pub original_request_id: ::prost::alloc::string::String,
}
/// ------------------ General Enums ------------------
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum OrderType {
    Unspecified = 0,
    /// MARKET = 2; // MVP 明确排除复杂市价单
    Limit = 1,
}
impl OrderType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Unspecified => "ORDER_TYPE_UNSPECIFIED",
            Self::Limit => "LIMIT",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "ORDER_TYPE_UNSPECIFIED" => Some(Self::Unspecified),
            "LIMIT" => Some(Self::Limit),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum OrderSide {
    Unspecified = 0,
    Buy = 1,
    Sell = 2,
}
impl OrderSide {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Unspecified => "ORDER_SIDE_UNSPECIFIED",
            Self::Buy => "BUY",
            Self::Sell => "SELL",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "ORDER_SIDE_UNSPECIFIED" => Some(Self::Unspecified),
            "BUY" => Some(Self::Buy),
            "SELL" => Some(Self::Sell),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum OrderStatus {
    Unspecified = 0,
    /// 待报 (已提交到服务器，等待进入订单簿)
    PendingNew = 1,
    /// 已报 (已进入订单簿，等待撮合)
    New = 2,
    /// 部分成交
    PartiallyFilled = 3,
    /// 完全成交
    Filled = 4,
    /// 已撤销
    Canceled = 5,
    /// 已拒绝 (例如 T+1 限制，资金不足等)
    Rejected = 6,
}
impl OrderStatus {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Unspecified => "ORDER_STATUS_UNSPECIFIED",
            Self::PendingNew => "PENDING_NEW",
            Self::New => "NEW",
            Self::PartiallyFilled => "PARTIALLY_FILLED",
            Self::Filled => "FILLED",
            Self::Canceled => "CANCELED",
            Self::Rejected => "REJECTED",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "ORDER_STATUS_UNSPECIFIED" => Some(Self::Unspecified),
            "PENDING_NEW" => Some(Self::PendingNew),
            "NEW" => Some(Self::New),
            "PARTIALLY_FILLED" => Some(Self::PartiallyFilled),
            "FILLED" => Some(Self::Filled),
            "CANCELED" => Some(Self::Canceled),
            "REJECTED" => Some(Self::Rejected),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum RejectionReason {
    ReasonUnspecified = 0,
    InsufficientFunds = 1,
    InsufficientPosition = 2,
    TPlus1Restriction = 3,
    InvalidOrderPrice = 4,
    InvalidOrderQuantity = 5,
    MarketClosed = 6,
    StockNotTradable = 7,
    DuplicateClientOrderId = 8,
    OrderNotFound = 9,
    Other = 99,
}
impl RejectionReason {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::ReasonUnspecified => "REASON_UNSPECIFIED",
            Self::InsufficientFunds => "INSUFFICIENT_FUNDS",
            Self::InsufficientPosition => "INSUFFICIENT_POSITION",
            Self::TPlus1Restriction => "T_PLUS_1_RESTRICTION",
            Self::InvalidOrderPrice => "INVALID_ORDER_PRICE",
            Self::InvalidOrderQuantity => "INVALID_ORDER_QUANTITY",
            Self::MarketClosed => "MARKET_CLOSED",
            Self::StockNotTradable => "STOCK_NOT_TRADABLE",
            Self::DuplicateClientOrderId => "DUPLICATE_CLIENT_ORDER_ID",
            Self::OrderNotFound => "ORDER_NOT_FOUND",
            Self::Other => "OTHER",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "REASON_UNSPECIFIED" => Some(Self::ReasonUnspecified),
            "INSUFFICIENT_FUNDS" => Some(Self::InsufficientFunds),
            "INSUFFICIENT_POSITION" => Some(Self::InsufficientPosition),
            "T_PLUS_1_RESTRICTION" => Some(Self::TPlus1Restriction),
            "INVALID_ORDER_PRICE" => Some(Self::InvalidOrderPrice),
            "INVALID_ORDER_QUANTITY" => Some(Self::InvalidOrderQuantity),
            "MARKET_CLOSED" => Some(Self::MarketClosed),
            "STOCK_NOT_TRADABLE" => Some(Self::StockNotTradable),
            "DUPLICATE_CLIENT_ORDER_ID" => Some(Self::DuplicateClientOrderId),
            "ORDER_NOT_FOUND" => Some(Self::OrderNotFound),
            "OTHER" => Some(Self::Other),
            _ => None,
        }
    }
}
