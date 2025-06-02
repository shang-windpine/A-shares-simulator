// Placeholder for core domain entities
// For example:
// pub struct Account;
// pub struct Stock;
// pub struct Order;
// pub struct Trade;
// pub struct Holding;

// Core domain entities and shared types
use std::sync::Arc;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::fmt::Display;

pub type Timestamp = DateTime<Utc>;

/// 订单方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OrderSide {
    /// 买入
    Buy,
    /// 卖出
    Sell,
}

impl Display for OrderSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderSide::Buy => write!(f, "Buy"),
            OrderSide::Sell => write!(f, "Sell"),
        }
    }
}

/// 订单类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OrderType {
    /// 限价单
    Limit,
    // 未来可扩展：
    // Market,  // 市价单
    // Stop,    // 止损单
}

impl Display for OrderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderType::Limit => write!(f, "Limit"),
        }
    }
}

/// 订单状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OrderStatus {
    /// 新订单
    New,
    /// 部分成交
    PartiallyFilled,
    /// 完全成交
    Filled,
    /// 已取消
    Cancelled,
    /// 被拒绝
    Rejected,
}

impl Display for OrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderStatus::New => write!(f, "New"),
            OrderStatus::PartiallyFilled => write!(f, "PartiallyFilled"),
            OrderStatus::Filled => write!(f, "Filled"),
            OrderStatus::Cancelled => write!(f, "Cancelled"),
            OrderStatus::Rejected => write!(f, "Rejected"),
        }
    }
}

/// 订单结构体
#[derive(Debug, Clone, PartialEq)]
pub struct Order {
    /// 订单ID
    pub order_id: Arc<str>,
    /// 股票代码
    pub stock_id: Arc<str>,
    /// 用户ID
    pub user_id: Arc<str>,
    /// 订单方向
    pub side: OrderSide,
    /// 订单类型
    pub order_type: OrderType,
    /// 价格（限价单）
    pub price: Decimal,
    /// 原始委托数量
    pub quantity: u64,
    /// 未成交数量
    pub unfilled_quantity: u64,
    /// 订单提交时间
    pub timestamp: Timestamp,
    /// 订单状态
    pub status: OrderStatus,
}

impl Order {
    /// 创建新的限价订单
    pub fn new_limit_order(
        order_id: String,
        stock_id: String,
        user_id: String,
        side: OrderSide,
        price: Decimal,
        quantity: u64,
        timestamp: Timestamp,
    ) -> Self {
        Self {
            order_id: order_id.into(),
            stock_id: stock_id.into(),
            user_id: user_id.into(),
            side,
            order_type: OrderType::Limit,
            price,
            quantity,
            unfilled_quantity: quantity,
            timestamp,
            status: OrderStatus::New,
        }
    }

    /// 检查订单是否完全成交
    pub fn is_filled(&self) -> bool {
        self.unfilled_quantity == 0
    }

    /// 检查订单是否部分成交
    pub fn is_partially_filled(&self) -> bool {
        self.unfilled_quantity > 0 && self.unfilled_quantity < self.quantity
    }

    /// 获取已成交数量
    pub fn filled_quantity(&self) -> u64 {
        self.quantity - self.unfilled_quantity
    }
}

/// 订单通知 - 发送给撮合引擎的消息类型
#[derive(Debug)]
pub enum OrderNotification {
    NewOrder(Order),
    CancelOrder { order_id: Arc<str>, stock_id: Arc<str> },
}

/// 用于在订单进入撮合前进行验证
pub trait OrderValidator: Send + Sync {
    /// 验证订单是否有效
    fn validate_order(&self, order: &Order) -> Result<(), String>;

    /// 验证价格是否有效
    fn validate_price(&self, price: Decimal) -> Result<(), String>;

    /// 验证数量是否有效
    fn validate_quantity(&self, quantity: u64) -> Result<(), String>;

    /// 验证股票代码是否有效
    fn validate_stock_id(&self, stock_id: &Arc<str>) -> Result<(), String>;
}

/// 交易信息
#[derive(Debug, Clone)]
pub struct Trade {
    pub id: String,
    pub stock_id: Arc<str>,
    pub price: rust_decimal::Decimal,
    pub quantity: u64,
    pub timestamp: Timestamp,
    pub aggressor_order_id: Arc<str>,
    pub resting_order_id: Arc<str>,
    pub buyer_order_id: Arc<str>,
    pub seller_order_id: Arc<str>,
    pub buyer_user_id: Arc<str>,
    pub seller_user_id: Arc<str>,
}

/// 订单在交易中的状态更新
#[derive(Debug, Clone)]
pub struct OrderStatusInTrade {
    pub order_id: Arc<str>,
    pub filled_quantity_in_trade: u64,  // 本次交易中的成交数量
    pub total_filled_quantity: u64,     // 总成交数量
    pub remaining_quantity: u64,        // 剩余数量
    pub is_fully_filled: bool,          // 是否完全成交
}

/// 综合的交易执行通知
#[derive(Debug, Clone)]
pub struct TradeExecution {
    pub trade: Trade,
    pub buyer_status: OrderStatusInTrade,
    pub seller_status: OrderStatusInTrade,
}

/// 匹配通知 - 从匹配引擎发出的消息类型
#[derive(Debug, Clone)]
pub enum MatchNotification {
    /// 交易执行完成
    TradeExecuted(TradeExecution),
    /// 订单取消确认
    OrderCancelled {
        order_id: Arc<str>,
        stock_id: Arc<str>,
        timestamp: Timestamp,
    },
    /// 订单取消被拒绝
    OrderCancelRejected {
        order_id: Arc<str>,
        stock_id: Arc<str>,
        reason: String,
        timestamp: Timestamp,
    },
}

/// 用于与每个股票任务通信的通知类型
#[derive(Debug, Clone)]
pub enum StockSpecificNotification {
    NewOrder(Order),
    CancelOrder { order_id: Arc<str> },
}

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }

    #[test]
    fn test_trade_creation() {
        let trade = Trade {
            id: "trade_001".to_string(),
            stock_id: "SH600036".into(),
            price: rust_decimal::Decimal::new(1050, 2), // 10.50
            quantity: 100,
            timestamp: Utc::now(),
            aggressor_order_id: "order_001".into(),
            resting_order_id: "order_002".into(),
            buyer_order_id: "order_001".into(),
            seller_order_id: "order_002".into(),
            buyer_user_id: "user_001".into(),
            seller_user_id: "user_002".into(),
        };
        
        assert_eq!(trade.stock_id.as_ref(), "SH600036");
        assert_eq!(trade.quantity, 100);
    }

    #[test]
    fn test_order_creation() {
        let order = Order::new_limit_order(
            "order_001".to_string(),
            "SH600036".to_string(),
            "user_001".to_string(),
            OrderSide::Buy,
            dec!(10.50),
            1000,
            chrono::Utc::now(),
        );

        assert_eq!(order.side, OrderSide::Buy);
        assert_eq!(order.price, dec!(10.50));
        assert_eq!(order.quantity, 1000);
        assert_eq!(order.unfilled_quantity, 1000);
        assert_eq!(order.status, OrderStatus::New);
        assert!(!order.is_filled());
        assert!(!order.is_partially_filled());
        assert_eq!(order.filled_quantity(), 0);
    }

    #[test]
    fn test_order_status_methods() {
        let mut order = Order::new_limit_order(
            "order_001".to_string(),
            "SH600036".to_string(),
            "user_001".to_string(),
            OrderSide::Buy,
            dec!(10.50),
            1000,
            chrono::Utc::now(),
        );

        // 部分成交
        order.unfilled_quantity = 600;
        order.status = OrderStatus::PartiallyFilled;
        assert!(order.is_partially_filled());
        assert!(!order.is_filled());
        assert_eq!(order.filled_quantity(), 400);

        // 完全成交
        order.unfilled_quantity = 0;
        order.status = OrderStatus::Filled;
        assert!(!order.is_partially_filled());
        assert!(order.is_filled());
        assert_eq!(order.filled_quantity(), 1000);
    }

    #[test]
    fn test_order_side_display() {
        assert_eq!(OrderSide::Buy.to_string(), "Buy");
        assert_eq!(OrderSide::Sell.to_string(), "Sell");
    }

    #[test]
    fn test_order_status_display() {
        assert_eq!(OrderStatus::New.to_string(), "New");
        assert_eq!(OrderStatus::PartiallyFilled.to_string(), "PartiallyFilled");
        assert_eq!(OrderStatus::Filled.to_string(), "Filled");
        assert_eq!(OrderStatus::Cancelled.to_string(), "Cancelled");
        assert_eq!(OrderStatus::Rejected.to_string(), "Rejected");
    }
} 