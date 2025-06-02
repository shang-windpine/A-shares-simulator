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

pub type Timestamp = DateTime<Utc>;

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

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

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
} 