// 导出模块
pub mod order_pool;
pub mod engine;

// 重新导出核心类型
pub use order_pool::{OrderPool, OrderPoolStats};
pub use engine::{OrderEngine, OrderEngineConfig, OrderEngineFactory};

// 从 core_entities 重新导出共享类型
pub use core_entities::{
    MatchNotification, Order, OrderSide, OrderType, OrderStatus, 
    OrderNotification, OrderValidator, Trade, TradeExecution, 
    OrderStatusInTrade, Timestamp
};

#[cfg(test)]
mod tests {
    // 使用 super::* 会自动导入 pub use core_entities::... 中的所有内容
    use super::*;
    use rust_decimal_macros::dec;
    // 不需要显式导入 Order, OrderSide, OrderStatus, Timestamp，因为它们通过 super::* 导入

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