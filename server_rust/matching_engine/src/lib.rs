//! # 撮合引擎 (Matching Engine)
//! 
//! 本模块实现了A股模拟交易软件的核心撮合引擎，负责订单匹配和交易生成。

pub mod engine;
pub mod errors;

// 重新导出共享类型
pub use core_entities::{
    MatchNotification, Trade, TradeExecution, OrderStatusInTrade, Timestamp,
    Order, OrderSide, OrderStatus, OrderType, OrderNotification, 
    OrderValidator, StockSpecificNotification
};

// Initialize tracing subscriber for the library
pub fn init_tracing() {
    // 使用简单的格式化输出，如果需要更复杂的配置，用户可以自己初始化
    tracing_subscriber::fmt::init();
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
} 