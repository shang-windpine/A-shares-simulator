use crate::{Order, OrderStatus, Timestamp};
use dashmap::DashMap;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};

/// 订单池统计信息
#[derive(Debug, Clone)]
pub struct OrderPoolStats {
    /// 总订单数
    pub total_orders: usize,
    /// 活跃订单数（未完成的订单）
    pub active_orders: usize,
    /// 已成交订单数
    pub filled_orders: usize,
    /// 已取消订单数
    pub cancelled_orders: usize,
    /// 按股票分组的订单数
    pub orders_by_stock: HashMap<String, usize>,
    /// 按用户分组的订单数
    pub orders_by_user: HashMap<String, usize>,
}

/// 高并发、高可用的订单池
/// 使用 DashMap 提供线程安全的并发访问
/// 支持按订单ID、用户ID、股票ID等多种方式查询
#[derive(Debug)]
pub struct OrderPool {
    /// 主索引：订单ID -> 订单
    orders: DashMap<String, Arc<RwLock<Order>>>,
    
    /// 用户索引：用户ID -> 订单ID集合
    user_orders: DashMap<String, Arc<RwLock<Vec<String>>>>,
    
    /// 股票索引：股票ID -> 订单ID集合
    stock_orders: DashMap<String, Arc<RwLock<Vec<String>>>>,
    
    /// 活跃订单索引：只包含未完成的订单
    active_orders: DashMap<String, Arc<RwLock<Order>>>,
    
    /// 统计信息
    stats: Arc<RwLock<OrderPoolStats>>,
}

impl OrderPool {
    /// 创建新的订单池
    pub fn new() -> Self {
        Self {
            orders: DashMap::new(),
            user_orders: DashMap::new(),
            stock_orders: DashMap::new(),
            active_orders: DashMap::new(),
            stats: Arc::new(RwLock::new(OrderPoolStats {
                total_orders: 0,
                active_orders: 0,
                filled_orders: 0,
                cancelled_orders: 0,
                orders_by_stock: HashMap::new(),
                orders_by_user: HashMap::new(),
            })),
        }
    }

    /// 添加新订单到池中
    pub fn add_order(&self, order: Order) -> Result<(), String> {
        let order_id = order.order_id.to_string();
        let user_id = order.user_id.to_string();
        let stock_id = order.stock_id.to_string();
        
        debug!("Adding order {} to pool", order_id);
        
        // 检查订单是否已存在
        if self.orders.contains_key(&order_id) {
            return Err(format!("Order {} already exists in pool", order_id));
        }
        
        let order_arc = Arc::new(RwLock::new(order));
        
        // 添加到主索引
        self.orders.insert(order_id.clone(), order_arc.clone());
        
        // 添加到活跃订单索引（如果是活跃状态）
        if self.is_active_status(order_arc.read().status) {
            self.active_orders.insert(order_id.clone(), order_arc.clone());
        }
        
        // 更新用户索引
        self.user_orders
            .entry(user_id.clone())
            .or_insert_with(|| Arc::new(RwLock::new(Vec::new())))
            .write()
            .push(order_id.clone());
        
        // 更新股票索引
        self.stock_orders
            .entry(stock_id.clone())
            .or_insert_with(|| Arc::new(RwLock::new(Vec::new())))
            .write()
            .push(order_id.clone());
        
        // 更新统计信息
        self.update_stats_on_add(&user_id, &stock_id, order_arc.read().status);
        
        info!("Successfully added order {} to pool", order_id);
        Ok(())
    }

    /// 根据订单ID获取订单
    pub fn get_order(&self, order_id: &str) -> Option<Arc<RwLock<Order>>> {
        self.orders.get(order_id).map(|entry| entry.value().clone())
    }

    /// 更新订单状态和数量
    pub fn update_order(
        &self,
        order_id: &str,
        new_status: OrderStatus,
        filled_quantity: u64,
    ) -> Result<(), String> {
        let order_arc = self.orders
            .get(order_id)
            .ok_or_else(|| format!("Order {} not found in pool", order_id))?
            .value()
            .clone();
        
        let mut order = order_arc.write();
        let old_status = order.status;
        
        // 更新订单状态和数量
        order.status = new_status;
        if filled_quantity > 0 {
            if order.unfilled_quantity >= filled_quantity {
                order.unfilled_quantity -= filled_quantity;
            } else {
                return Err(format!(
                    "Invalid filled quantity {} for order {} with unfilled quantity {}",
                    filled_quantity, order_id, order.unfilled_quantity
                ));
            }
        }
        
        // 更新索引
        self.update_indices_on_status_change(order_id, old_status, new_status);
        
        // 更新统计信息
        self.update_stats_on_status_change(old_status, new_status);
        
        info!(
            "Updated order {} status from {:?} to {:?}, filled_quantity: {}",
            order_id, old_status, new_status, filled_quantity
        );
        
        Ok(())
    }

    /// 取消订单
    pub fn cancel_order(&self, order_id: &str) -> Result<(), String> {
        self.update_order(order_id, OrderStatus::Cancelled, 0)
    }

    /// 根据用户ID获取所有订单
    pub fn get_orders_by_user(&self, user_id: &str) -> Vec<Arc<RwLock<Order>>> {
        if let Some(order_ids) = self.user_orders.get(user_id) {
            order_ids
                .read()
                .iter()
                .filter_map(|order_id| self.orders.get(order_id).map(|entry| entry.value().clone()))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// 根据股票ID获取所有订单
    pub fn get_orders_by_stock(&self, stock_id: &str) -> Vec<Arc<RwLock<Order>>> {
        if let Some(order_ids) = self.stock_orders.get(stock_id) {
            order_ids
                .read()
                .iter()
                .filter_map(|order_id| self.orders.get(order_id).map(|entry| entry.value().clone()))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// 获取所有活跃订单
    pub fn get_active_orders(&self) -> Vec<Arc<RwLock<Order>>> {
        self.active_orders
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// 获取活跃订单（按股票分组）
    pub fn get_active_orders_by_stock(&self, stock_id: &str) -> Vec<Arc<RwLock<Order>>> {
        self.get_orders_by_stock(stock_id)
            .into_iter()
            .filter(|order| self.is_active_status(order.read().status))
            .collect()
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> OrderPoolStats {
        self.stats.read().clone()
    }

    /// 清理已完成的订单（可选的内存管理）
    pub fn cleanup_completed_orders(&self, before_timestamp: Timestamp) -> usize {
        let mut cleaned_count = 0;
        let mut orders_to_remove = Vec::new();
        
        // 找到需要清理的订单
        for entry in self.orders.iter() {
            let order = entry.value().read();
            if (order.status == OrderStatus::Filled || order.status == OrderStatus::Cancelled)
                && order.timestamp < before_timestamp
            {
                orders_to_remove.push(entry.key().clone());
            }
        }
        
        // 移除订单
        for order_id in orders_to_remove {
            if let Some((_, order_arc)) = self.orders.remove(&order_id) {
                let order = order_arc.read();
                self.remove_from_indices(&order_id, &order.user_id, &order.stock_id);
                cleaned_count += 1;
            }
        }
        
        info!("Cleaned up {} completed orders", cleaned_count);
        cleaned_count
    }

    /// 检查订单状态是否为活跃状态
    fn is_active_status(&self, status: OrderStatus) -> bool {
        matches!(status, OrderStatus::New | OrderStatus::PartiallyFilled)
    }

    /// 更新索引（当订单状态变化时）
    fn update_indices_on_status_change(
        &self,
        order_id: &str,
        old_status: OrderStatus,
        new_status: OrderStatus,
    ) {
        let was_active = self.is_active_status(old_status);
        let is_active = self.is_active_status(new_status);
        
        if was_active && !is_active {
            // 从活跃订单中移除
            self.active_orders.remove(order_id);
        } else if !was_active && is_active {
            // 添加到活跃订单
            if let Some(order_arc) = self.orders.get(order_id) {
                self.active_orders.insert(order_id.to_string(), order_arc.value().clone());
            }
        }
    }

    /// 更新统计信息（添加订单时）
    fn update_stats_on_add(&self, user_id: &str, stock_id: &str, status: OrderStatus) {
        let mut stats = self.stats.write();
        stats.total_orders += 1;
        
        if self.is_active_status(status) {
            stats.active_orders += 1;
        } else if status == OrderStatus::Filled {
            stats.filled_orders += 1;
        } else if status == OrderStatus::Cancelled {
            stats.cancelled_orders += 1;
        }
        
        *stats.orders_by_user.entry(user_id.to_string()).or_insert(0) += 1;
        *stats.orders_by_stock.entry(stock_id.to_string()).or_insert(0) += 1;
    }

    /// 更新统计信息（状态变化时）
    fn update_stats_on_status_change(&self, old_status: OrderStatus, new_status: OrderStatus) {
        let mut stats = self.stats.write();
        
        // 更新活跃订单计数
        if self.is_active_status(old_status) && !self.is_active_status(new_status) {
            stats.active_orders = stats.active_orders.saturating_sub(1);
        } else if !self.is_active_status(old_status) && self.is_active_status(new_status) {
            stats.active_orders += 1;
        }
        
        // 更新完成状态计数
        if old_status != OrderStatus::Filled && new_status == OrderStatus::Filled {
            stats.filled_orders += 1;
        }
        if old_status != OrderStatus::Cancelled && new_status == OrderStatus::Cancelled {
            stats.cancelled_orders += 1;
        }
    }

    /// 从索引中移除订单
    fn remove_from_indices(&self, order_id: &str, user_id: &Arc<str>, stock_id: &Arc<str>) {
        // 从用户索引中移除
        if let Some(user_orders) = self.user_orders.get(user_id.as_ref()) {
            user_orders.write().retain(|id| id != order_id);
        }
        
        // 从股票索引中移除
        if let Some(stock_orders) = self.stock_orders.get(stock_id.as_ref()) {
            stock_orders.write().retain(|id| id != order_id);
        }
        
        // 从活跃订单中移除
        self.active_orders.remove(order_id);
    }
}

impl Default for OrderPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OrderSide;
    use rust_decimal_macros::dec;

    #[test]
    fn test_order_pool_basic_operations() {
        let pool = OrderPool::new();
        
        let order = Order::new_limit_order(
            "order_001".to_string(),
            "SH600036".to_string(),
            "user_001".to_string(),
            OrderSide::Buy,
            dec!(10.50),
            1000,
            chrono::Utc::now(),
        );
        
        // 添加订单
        assert!(pool.add_order(order.clone()).is_ok());
        
        // 获取订单
        let retrieved = pool.get_order("order_001").unwrap();
        assert_eq!(retrieved.read().order_id.as_ref(), "order_001");
        
        // 更新订单
        assert!(pool.update_order("order_001", OrderStatus::PartiallyFilled, 500).is_ok());
        assert_eq!(retrieved.read().unfilled_quantity, 500);
        
        // 检查统计信息
        let stats = pool.get_stats();
        assert_eq!(stats.total_orders, 1);
        assert_eq!(stats.active_orders, 1);
    }

    #[test]
    fn test_order_pool_indices() {
        let pool = OrderPool::new();
        
        let order1 = Order::new_limit_order(
            "order_001".to_string(),
            "SH600036".to_string(),
            "user_001".to_string(),
            OrderSide::Buy,
            dec!(10.50),
            1000,
            chrono::Utc::now(),
        );
        
        let order2 = Order::new_limit_order(
            "order_002".to_string(),
            "SH600036".to_string(),
            "user_002".to_string(),
            OrderSide::Sell,
            dec!(10.60),
            500,
            chrono::Utc::now(),
        );
        
        pool.add_order(order1).unwrap();
        pool.add_order(order2).unwrap();
        
        // 按用户查询
        let user1_orders = pool.get_orders_by_user("user_001");
        assert_eq!(user1_orders.len(), 1);
        
        // 按股票查询
        let stock_orders = pool.get_orders_by_stock("SH600036");
        assert_eq!(stock_orders.len(), 2);
        
        // 活跃订单查询
        let active_orders = pool.get_active_orders();
        assert_eq!(active_orders.len(), 2);
    }
} 