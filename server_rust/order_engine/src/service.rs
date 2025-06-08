//! 订单引擎服务接口定义

use async_trait::async_trait;
use crate::{Order, OrderPoolStats};

/// 订单引擎服务接口
/// 
/// 该接口定义了外部系统（如 message_dispatcher）与订单引擎交互的标准方法。
#[async_trait]
pub trait OrderEngineService: Send + Sync {
    /// 启动订单引擎
    async fn start(&self) -> Result<(), String>;

    /// 停止订单引擎
    async fn stop(&self) -> Result<(), String>;

    /// 检查引擎是否正在运行
    async fn is_running(&self) -> bool;

    /// 提交新订单
    async fn submit_order(&self, order: Order) -> Result<String, String>;

    /// 取消订单
    async fn cancel_order(&self, order_id: &str, stock_id: &str) -> Result<(), String>;

    /// 获取订单信息
    fn get_order(&self, order_id: &str) -> Option<Order>;

    /// 获取用户的所有订单
    fn get_user_orders(&self, user_id: &str) -> Vec<Order>;

    /// 获取股票的所有订单
    fn get_stock_orders(&self, stock_id: &str) -> Vec<Order>;

    /// 获取所有活跃订单
    fn get_active_orders(&self) -> Vec<Order>;

    /// 获取订单池统计信息
    fn get_stats(&self) -> OrderPoolStats;

    /// 健康检查
    async fn health_check(&self) -> bool;
} 