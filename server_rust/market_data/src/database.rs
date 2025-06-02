//! 市场数据数据库操作接口

use std::sync::Arc;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::{MySqlPool, Row};
use tracing::{error, info, instrument};
use thiserror::Error;

use crate::data_types::StaticMarketData;
use core_entities::Timestamp;

/// 数据库配置
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// 数据库连接URL
    pub database_url: String,
    /// 最大连接数
    pub max_connections: u32,
    /// 连接超时时间（秒）
    pub connect_timeout_secs: u64,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            database_url: "mysql://root:password@localhost:3306/a_shares_db".to_string(),
            max_connections: 10,
            connect_timeout_secs: 30,
        }
    }
}

/// 数据库操作错误类型
#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("数据库连接错误: {0}")]
    ConnectionError(#[from] sqlx::Error),
    
    #[error("股票 {stock_id} 在日期 {trade_date} 的静态数据未找到")]
    StaticDataNotFound {
        stock_id: String,
        trade_date: NaiveDate,
    },
    
    #[error("数据解析错误: {0}")]
    DataParseError(String),
    
    #[error("数据库配置错误: {0}")]
    ConfigError(String),
}

/// 市场数据存储接口
#[async_trait::async_trait]
pub trait MarketDataRepository: Send + Sync {
    /// 获取指定日期的单个股票静态数据
    async fn get_static_market_data(
        &self,
        stock_id: &str,
        trade_date: NaiveDate,
    ) -> Result<StaticMarketData, DatabaseError>;

    /// 获取指定日期的多个股票静态数据
    async fn get_multiple_static_market_data(
        &self,
        stock_ids: &[&str],
        trade_date: NaiveDate,
    ) -> Result<Vec<StaticMarketData>, DatabaseError>;

    /// 获取指定日期的全市场静态数据
    async fn get_all_static_market_data(
        &self,
        trade_date: NaiveDate,
    ) -> Result<Vec<StaticMarketData>, DatabaseError>;

    /// 保存或更新静态市场数据
    async fn save_static_market_data(
        &self,
        data: &StaticMarketData,
    ) -> Result<(), DatabaseError>;

    /// 批量保存静态市场数据
    async fn save_multiple_static_market_data(
        &self,
        data_list: &[StaticMarketData],
    ) -> Result<(), DatabaseError>;
}

/// MySQL 数据库实现
pub struct MySqlMarketDataRepository {
    pool: MySqlPool,
}

impl MySqlMarketDataRepository {
    /// 创建新的 MySQL 存储实例
    pub async fn new(config: DatabaseConfig) -> Result<Self, DatabaseError> {
        info!("正在连接MySQL数据库: {}", config.database_url);
        
        let pool = sqlx::mysql::MySqlPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(std::time::Duration::from_secs(config.connect_timeout_secs))
            .connect(&config.database_url)
            .await?;

        info!("成功连接到MySQL数据库");
        
        Ok(Self { pool })
    }

    /// 初始化数据库表
    pub async fn initialize_tables(&self) -> Result<(), DatabaseError> {
        info!("正在初始化市场数据表...");
        
        let create_table_sql = r#"
            CREATE TABLE IF NOT EXISTS market_static_data (
                id BIGINT AUTO_INCREMENT PRIMARY KEY,
                stock_id VARCHAR(20) NOT NULL COMMENT '股票代码',
                trade_date DATE NOT NULL COMMENT '交易日期',
                open_price DECIMAL(12,4) NOT NULL COMMENT '开盘价',
                prev_close_price DECIMAL(12,4) NOT NULL COMMENT '昨日收盘价',
                limit_up_price DECIMAL(12,4) NOT NULL COMMENT '涨停价',
                limit_down_price DECIMAL(12,4) NOT NULL COMMENT '跌停价',
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '更新时间',
                
                UNIQUE KEY uk_stock_date (stock_id, trade_date),
                INDEX idx_trade_date (trade_date),
                INDEX idx_stock_id (stock_id)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='股票静态市场数据表'
        "#;

        sqlx::query(create_table_sql)
            .execute(&self.pool)
            .await?;

        info!("市场数据表初始化完成");
        Ok(())
    }

    /// 获取数据库连接池
    pub fn pool(&self) -> &MySqlPool {
        &self.pool
    }
}

#[async_trait::async_trait]
impl MarketDataRepository for MySqlMarketDataRepository {
    #[instrument(skip(self))]
    async fn get_static_market_data(
        &self,
        stock_id: &str,
        trade_date: NaiveDate,
    ) -> Result<StaticMarketData, DatabaseError> {
        let sql = r#"
            SELECT 
                stock_id, trade_date, open_price, prev_close_price, 
                limit_up_price, limit_down_price, created_at
            FROM market_static_data 
            WHERE stock_id = ? AND trade_date = ?
        "#;

        let row = sqlx::query(sql)
            .bind(stock_id)
            .bind(trade_date)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => {
                let static_data = StaticMarketData {
                    stock_id: row.get::<String, _>("stock_id").into(),
                    trade_date: row.get("trade_date"),
                    open_price: row.get("open_price"),
                    prev_close_price: row.get("prev_close_price"),
                    limit_up_price: row.get("limit_up_price"),
                    limit_down_price: row.get("limit_down_price"),
                    created_at: row.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
                };
                Ok(static_data)
            }
            None => Err(DatabaseError::StaticDataNotFound {
                stock_id: stock_id.to_string(),
                trade_date,
            }),
        }
    }

    #[instrument(skip(self, stock_ids))]
    async fn get_multiple_static_market_data(
        &self,
        stock_ids: &[&str],
        trade_date: NaiveDate,
    ) -> Result<Vec<StaticMarketData>, DatabaseError> {
        if stock_ids.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders = vec!["?"; stock_ids.len()].join(",");
        let sql = format!(
            r#"
            SELECT 
                stock_id, trade_date, open_price, prev_close_price, 
                limit_up_price, limit_down_price, created_at
            FROM market_static_data 
            WHERE stock_id IN ({}) AND trade_date = ?
            ORDER BY stock_id
            "#,
            placeholders
        );

        let mut query = sqlx::query(&sql);
        for stock_id in stock_ids {
            query = query.bind(*stock_id);
        }
        query = query.bind(trade_date);

        let rows = query.fetch_all(&self.pool).await?;

        let mut results = Vec::new();
        for row in rows {
            let static_data = StaticMarketData {
                stock_id: row.get::<String, _>("stock_id").into(),
                trade_date: row.get("trade_date"),
                open_price: row.get("open_price"),
                prev_close_price: row.get("prev_close_price"),
                limit_up_price: row.get("limit_up_price"),
                limit_down_price: row.get("limit_down_price"),
                created_at: row.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
            };
            results.push(static_data);
        }

        Ok(results)
    }

    #[instrument(skip(self))]
    async fn get_all_static_market_data(
        &self,
        trade_date: NaiveDate,
    ) -> Result<Vec<StaticMarketData>, DatabaseError> {
        let sql = r#"
            SELECT 
                stock_id, trade_date, open_price, prev_close_price, 
                limit_up_price, limit_down_price, created_at
            FROM market_static_data 
            WHERE trade_date = ?
            ORDER BY stock_id
        "#;

        let rows = sqlx::query(sql)
            .bind(trade_date)
            .fetch_all(&self.pool)
            .await?;

        let mut results = Vec::new();
        for row in rows {
            let static_data = StaticMarketData {
                stock_id: row.get::<String, _>("stock_id").into(),
                trade_date: row.get("trade_date"),
                open_price: row.get("open_price"),
                prev_close_price: row.get("prev_close_price"),
                limit_up_price: row.get("limit_up_price"),
                limit_down_price: row.get("limit_down_price"),
                created_at: row.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
            };
            results.push(static_data);
        }

        info!("加载了 {} 只股票的静态行情数据", results.len());
        Ok(results)
    }

    #[instrument(skip(self, data))]
    async fn save_static_market_data(
        &self,
        data: &StaticMarketData,
    ) -> Result<(), DatabaseError> {
        let sql = r#"
            INSERT INTO market_static_data 
                (stock_id, trade_date, open_price, prev_close_price, limit_up_price, limit_down_price, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON DUPLICATE KEY UPDATE
                open_price = VALUES(open_price),
                prev_close_price = VALUES(prev_close_price),
                limit_up_price = VALUES(limit_up_price),
                limit_down_price = VALUES(limit_down_price),
                updated_at = CURRENT_TIMESTAMP
        "#;

        sqlx::query(sql)
            .bind(data.stock_id.as_ref())
            .bind(data.trade_date)
            .bind(data.open_price)
            .bind(data.prev_close_price)
            .bind(data.limit_up_price)
            .bind(data.limit_down_price)
            .bind(data.created_at)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    #[instrument(skip(self, data_list))]
    async fn save_multiple_static_market_data(
        &self,
        data_list: &[StaticMarketData],
    ) -> Result<(), DatabaseError> {
        if data_list.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;

        let sql = r#"
            INSERT INTO market_static_data 
                (stock_id, trade_date, open_price, prev_close_price, limit_up_price, limit_down_price, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON DUPLICATE KEY UPDATE
                open_price = VALUES(open_price),
                prev_close_price = VALUES(prev_close_price),
                limit_up_price = VALUES(limit_up_price),
                limit_down_price = VALUES(limit_down_price),
                updated_at = CURRENT_TIMESTAMP
        "#;

        for data in data_list {
            sqlx::query(sql)
                .bind(data.stock_id.as_ref())
                .bind(data.trade_date)
                .bind(data.open_price)
                .bind(data.prev_close_price)
                .bind(data.limit_up_price)
                .bind(data.limit_down_price)
                .bind(data.created_at)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;
        info!("批量保存了 {} 条静态市场数据", data_list.len());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use chrono::{NaiveDate, Utc};

    // 注意: 这些测试需要实际的数据库连接，通常在集成测试中运行
    
    #[tokio::test]
    #[ignore] // 默认忽略，需要数据库时手动运行
    async fn test_database_operations() {
        let config = DatabaseConfig::default();
        let repo = MySqlMarketDataRepository::new(config).await.unwrap();
        
        // 初始化表
        repo.initialize_tables().await.unwrap();
        
        // 创建测试数据
        let test_data = StaticMarketData {
            stock_id: "TEST001".into(),
            trade_date: NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            open_price: dec!(10.00),
            prev_close_price: dec!(9.50),
            limit_up_price: dec!(10.45),
            limit_down_price: dec!(8.55),
            created_at: Utc::now(),
        };
        
        // 保存数据
        repo.save_static_market_data(&test_data).await.unwrap();
        
        // 获取数据
        let retrieved = repo
            .get_static_market_data("TEST001", test_data.trade_date)
            .await
            .unwrap();
        
        assert_eq!(retrieved.stock_id.as_ref(), "TEST001");
        assert_eq!(retrieved.open_price, dec!(10.00));
        assert_eq!(retrieved.prev_close_price, dec!(9.50));
    }
} 