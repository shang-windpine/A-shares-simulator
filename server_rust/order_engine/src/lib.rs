use rust_decimal::Decimal;
use std::collections::HashMap;
use std::fmt::Display;

/// 时间戳类型
pub type Timestamp = chrono::DateTime<chrono::Utc>;
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
    pub id: String,
    /// 股票代码
    pub stock_id: String,
    /// 用户ID
    pub user_id: String,
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
        id: String,
        stock_id: String,
        user_id: String,
        side: OrderSide,
        price: Decimal,
        quantity: u64,
        timestamp: Timestamp,
    ) -> Self {
        Self {
            id,
            stock_id,
            user_id,
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

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

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

    /// 订单验证器接口
/// 
/// 用于在订单进入撮合前进行验证
pub trait OrderValidator: Send + Sync {
    /// 验证订单是否有效
    fn validate_order(&self, order: &Order) -> Result<(), String>;

    /// 验证价格是否有效
    fn validate_price(&self, price: Decimal) -> Result<(), String>;

    /// 验证数量是否有效
    fn validate_quantity(&self, quantity: u64) -> Result<(), String>;

    /// 验证股票代码是否有效
    fn validate_stock_id(&self, stock_id: &String) -> Result<(), String>;
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