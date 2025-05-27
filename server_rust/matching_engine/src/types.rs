//! 撮合引擎核心数据类型定义

use rust_decimal::Decimal;
use std::collections::HashMap;
use std::fmt::Display;

/// 股票代码类型
pub type StockId = String;

/// 订单ID类型
pub type OrderId = String;

/// 用户ID类型
pub type UserId = String;

/// 交易ID类型
pub type TradeId = String;

/// 价格类型 - 使用高精度小数
pub type Price = Decimal;

/// 数量类型
pub type Quantity = u64;

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
    pub id: OrderId,
    /// 股票代码
    pub stock_id: StockId,
    /// 用户ID
    pub user_id: UserId,
    /// 订单方向
    pub side: OrderSide,
    /// 订单类型
    pub order_type: OrderType,
    /// 价格（限价单）
    pub price: Price,
    /// 原始委托数量
    pub quantity: Quantity,
    /// 未成交数量
    pub unfilled_quantity: Quantity,
    /// 订单提交时间
    pub timestamp: Timestamp,
    /// 订单状态
    pub status: OrderStatus,
}

impl Order {
    /// 创建新的限价订单
    pub fn new_limit_order(
        id: OrderId,
        stock_id: StockId,
        user_id: UserId,
        side: OrderSide,
        price: Price,
        quantity: Quantity,
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
    pub fn filled_quantity(&self) -> Quantity {
        self.quantity - self.unfilled_quantity
    }
}

/// 交易记录结构体
#[derive(Debug, Clone, PartialEq)]
pub struct Trade {
    /// 交易唯一ID
    pub id: TradeId,
    /// 股票代码
    pub stock_id: StockId,
    /// 成交价格
    pub price: Price,
    /// 成交数量
    pub quantity: Quantity,
    /// 成交时间
    pub timestamp: Timestamp,
    /// 主动方订单ID
    pub aggressor_order_id: OrderId,
    /// 被动方订单ID
    pub resting_order_id: OrderId,
    /// 买方订单ID
    pub buyer_order_id: OrderId,
    /// 卖方订单ID
    pub seller_order_id: OrderId,
    /// 买方用户ID
    pub buyer_user_id: UserId,
    /// 卖方用户ID
    pub seller_user_id: UserId,
}

/// 交易确认结构体（用于订单簿更新）
#[derive(Debug, Clone, PartialEq)]
pub struct TradeConfirmation {
    /// 交易记录
    pub trade: Trade,
    /// 主动方订单剩余数量
    pub aggressor_remaining_quantity: Quantity,
    /// 被动方订单剩余数量
    pub resting_remaining_quantity: Quantity,
}

/// 市场行情tick数据
#[derive(Debug, Clone, PartialEq)]
pub struct MarketTick {
    /// 股票代码
    pub stock_id: StockId,
    /// 最优买一价（即买一价，是买方愿意支付的最高价格）
    pub best_bid_price: Option<Price>,
    /// 最优买一量（即买一量，对应最优买一价的挂单总量）
    pub best_bid_quantity: Option<Quantity>,
    /// 最优卖一价（即卖一价，是卖方愿意接受的最低价格）
    pub best_ask_price: Option<Price>,
    /// 最优卖一量（即卖一量，对应最优卖一价的挂单总量）
    pub best_ask_quantity: Option<Quantity>,
    /// 最新成交价
    pub last_traded_price: Option<Price>,
    /// 时间戳
    pub timestamp: Timestamp,
}

/// 订单簿条目（某个价格层级的聚合信息）
#[derive(Debug, Clone, PartialEq)]
pub struct OrderBookEntry {
    /// 价格
    pub price: Price,
    /// 该价格层级的总数量
    pub total_quantity: Quantity,
    /// 该价格层级的订单数量
    pub order_count: usize,
}

/// 订单簿视图（某个深度的快照）
#[derive(Debug, Clone, PartialEq)]
pub struct OrderBookView {
    /// 股票代码
    pub stock_id: StockId,
    /// 买单簿（按价格从高到低排序）
    pub bids: Vec<OrderBookEntry>,
    /// 卖单簿（按价格从低到高排序）
    pub asks: Vec<OrderBookEntry>,
    /// 快照时间
    pub timestamp: Timestamp,
}

impl OrderBookView {
    /// 创建空的订单簿视图
    pub fn new(stock_id: StockId, timestamp: Timestamp) -> Self {
        Self {
            stock_id,
            bids: Vec::new(),
            asks: Vec::new(),
            timestamp,
        }
    }

    /// 获取最优买价
    pub fn best_bid_price(&self) -> Option<Price> {
        self.bids.first().map(|entry| entry.price)
    }

    /// 获取最优卖价
    pub fn best_ask_price(&self) -> Option<Price> {
        self.asks.first().map(|entry| entry.price)
    }

    /// 获取买卖价差
    pub fn spread(&self) -> Option<Price> {
        match (self.best_bid_price(), self.best_ask_price()) {
            (Some(bid), Some(ask)) => Some(ask - bid),
            _ => None,
        }
    }
}

/// 撮合统计信息
#[derive(Debug, Clone, PartialEq)]
pub struct MatchingStatistics {
    /// 处理的订单总数
    pub total_orders_processed: u64,
    /// 生成的交易总数
    pub total_trades_generated: u64,
    /// 完全成交的订单数
    pub fully_filled_orders: u64,
    /// 部分成交的订单数
    pub partially_filled_orders: u64,
    /// 被取消的订单数
    pub cancelled_orders: u64,
    /// 每个股票的统计信息
    pub per_stock_stats: HashMap<StockId, StockMatchingStats>,
}

/// 单个股票的撮合统计信息
#[derive(Debug, Clone, PartialEq)]
pub struct StockMatchingStats {
    /// 处理的订单数
    pub orders_processed: u64,
    /// 生成的交易数
    pub trades_generated: u64,
    /// 总成交量
    pub total_volume: Quantity,
    /// 总成交额
    pub total_turnover: Decimal,
    /// 最新成交价
    pub last_price: Option<Price>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_stock_id_creation() {
        let stock_id: StockId = "SH600036".to_string();
        assert_eq!(stock_id, "SH600036");
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
    fn test_order_book_view() {
        let stock_id = "SH600036".to_string();
        let timestamp = chrono::Utc::now();
        let mut order_book = OrderBookView::new(stock_id.clone(), timestamp);

        // 添加买单
        order_book.bids.push(OrderBookEntry {
            price: dec!(10.50),
            total_quantity: 1000,
            order_count: 2,
        });
        order_book.bids.push(OrderBookEntry {
            price: dec!(10.49),
            total_quantity: 500,
            order_count: 1,
        });

        // 添加卖单
        order_book.asks.push(OrderBookEntry {
            price: dec!(10.51),
            total_quantity: 800,
            order_count: 3,
        });
        order_book.asks.push(OrderBookEntry {
            price: dec!(10.52),
            total_quantity: 1200,
            order_count: 1,
        });

        assert_eq!(order_book.best_bid_price(), Some(dec!(10.50)));
        assert_eq!(order_book.best_ask_price(), Some(dec!(10.51)));
        assert_eq!(order_book.spread(), Some(dec!(0.01)));
    }

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

    #[test]
    fn test_market_tick() {
        let tick = MarketTick {
            stock_id: "SH600036".to_string(),
            best_bid_price: Some(dec!(10.50)),
            best_bid_quantity: Some(1000),
            best_ask_price: Some(dec!(10.51)),
            best_ask_quantity: Some(800),
            last_traded_price: Some(dec!(10.50)),
            timestamp: chrono::Utc::now(),
        };

        assert_eq!(tick.best_bid_price, Some(dec!(10.50)));
        assert_eq!(tick.best_ask_price, Some(dec!(10.51)));
    }
} 