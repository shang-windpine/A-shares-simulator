use crate::errors::*;
use crate::types::*;
use async_trait::async_trait;
use rust_decimal::Decimal;

use order_engine::Order;

/// 撮合引擎核心接口
/// 
/// 定义了撮合引擎的核心功能接口
#[async_trait]
pub trait MatchingEngine: Send + Sync {

    /// 获取订单簿快照
    async fn get_order_book_snapshot(
        &self,
        stock_id: &String,
        depth: usize,
    ) -> MatchingEngineResult<OrderBookView>;

    /// 发送成交回报给订单引擎
    /// 撮合引擎在产生成交后调用此方法，由订单引擎负责更新订单状态和持久化
    async fn process_trade_confirmation(&self, trade: &Trade) -> MatchingEngineResult<()>;

    /// 启动撮合引擎
    async fn start(&mut self) -> MatchingEngineResult<()>;

    /// 停止撮合引擎
    async fn stop(&mut self) -> MatchingEngineResult<()>;

    /// 检查引擎是否运行中
    fn is_running(&self) -> bool;

    fn determine_trade_price(
        &self,
        aggressor_order: &Order,
        resting_order: &Order,
    ) -> MatchingEngineResult<Decimal>;
}