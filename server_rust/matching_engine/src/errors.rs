//! 撮合引擎错误类型定义

use thiserror::Error;

/// 撮合引擎相关的错误类型
#[derive(Error, Debug, Clone, PartialEq)]
pub enum MatchingEngineError {
    #[error("订单无效: {reason}")]
    InvalidOrder { reason: String },
    
    #[error("订单不存在: {order_id}")]
    OrderNotFound { order_id: String },
    
    #[error("股票代码无效: {stock_id}")]
    InvalidStockId { stock_id: String },
    
    #[error("撮合逻辑错误: {reason}")]
    MatchingLogicError { reason: String },
    
    #[error("订单簿操作失败: {reason}")]
    OrderBookOperationFailed { reason: String },
    
    #[error("存储错误: {reason}")]
    StorageError { reason: String },
    
    #[error("观察者通知失败: {reason}")]
    ObserverNotificationFailed { reason: String },
    
    #[error("内部错误: {reason}")]
    InternalError { reason: String },
}

/// 订单簿存储相关的错误类型
#[derive(Error, Debug, Clone, PartialEq)]
pub enum OrderBookStoreError {
    #[error("连接失败: {reason}")]
    ConnectionFailed { reason: String },
    
    #[error("序列化/反序列化失败: {reason}")]
    SerializationError { reason: String },
    
    #[error("数据不存在: {key}")]
    DataNotFound { key: String },
    
    #[error("数据格式错误: {reason}")]
    InvalidDataFormat { reason: String },
    
    #[error("操作超时")]
    Timeout,
    
    #[error("存储容量不足")]
    InsufficientCapacity,
    
    #[error("未知存储错误: {reason}")]
    Unknown { reason: String },
}

/// 撮合生命周期观察者错误类型
#[derive(Error, Debug, Clone, PartialEq)]
pub enum MatchLifecycleObserverError {
    #[error("回调执行失败: {reason}")]
    CallbackExecutionFailed { reason: String },
    
    #[error("通知服务不可用: {service}")]
    NotificationServiceUnavailable { service: String },
    
    #[error("数据验证失败: {reason}")]
    DataValidationFailed { reason: String },
    
    #[error("外部服务调用失败: {service}: {reason}")]
    ExternalServiceCallFailed { service: String, reason: String },
}

/// 通用的撮合引擎结果类型
pub type MatchingEngineResult<T> = Result<T, MatchingEngineError>;

/// 订单簿存储结果类型
pub type OrderBookStoreResult<T> = Result<T, OrderBookStoreError>;

/// 观察者结果类型
pub type MatchLifecycleObserverResult<T> = Result<T, MatchLifecycleObserverError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matching_engine_error_display() {
        let error = MatchingEngineError::InvalidOrder {
            reason: "价格不能为负数".to_string(),
        };
        assert_eq!(error.to_string(), "订单无效: 价格不能为负数");
    }

    #[test]
    fn test_orderbook_store_error_display() {
        let error = OrderBookStoreError::ConnectionFailed {
            reason: "Redis连接超时".to_string(),
        };
        assert_eq!(error.to_string(), "连接失败: Redis连接超时");
    }

    #[test]
    fn test_observer_error_display() {
        let error = MatchLifecycleObserverError::CallbackExecutionFailed {
            reason: "网络超时".to_string(),
        };
        assert_eq!(error.to_string(), "回调执行失败: 网络超时");
    }

    #[test]
    fn test_error_equality() {
        let error1 = MatchingEngineError::OrderNotFound {
            order_id: "12345".to_string(),
        };
        let error2 = MatchingEngineError::OrderNotFound {
            order_id: "12345".to_string(),
        };
        assert_eq!(error1, error2);
    }
} 