//! 撮合引擎核心数据类型定义

use rust_decimal::Decimal;
use order_engine::Order;

/// 时间戳类型
pub type Timestamp = chrono::DateTime<chrono::Utc>;

/// 交易记录结构体
#[derive(Debug, Clone, PartialEq)]
pub struct Trade {
    /// 交易唯一ID
    pub id: String,
    /// 股票代码
    pub stock_id: String,
    /// 成交价格
    pub price: Decimal,
    /// 成交数量
    pub quantity: u64,
    /// 成交时间
    pub timestamp: Timestamp,
    /// 主动方订单ID
    pub aggressor_order_id: String,
    /// 被动方订单ID
    pub resting_order_id: String,
    /// 买方订单ID
    pub buyer_order_id: String,
    /// 卖方订单ID
    pub seller_order_id: String,
    /// 买方用户ID
    pub buyer_user_id: String,
    /// 卖方用户ID
    pub seller_user_id: String,
}

/// 交易确认结构体（用于订单簿更新）
#[derive(Debug, Clone, PartialEq)]
pub struct TradeConfirmation {
    /// 交易记录
    pub trade: Trade,
    /// 主动方订单剩余数量
    pub aggressor_remaining_quantity: u64,
    /// 被动方订单剩余数量
    pub resting_remaining_quantity: u64,
}

/// 订单簿条目（某个价格层级的聚合信息）
#[derive(Debug, Clone, PartialEq)]
pub struct OrderBookEntry {
    /// 价格
    pub price: Decimal,
    /// 该价格层级的总数量
    pub total_quantity: u64,
    /// 该价格层级的订单数量
    pub order_count: usize,
    /// 该价格层级的订单队列（按时间排序）
    pub orders: Vec<Order>,
}

/// 订单簿视图（某个深度的快照）
#[derive(Debug, Clone, PartialEq)]
pub struct OrderBookView {
    /// 股票代码
    pub stock_id: String,
    /// 买单簿（按价格从高到低排序）
    pub bids: Vec<OrderBookEntry>,
    /// 卖单簿（按价格从低到高排序）
    pub asks: Vec<OrderBookEntry>,
    /// 快照时间
    pub timestamp: Timestamp,
}

impl OrderBookView {
    /// 创建空的订单簿视图
    pub fn new(stock_id: String, timestamp: Timestamp) -> Self {
        Self {
            stock_id,
            bids: Vec::new(),
            asks: Vec::new(),
            timestamp,
        }
    }

    /// 获取最优买价
    pub fn best_bid_price(&self) -> Option<Decimal> {
        self.bids.first().map(|entry| entry.price)
    }

    /// 获取最优卖价
    pub fn best_ask_price(&self) -> Option<Decimal> {
        self.asks.first().map(|entry| entry.price)
    }

    /// 获取买卖价差
    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_bid_price(), self.best_ask_price()) {
            (Some(bid), Some(ask)) => Some(ask - bid),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_trade_creation() {
        let trade = Trade {
            id: "trade_001".to_string(),
            stock_id: "SH600036".to_string(),
            price: dec!(10.50),
            quantity: 500,
            timestamp: chrono::Utc::now(),
            aggressor_order_id: "order_001".to_string(),
            resting_order_id: "order_002".to_string(),
            buyer_order_id: "order_001".to_string(),
            seller_order_id: "order_002".to_string(),
            buyer_user_id: "user_001".to_string(),
            seller_user_id: "user_002".to_string(),
        };

        assert_eq!(trade.price, dec!(10.50));
        assert_eq!(trade.quantity, 500);
    }

    #[test]
    fn test_order_book_view() {
        let stock_id = "SH600036".to_string();
        let timestamp = chrono::Utc::now();
        let mut order_book = OrderBookView::new(stock_id.clone(), timestamp);

        // 添加买单
        order_book.bids.push(OrderBookEntry {
            price: dec!(10.50),
            total_quantity: 1000,
            order_count: 2,
            orders: Vec::new(),
        });
        order_book.bids.push(OrderBookEntry {
            price: dec!(10.49),
            total_quantity: 500,
            order_count: 1,
            orders: Vec::new(),
        });

        // 添加卖单
        order_book.asks.push(OrderBookEntry {
            price: dec!(10.51),
            total_quantity: 800,
            order_count: 3,
            orders: Vec::new(),
        });
        order_book.asks.push(OrderBookEntry {
            price: dec!(10.52),
            total_quantity: 1200,
            order_count: 1,
            orders: Vec::new(),
        });

        assert_eq!(order_book.best_bid_price(), Some(dec!(10.50)));
        assert_eq!(order_book.best_ask_price(), Some(dec!(10.51)));
        assert_eq!(order_book.spread(), Some(dec!(0.01)));
    }
} 