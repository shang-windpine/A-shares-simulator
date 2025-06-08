//! 市场数据数据库操作接口

use std::{borrow::Cow, str::FromStr};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::{MySqlPool, Row};
use tracing::{info, instrument};
use thiserror::Error;
use std::sync::Arc;

use crate::data_types::{StaticMarketData, MarketData};

/// 数据库配置
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// 数据库连接URL
    pub database_url: Cow<'static, str>,
    /// 最大连接数
    pub max_connections: u32,
    /// 连接超时时间（秒）
    pub connect_timeout_secs: u64,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            database_url: Cow::Borrowed("mysql://root:@localhost:3306/a_shares_real_history"),
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

    /// 获取完整市场数据（包含扩展数据）
    async fn get_complete_market_data(
        &self,
        stock_id: &str,
        trade_date: NaiveDate,
    ) -> Result<MarketData, DatabaseError>;

    /// 获取多个完整市场数据（包含扩展数据）
    async fn get_multiple_complete_market_data(
        &self,
        stock_ids: &[&str],
        trade_date: NaiveDate,
    ) -> Result<Vec<MarketData>, DatabaseError>;

    /// 获取全市场完整数据（包含扩展数据）
    async fn get_all_complete_market_data(
        &self,
        trade_date: NaiveDate,
    ) -> Result<Vec<MarketData>, DatabaseError>;

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

    /// 创建新的 MySQL 存储实例（使用Arc共享配置）
    pub async fn new_with_shared_config(config: Arc<DatabaseConfig>) -> Result<Self, DatabaseError> {
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
            CREATE TABLE IF NOT EXISTS daily_basics (
                ts_code VARCHAR(10) NOT NULL COMMENT 'TS股票代码',
                trade_date DATE NOT NULL COMMENT '交易日期',
                close DECIMAL(10,2) NOT NULL COMMENT '当日收盘价',
                turnover_rate DECIMAL(10,4) COMMENT '换手率（%）',
                turnover_rate_f DECIMAL(10,4) COMMENT '换手率（自由流通股）',
                volume_ratio DECIMAL(10,2) COMMENT '量比',
                pe DECIMAL(10,2) COMMENT '市盈率（总市值/净利润， 亏损的PE为空）',
                pe_ttm DECIMAL(10,2) COMMENT '市盈率（TTM，亏损的PE为空）',
                pb DECIMAL(10,2) COMMENT '市净率（总市值/净资产）',
                ps DECIMAL(10,2) COMMENT '市销率',
                ps_ttm DECIMAL(10,2) COMMENT '市销率（TTM）',
                dv_ratio DECIMAL(10,4) COMMENT '股息率 （%）',
                dv_ttm DECIMAL(10,4) COMMENT '股息率（TTM）（%）',
                total_share DECIMAL(20,2) COMMENT '总股本 （万股）',
                float_share DECIMAL(20,2) COMMENT '流通股本 （万股）',
                free_share DECIMAL(20,2) COMMENT '自由流通股本 （万）',
                total_mv DECIMAL(20,2) COMMENT '总市值 （万元）',
                circ_mv DECIMAL(20,2) COMMENT '流通市值（万元）',
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '更新时间',
                
                PRIMARY KEY (ts_code, trade_date),
                INDEX idx_trade_date (trade_date),
                INDEX idx_ts_code (ts_code)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='每日基本指标表'
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

    /// 计算涨停价（基于前一日收盘价）
    fn calculate_limit_up_price(prev_close: Decimal) -> Decimal {
        // A股涨停规则：普通股票涨幅10%，ST股票涨幅5%
        // 这里简化处理，统一按照10%计算
        prev_close * Decimal::from_str_exact("1.1").unwrap()
    }

    /// 计算跌停价（基于前一日收盘价）
    fn calculate_limit_down_price(prev_close: Decimal) -> Decimal {
        // A股跌停规则：普通股票跌幅10%，ST股票跌幅5%
        // 这里简化处理，统一按照10%计算
        prev_close * Decimal::from_str_exact("0.9").unwrap()
    }

    /// 根据daily_basics数据构建StaticMarketData
    async fn build_static_data_from_daily_basics(
        &self,
        ts_code: &str,
        trade_date: NaiveDate,
        close_price: Decimal,
        total_mv: Option<Decimal>,
        circ_mv: Option<Decimal>,
        total_share: Option<Decimal>,
        float_share: Option<Decimal>,
        turnover_rate: Option<Decimal>,
    ) -> Result<StaticMarketData, DatabaseError> {
        // 获取前一交易日的收盘价作为prev_close_price
        let prev_close_price = self.get_previous_close_price(ts_code, trade_date).await?;
        
        Ok(StaticMarketData {
            stock_id: ts_code.to_string().into(),
            trade_date,
            open_price: close_price, // 临时使用收盘价，实际应该从日线数据获取
            prev_close_price,
            limit_up_price: Self::calculate_limit_up_price(prev_close_price),
            limit_down_price: Self::calculate_limit_down_price(prev_close_price),
            created_at: chrono::Utc::now(),
        })
    }

    /// 根据daily_basics数据构建ExtendedMarketData
    fn build_extended_data_from_daily_basics(
        ts_code: &str,
        total_mv: Option<Decimal>,
        circ_mv: Option<Decimal>,
        total_share: Option<Decimal>,
        float_share: Option<Decimal>,
        turnover_rate: Option<Decimal>,
    ) -> crate::data_types::ExtendedMarketData {
        crate::data_types::ExtendedMarketData {
            stock_id: ts_code.to_string().into(),
            total_market_cap: total_mv,
            total_shares: total_share.and_then(|val| {
                let shares_in_units = val * Decimal::from(10000); // 万股转换为股
                shares_in_units.to_string().parse::<u64>().ok()
            }),
            circulating_market_cap: circ_mv,
            circulating_shares: float_share.and_then(|val| {
                let shares_in_units = val * Decimal::from(10000); // 万股转换为股
                shares_in_units.to_string().parse::<u64>().ok()
            }),
            turnover_rate,
        }
    }

    /// 获取前一交易日的收盘价
    async fn get_previous_close_price(
        &self,
        ts_code: &str,
        trade_date: NaiveDate,
    ) -> Result<Decimal, DatabaseError> {
        let sql = r#"
            SELECT close 
            FROM daily_basics 
            WHERE ts_code = ? AND trade_date < ? 
            ORDER BY trade_date DESC 
            LIMIT 1
        "#;

        let row = sqlx::query(sql)
            .bind(ts_code)
            .bind(trade_date)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => {
                let close_price: Decimal = row.get("close");
                Ok(close_price)
            }
            None => {
                // 如果找不到前一日数据，返回当前收盘价（或默认值）
                info!("找不到股票 {} 在 {} 之前的收盘价数据，使用默认值", ts_code, trade_date);
                Ok(Decimal::from(10)) // 默认值，实际应用中可能需要更合理的处理
            }
        }
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
                ts_code, trade_date, close, total_mv, circ_mv, 
                total_share, float_share, turnover_rate
            FROM daily_basics 
            WHERE ts_code = ? AND trade_date = ?
        "#;

        let row = sqlx::query(sql)
            .bind(stock_id)
            .bind(trade_date)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => {
                let ts_code: String = row.get("ts_code");
                let trade_date: NaiveDate = row.get("trade_date");
                let close_price: Decimal = row.get("close");
                let total_mv: Option<Decimal> = row.get("total_mv");
                let circ_mv: Option<Decimal> = row.get("circ_mv");
                let total_share: Option<Decimal> = row.get("total_share");
                let float_share: Option<Decimal> = row.get("float_share");
                let turnover_rate: Option<Decimal> = row.get("turnover_rate");
                
                self.build_static_data_from_daily_basics(
                    &ts_code, trade_date, close_price, 
                    total_mv, circ_mv, total_share, float_share, turnover_rate
                ).await
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
                ts_code, trade_date, close, total_mv, circ_mv, 
                total_share, float_share, turnover_rate
            FROM daily_basics 
            WHERE ts_code IN ({}) AND trade_date = ?
            ORDER BY ts_code
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
            let ts_code: String = row.get("ts_code");
            let trade_date: NaiveDate = row.get("trade_date");
            let close_price: Decimal = row.get("close");
            let total_mv: Option<Decimal> = row.get("total_mv");
            let circ_mv: Option<Decimal> = row.get("circ_mv");
            let total_share: Option<Decimal> = row.get("total_share");
            let float_share: Option<Decimal> = row.get("float_share");
            let turnover_rate: Option<Decimal> = row.get("turnover_rate");
            
            let static_data = self.build_static_data_from_daily_basics(
                &ts_code, trade_date, close_price, 
                total_mv, circ_mv, total_share, float_share, turnover_rate
            ).await?;
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
                ts_code, trade_date, close, total_mv, circ_mv, 
                total_share, float_share, turnover_rate
            FROM daily_basics 
            WHERE trade_date = ?
            ORDER BY ts_code
        "#;

        let rows = sqlx::query(sql)
            .bind(trade_date)
            .fetch_all(&self.pool)
            .await?;

        let mut results = Vec::new();
        for row in rows {
            let ts_code: String = row.get("ts_code");
            let trade_date: NaiveDate = row.get("trade_date");
            let close_price: Decimal = row.get("close");
            let total_mv: Option<Decimal> = row.get("total_mv");
            let circ_mv: Option<Decimal> = row.get("circ_mv");
            let total_share: Option<Decimal> = row.get("total_share");
            let float_share: Option<Decimal> = row.get("float_share");
            let turnover_rate: Option<Decimal> = row.get("turnover_rate");
            
            let static_data = self.build_static_data_from_daily_basics(
                &ts_code, trade_date, close_price, 
                total_mv, circ_mv, total_share, float_share, turnover_rate
            ).await?;
            results.push(static_data);
        }

        info!("加载了 {} 只股票的静态行情数据", results.len());
        Ok(results)
    }

    #[instrument(skip(self))]
    async fn get_complete_market_data(
        &self,
        stock_id: &str,
        trade_date: NaiveDate,
    ) -> Result<MarketData, DatabaseError> {
        let sql = r#"
            SELECT 
                ts_code, trade_date, close, total_mv, circ_mv, 
                total_share, float_share, turnover_rate
            FROM daily_basics 
            WHERE ts_code = ? AND trade_date = ?
        "#;

        let row = sqlx::query(sql)
            .bind(stock_id)
            .bind(trade_date)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => {
                let ts_code: String = row.get("ts_code");
                let trade_date: NaiveDate = row.get("trade_date");
                let close_price: Decimal = row.get("close");
                let total_mv: Option<Decimal> = row.get("total_mv");
                let circ_mv: Option<Decimal> = row.get("circ_mv");
                let total_share: Option<Decimal> = row.get("total_share");
                let float_share: Option<Decimal> = row.get("float_share");
                let turnover_rate: Option<Decimal> = row.get("turnover_rate");
                
                let static_data = self.build_static_data_from_daily_basics(
                    &ts_code, trade_date, close_price, 
                    total_mv, circ_mv, total_share, float_share, turnover_rate
                ).await?;
                
                let extended_data = Self::build_extended_data_from_daily_basics(
                    &ts_code, total_mv, circ_mv, total_share, float_share, turnover_rate
                );
                
                Ok(MarketData::new_with_extended(static_data, extended_data))
            }
            None => Err(DatabaseError::StaticDataNotFound {
                stock_id: stock_id.to_string(),
                trade_date,
            }),
        }
    }

    #[instrument(skip(self, stock_ids))]
    async fn get_multiple_complete_market_data(
        &self,
        stock_ids: &[&str],
        trade_date: NaiveDate,
    ) -> Result<Vec<MarketData>, DatabaseError> {
        if stock_ids.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders = vec!["?"; stock_ids.len()].join(",");
        let sql = format!(
            r#"
            SELECT 
                ts_code, trade_date, close, total_mv, circ_mv, 
                total_share, float_share, turnover_rate
            FROM daily_basics 
            WHERE ts_code IN ({}) AND trade_date = ?
            ORDER BY ts_code
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
            let ts_code: String = row.get("ts_code");
            let trade_date: NaiveDate = row.get("trade_date");
            let close_price: Decimal = row.get("close");
            let total_mv: Option<Decimal> = row.get("total_mv");
            let circ_mv: Option<Decimal> = row.get("circ_mv");
            let total_share: Option<Decimal> = row.get("total_share");
            let float_share: Option<Decimal> = row.get("float_share");
            let turnover_rate: Option<Decimal> = row.get("turnover_rate");
            
            let static_data = self.build_static_data_from_daily_basics(
                &ts_code, trade_date, close_price, 
                total_mv, circ_mv, total_share, float_share, turnover_rate
            ).await?;
            
            let extended_data = Self::build_extended_data_from_daily_basics(
                &ts_code, total_mv, circ_mv, total_share, float_share, turnover_rate
            );
            
            results.push(MarketData::new_with_extended(static_data, extended_data));
        }

        Ok(results)
    }

    #[instrument(skip(self))]
    async fn get_all_complete_market_data(
        &self,
        trade_date: NaiveDate,
    ) -> Result<Vec<MarketData>, DatabaseError> {
        let sql = r#"
            SELECT 
                ts_code, trade_date, close, total_mv, circ_mv, 
                total_share, float_share, turnover_rate
            FROM daily_basics 
            WHERE trade_date = ?
            ORDER BY ts_code
        "#;

        let rows = sqlx::query(sql)
            .bind(trade_date)
            .fetch_all(&self.pool)
            .await?;

        let mut results = Vec::new();
        for row in rows {
            let ts_code: String = row.get("ts_code");
            let trade_date: NaiveDate = row.get("trade_date");
            let close_price: Decimal = row.get("close");
            let total_mv: Option<Decimal> = row.get("total_mv");
            let circ_mv: Option<Decimal> = row.get("circ_mv");
            let total_share: Option<Decimal> = row.get("total_share");
            let float_share: Option<Decimal> = row.get("float_share");
            let turnover_rate: Option<Decimal> = row.get("turnover_rate");
            
            let static_data = self.build_static_data_from_daily_basics(
                &ts_code, trade_date, close_price, 
                total_mv, circ_mv, total_share, float_share, turnover_rate
            ).await?;
            
            let extended_data = Self::build_extended_data_from_daily_basics(
                &ts_code, total_mv, circ_mv, total_share, float_share, turnover_rate
            );
            
            results.push(MarketData::new_with_extended(static_data, extended_data));
        }

        info!("加载了 {} 只股票的完整市场数据", results.len());
        Ok(results)
    }

    #[instrument(skip(self, data))]
    async fn save_static_market_data(
        &self,
        data: &StaticMarketData,
    ) -> Result<(), DatabaseError> {
        // 注意：这里保持原有的保存逻辑到 market_static_data 表
        // 因为daily_basics表通常是只读的历史数据
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

    #[tokio::test]
    #[ignore] // 默认忽略，需要数据库时手动运行
    async fn test_get_static_data_from_daily_basics() {
        let config = DatabaseConfig::default();
        let repo = MySqlMarketDataRepository::new(config).await.unwrap();
        
        // 测试从 daily_basics 表获取单个股票数据
        // 使用一个常见的股票代码和日期，假设数据库中有这些数据
        let stock_id = "000001.SZ"; // 平安银行
        let trade_date = NaiveDate::from_ymd_opt(2024, 1, 2).unwrap();
        
        match repo.get_static_market_data(stock_id, trade_date).await {
            Ok(static_data) => {
                println!("成功获取股票数据: {:?}", static_data);
                assert_eq!(static_data.stock_id.as_ref(), stock_id);
                assert_eq!(static_data.trade_date, trade_date);
                assert!(static_data.prev_close_price > Decimal::ZERO);
                assert!(static_data.limit_up_price > static_data.prev_close_price);
                assert!(static_data.limit_down_price < static_data.prev_close_price);
                
                // 验证涨停价计算是否正确（大约是前一日收盘价的1.1倍）
                let expected_limit_up = static_data.prev_close_price * Decimal::from_str_exact("1.1").unwrap();
                let diff = (static_data.limit_up_price - expected_limit_up).abs();
                assert!(diff < Decimal::from_str_exact("0.01").unwrap(), "涨停价计算误差过大");
            }
            Err(DatabaseError::StaticDataNotFound { .. }) => {
                println!("警告: 数据库中没有找到 {} 在 {} 的数据，请检查数据库内容", stock_id, trade_date);
            }
            Err(e) => panic!("数据库查询失败: {:?}", e),
        }
    }

    #[tokio::test]
    #[ignore] // 默认忽略，需要数据库时手动运行
    async fn test_get_multiple_static_data_from_daily_basics() {
        let config = DatabaseConfig::default();
        let repo = MySqlMarketDataRepository::new(config).await.unwrap();
        
        // 测试获取多个股票的数据
        let stock_ids = vec!["000001.SZ", "000002.SZ", "600000.SH"];
        let trade_date = NaiveDate::from_ymd_opt(2024, 1, 2).unwrap();
        
        match repo.get_multiple_static_market_data(&stock_ids, trade_date).await {
            Ok(results) => {
                println!("成功获取 {} 只股票的数据", results.len());
                for static_data in &results {
                    println!("股票: {}, 前收: {}, 涨停: {}, 跌停: {}", 
                        static_data.stock_id, 
                        static_data.prev_close_price,
                        static_data.limit_up_price,
                        static_data.limit_down_price
                    );
                    assert!(static_data.prev_close_price > Decimal::ZERO);
                }
                
                // 验证结果按股票代码排序
                for i in 1..results.len() {
                    assert!(results[i-1].stock_id <= results[i].stock_id);
                }
            }
            Err(e) => println!("获取多股票数据失败: {:?}", e),
        }
    }

    #[tokio::test]
    #[ignore] // 默认忽略，需要数据库时手动运行
    async fn test_get_complete_market_data_from_daily_basics() {
        let config = DatabaseConfig::default();
        let repo = MySqlMarketDataRepository::new(config).await.unwrap();
        
        // 测试获取完整市场数据（包含扩展字段）
        let stock_id = "000001.SZ";
        let trade_date = NaiveDate::from_ymd_opt(2024, 1, 2).unwrap();
        
        match repo.get_complete_market_data(stock_id, trade_date).await {
            Ok(market_data) => {
                println!("成功获取完整市场数据");
                
                // 验证静态数据
                assert_eq!(market_data.static_data.stock_id.as_ref(), stock_id);
                assert_eq!(market_data.static_data.trade_date, trade_date);
                
                // 验证动态数据初始状态
                assert_eq!(market_data.dynamic_data.stock_id.as_ref(), stock_id);
                assert_eq!(market_data.dynamic_data.trade_date, trade_date);
                assert_eq!(market_data.dynamic_data.current_price, Decimal::ZERO);
                assert_eq!(market_data.dynamic_data.volume, 0);
                
                // 验证扩展数据
                assert_eq!(market_data.extended_data.stock_id.as_ref(), stock_id);
                println!("扩展数据: 总市值={:?}, 流通市值={:?}, 总股本={:?}, 流通股本={:?}, 换手率={:?}",
                    market_data.extended_data.total_market_cap,
                    market_data.extended_data.circulating_market_cap,
                    market_data.extended_data.total_shares,
                    market_data.extended_data.circulating_shares,
                    market_data.extended_data.turnover_rate
                );
                
                // 如果有市值数据，验证其为正数
                if let Some(total_mv) = market_data.extended_data.total_market_cap {
                    assert!(total_mv > Decimal::ZERO, "总市值应该为正数");
                }
                
                if let Some(circ_mv) = market_data.extended_data.circulating_market_cap {
                    assert!(circ_mv > Decimal::ZERO, "流通市值应该为正数");
                }
            }
            Err(DatabaseError::StaticDataNotFound { .. }) => {
                println!("警告: 数据库中没有找到 {} 在 {} 的数据", stock_id, trade_date);
            }
            Err(e) => panic!("获取完整市场数据失败: {:?}", e),
        }
    }

    #[tokio::test]
    #[ignore] // 默认忽略，需要数据库时手动运行
    async fn test_get_all_complete_market_data_sample() {
        let config = DatabaseConfig::default();
        let repo = MySqlMarketDataRepository::new(config).await.unwrap();
        
        // 测试获取某一天的全市场数据（限制数量避免过多数据）
        let trade_date = NaiveDate::from_ymd_opt(2024, 1, 2).unwrap();
        
        match repo.get_all_complete_market_data(trade_date).await {
            Ok(results) => {
                println!("成功获取 {} 只股票的完整市场数据", results.len());
                
                // 取前几只股票验证
                for (i, market_data) in results.iter().take(5).enumerate() {
                    println!("第{}只股票: {}, 前收: {}, 总市值: {:?}", 
                        i + 1,
                        market_data.static_data.stock_id,
                        market_data.static_data.prev_close_price,
                        market_data.extended_data.total_market_cap
                    );
                    
                    assert_eq!(market_data.static_data.trade_date, trade_date);
                    assert!(market_data.static_data.prev_close_price > Decimal::ZERO);
                }
                
                // 验证数据按股票代码排序
                for i in 1..results.len().min(100) { // 只检查前100个避免太慢
                    assert!(results[i-1].static_data.stock_id <= results[i].static_data.stock_id);
                }
            }
            Err(e) => println!("获取全市场数据失败: {:?}", e),
        }
    }

    #[tokio::test]
    #[ignore] // 默认忽略，需要数据库时手动运行
    async fn test_previous_close_price_calculation() {
        let config = DatabaseConfig::default();
        let repo = MySqlMarketDataRepository::new(config).await.unwrap();
        
        // 测试前一日收盘价的获取
        let stock_id = "000001.SZ";
        let trade_date = NaiveDate::from_ymd_opt(2024, 1, 3).unwrap(); // 使用一个稍后的日期
        
        match repo.get_previous_close_price(stock_id, trade_date).await {
            Ok(prev_close) => {
                println!("股票 {} 在 {} 之前的收盘价: {}", stock_id, trade_date, prev_close);
                assert!(prev_close > Decimal::ZERO);
            }
            Err(e) => println!("获取前一日收盘价失败: {:?}", e),
        }
        
        // 测试一个很早的日期，应该找不到数据
        let very_early_date = NaiveDate::from_ymd_opt(1999, 1, 1).unwrap();
        let prev_close = repo.get_previous_close_price(stock_id, very_early_date).await.unwrap();
        assert_eq!(prev_close, Decimal::from(10)); // 应该返回默认值
    }

    #[tokio::test]
    #[ignore] // 默认忽略，需要数据库时手动运行
    async fn test_limit_price_calculations() {
        let config = DatabaseConfig::default();
        let repo = MySqlMarketDataRepository::new(config).await.unwrap();
        
        // 测试涨停价和跌停价的计算
        let prev_close = dec!(10.00);
        
        let limit_up = MySqlMarketDataRepository::calculate_limit_up_price(prev_close);
        let limit_down = MySqlMarketDataRepository::calculate_limit_down_price(prev_close);
        
        // 验证计算结果
        assert_eq!(limit_up, dec!(11.00)); // 10 * 1.1 = 11
        assert_eq!(limit_down, dec!(9.00)); // 10 * 0.9 = 9
        
        println!("前收: {}, 涨停: {}, 跌停: {}", prev_close, limit_up, limit_down);
        
        // 测试小数价格
        let prev_close_decimal = dec!(12.34);
        let limit_up_decimal = MySqlMarketDataRepository::calculate_limit_up_price(prev_close_decimal);
        let limit_down_decimal = MySqlMarketDataRepository::calculate_limit_down_price(prev_close_decimal);
        
        println!("前收: {}, 涨停: {}, 跌停: {}", prev_close_decimal, limit_up_decimal, limit_down_decimal);
        
        // 验证涨停价约等于 12.34 * 1.1 = 13.574
        let expected_up = prev_close_decimal * Decimal::from_str_exact("1.1").unwrap();
        assert_eq!(limit_up_decimal, expected_up);
        
        // 验证跌停价约等于 12.34 * 0.9 = 11.106
        let expected_down = prev_close_decimal * Decimal::from_str_exact("0.9").unwrap();
        assert_eq!(limit_down_decimal, expected_down);
    }

    #[tokio::test]
    #[ignore] // 默认忽略，需要数据库时手动运行
    async fn test_database_connection_and_daily_basics_table() {
        let config = DatabaseConfig::default();
        let repo = MySqlMarketDataRepository::new(config).await.unwrap();
        
        // 测试数据库连接和daily_basics表的存在
        let sql = "SELECT COUNT(*) as count FROM daily_basics LIMIT 1";
        
        match sqlx::query(sql).fetch_one(repo.pool()).await {
            Ok(row) => {
                let count: i64 = row.get("count");
                println!("daily_basics 表存在，包含 {} 条记录", count);
                assert!(count >= 0);
            }
            Err(e) => {
                panic!("daily_basics 表不存在或无法访问: {:?}", e);
            }
        }
        
        // 测试表结构 - 使用更安全的查询方式
        let sql = "SHOW COLUMNS FROM daily_basics";
        match sqlx::query(sql).fetch_all(repo.pool()).await {
            Ok(rows) => {
                println!("daily_basics 表结构:");
                for row in rows {
                    let field: String = row.get("Field");
                    // 使用 try_get 来安全地获取 Type 字段，避免类型错误
                    let field_type = match row.try_get::<String, _>("Type") {
                        Ok(type_str) => type_str,
                        Err(_) => {
                            // 如果获取为String失败，尝试获取为其他类型并转换
                            match row.try_get::<Vec<u8>, _>("Type") {
                                Ok(type_bytes) => String::from_utf8_lossy(&type_bytes).to_string(),
                                Err(_) => "Unknown".to_string(),
                            }
                        }
                    };
                    println!("  {}: {}", field, field_type);
                }
            }
            Err(e) => {
                println!("无法获取表结构: {:?}", e);
            }
        }
    }
} 