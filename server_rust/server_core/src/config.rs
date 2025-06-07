//! 统一配置管理模块
//! 
//! 提供通过配置文件（YAML）管理所有子模块配置的功能

use config::{Config, ConfigError, File, Environment};
use serde::{Deserialize, Serialize, Deserializer};
use std::sync::Arc;
use chrono::NaiveDate;
use std::sync::LazyLock;

// 静态默认值常量
pub const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:8080";
pub const DEFAULT_DATABASE_URL: &str = "mysql://root:@localhost:3306/a_shares_real_history";
pub const DEFAULT_LOG_FILE_PATH: &str = "logs/app.log";
pub const DEFAULT_TRADE_DATE: &str = "2024-09-23";
pub const DEFAULT_LOG_LEVEL: &str = "info";

// 预分配的 Arc<str> 用于常用默认值
static DEFAULT_LISTEN_ADDR_ARC: LazyLock<Arc<str>> = LazyLock::new(|| DEFAULT_LISTEN_ADDR.into());
static DEFAULT_DATABASE_URL_ARC: LazyLock<Arc<str>> = LazyLock::new(|| DEFAULT_DATABASE_URL.into());
static DEFAULT_LOG_FILE_PATH_ARC: LazyLock<Arc<str>> = LazyLock::new(|| DEFAULT_LOG_FILE_PATH.into());
static DEFAULT_TRADE_DATE_ARC: LazyLock<Arc<str>> = LazyLock::new(|| DEFAULT_TRADE_DATE.into());
static DEFAULT_LOG_LEVEL_ARC: LazyLock<Arc<str>> = LazyLock::new(|| DEFAULT_LOG_LEVEL.into());

// 自定义反序列化函数，支持默认值优化
fn deserialize_arc_str<'de, D>(deserializer: D) -> Result<Arc<str>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(s.into())
}

fn deserialize_listen_addr<'de, D>(deserializer: D) -> Result<Arc<str>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s == DEFAULT_LISTEN_ADDR {
        Ok(DEFAULT_LISTEN_ADDR_ARC.clone())
    } else {
        Ok(s.into())
    }
}

fn deserialize_database_url<'de, D>(deserializer: D) -> Result<Arc<str>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s == DEFAULT_DATABASE_URL {
        Ok(DEFAULT_DATABASE_URL_ARC.clone())
    } else {
        Ok(s.into())
    }
}

fn deserialize_log_file_path<'de, D>(deserializer: D) -> Result<Arc<str>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s == DEFAULT_LOG_FILE_PATH {
        Ok(DEFAULT_LOG_FILE_PATH_ARC.clone())
    } else {
        Ok(s.into())
    }
}

fn deserialize_trade_date<'de, D>(deserializer: D) -> Result<Arc<str>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s == DEFAULT_TRADE_DATE {
        Ok(DEFAULT_TRADE_DATE_ARC.clone())
    } else {
        Ok(s.into())
    }
}

fn deserialize_log_level<'de, D>(deserializer: D) -> Result<Arc<str>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s == DEFAULT_LOG_LEVEL {
        Ok(DEFAULT_LOG_LEVEL_ARC.clone())
    } else {
        Ok(s.into())
    }
}

/// 应用程序统一配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 服务器配置
    pub server: ServerConfig,
    /// 订单引擎配置
    pub order_engine: OrderEngineConfig,
    /// 市场数据引擎配置
    pub market_data_engine: MarketDataEngineConfig,
    /// 数据库配置
    pub database: DatabaseConfig,
    /// 日志配置
    pub logging: LoggingConfig,
}

// 默认值函数
fn default_listen_addr() -> Arc<str> {
    DEFAULT_LISTEN_ADDR_ARC.clone()
}

fn default_max_connections() -> usize {
    10_000
}

fn default_max_message_size() -> usize {
    1024 * 1024 // 1MB
}

fn default_buffer_size() -> usize {
    8192 // 8KB
}

fn default_heartbeat_timeout_secs() -> u64 {
    30
}

fn default_database_url() -> Arc<str> {
    DEFAULT_DATABASE_URL_ARC.clone()
}

fn default_db_max_connections() -> u32 {
    10
}

fn default_connect_timeout_secs() -> u64 {
    30
}

fn default_trade_date() -> Arc<str> {
    DEFAULT_TRADE_DATE_ARC.clone()
}

fn default_log_level() -> Arc<str> {
    DEFAULT_LOG_LEVEL_ARC.clone()
}

fn default_log_file_path() -> Arc<str> {
    DEFAULT_LOG_FILE_PATH_ARC.clone()
}

/// 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// 监听地址
    #[serde(default = "default_listen_addr", deserialize_with = "deserialize_listen_addr")]
    pub listen_addr: Arc<str>,
    /// 最大连接数
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,
    /// 消息最大尺寸（字节）
    #[serde(default = "default_max_message_size")]
    pub max_message_size: usize,
    /// 缓冲区大小（字节）
    #[serde(default = "default_buffer_size")]
    pub buffer_size: usize,
    /// 心跳超时（秒）
    #[serde(default = "default_heartbeat_timeout_secs")]
    pub heartbeat_timeout_secs: u64,
}

/// 订单引擎配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderEngineConfig {
    /// 订单通知channel的缓冲区大小
    pub order_notification_buffer_size: usize,
    /// 匹配通知channel的缓冲区大小
    pub match_notification_buffer_size: usize,
    /// 是否启用订单验证
    pub enable_validation: bool,
    /// 订单池清理间隔（秒）
    pub cleanup_interval_seconds: u64,
    /// 保留已完成订单的时间（小时）
    pub retain_completed_orders_hours: i64,
}

/// 市场数据引擎配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataEngineConfig {
    /// 交易日期（格式：YYYY-MM-DD）
    #[serde(default = "default_trade_date", deserialize_with = "deserialize_trade_date")]
    pub trade_date: Arc<str>,
    /// 是否在启动时自动加载全市场数据
    #[serde(default = "default_true")]
    pub auto_load_all_market_data: bool,
    /// 行情通知缓冲区大小
    #[serde(default = "default_notification_buffer_size")]
    pub notification_buffer_size: usize,
    /// 请求响应缓冲区大小
    #[serde(default = "default_request_buffer_size")]
    pub request_buffer_size: usize,
}

fn default_true() -> bool {
    true
}

fn default_notification_buffer_size() -> usize {
    1000
}

fn default_request_buffer_size() -> usize {
    100
}

/// 数据库配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// 数据库连接URL
    #[serde(default = "default_database_url", deserialize_with = "deserialize_database_url")]
    pub database_url: Arc<str>,
    /// 最大连接数
    #[serde(default = "default_db_max_connections")]
    pub max_connections: u32,
    /// 连接超时时间（秒）
    #[serde(default = "default_connect_timeout_secs")]
    pub connect_timeout_secs: u64,
}

/// 日志配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// 日志级别: trace, debug, info, warn, error
    #[serde(default = "default_log_level", deserialize_with = "deserialize_log_level")]
    pub level: Arc<str>,
    /// 是否输出到控制台
    #[serde(default = "default_true")]
    pub console: bool,
    /// 是否输出到文件
    #[serde(default = "default_false")]
    pub file: bool,
    /// 日志文件路径
    #[serde(default = "default_log_file_path", deserialize_with = "deserialize_log_file_path")]
    pub file_path: Arc<str>,
    /// 是否启用 JSON 格式
    #[serde(default = "default_false")]
    pub json_format: bool,
}

fn default_false() -> bool {
    false
}

impl AppConfig {
    /// 从配置文件加载配置
    pub fn from_file(path: &str) -> Result<Self, ConfigError> {
        let settings = Config::builder()
            .add_source(File::with_name(path))
            .add_source(Environment::with_prefix("SHARES_SIM").separator("__"))
            .build()?;

        settings.try_deserialize()
    }

    /// 从默认位置加载配置
    pub fn load() -> Result<Self, ConfigError> {
        // 尝试按优先级加载配置文件
        let possible_paths = [
            "config.yml",
            "config.yaml", 
            "config/app.yml",
            "config/app.yaml",
            "src/config/app.yml",
            "src/config/app.yaml",
        ];

        for path in &possible_paths {
            if std::path::Path::new(path).exists() {
                return Self::from_file(path);
            }
        }

        // 如果没有找到配置文件，使用默认配置
        Ok(Self::default())
    }

    /// 保存配置到文件
    pub fn save_to_file(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let yaml = serde_yaml::to_string(self)?;
        std::fs::write(path, yaml)?;
        Ok(())
    }

    /// 获取解析后的交易日期
    pub fn get_trade_date(&self) -> Result<NaiveDate, chrono::ParseError> {
        NaiveDate::parse_from_str(&self.market_data_engine.trade_date, "%Y-%m-%d")
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                listen_addr: default_listen_addr(),
                max_connections: default_max_connections(),
                max_message_size: default_max_message_size(),
                buffer_size: default_buffer_size(),
                heartbeat_timeout_secs: default_heartbeat_timeout_secs(),
            },
            order_engine: OrderEngineConfig {
                order_notification_buffer_size: 10000,
                match_notification_buffer_size: 10000,
                enable_validation: true,
                cleanup_interval_seconds: 3600,
                retain_completed_orders_hours: 24,
            },
            market_data_engine: MarketDataEngineConfig {
                trade_date: default_trade_date(),
                auto_load_all_market_data: true,
                notification_buffer_size: default_notification_buffer_size(),
                request_buffer_size: default_request_buffer_size(),
            },
            database: DatabaseConfig {
                database_url: default_database_url(),
                max_connections: default_db_max_connections(),
                connect_timeout_secs: default_connect_timeout_secs(),
            },
            logging: LoggingConfig {
                level: default_log_level(),
                console: true,
                file: false,
                file_path: default_log_file_path(),
                json_format: false,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        
        assert_eq!(config.server.listen_addr.as_ref(), "127.0.0.1:8080");
        assert_eq!(config.server.max_connections, 10_000);
        assert_eq!(config.database.max_connections, 10);
        assert_eq!(config.order_engine.enable_validation, true);
        assert_eq!(config.market_data_engine.trade_date.as_ref(), "2024-09-23");
    }

    #[test]
    fn test_config_serialization() {
        let config = AppConfig::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        
        // 确保可以序列化为YAML
        assert!(yaml.contains("server:"));
        assert!(yaml.contains("database:"));
        assert!(yaml.contains("order_engine:"));
        assert!(yaml.contains("market_data_engine:"));
        
        // 确保可以反序列化
        let deserialized: AppConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.server.listen_addr.as_ref(), config.server.listen_addr.as_ref());
    }

    #[test]
    fn test_config_file_operations() {
        let config = AppConfig::default();
        let temp_file = NamedTempFile::with_suffix(".yml").unwrap();
        let file_path = temp_file.path().to_str().unwrap();
        
        // 保存配置到文件
        config.save_to_file(file_path).unwrap();
        
        // 直接从YAML文件加载，不使用环境变量
        let settings = Config::builder()
            .add_source(File::with_name(file_path))
            .build()
            .unwrap();
        let loaded_config: AppConfig = settings.try_deserialize().unwrap();
        
        assert_eq!(loaded_config.server.listen_addr.as_ref(), config.server.listen_addr.as_ref());
        assert_eq!(loaded_config.database.database_url.as_ref(), config.database.database_url.as_ref());
    }

    #[test]
    fn test_trade_date_parsing() {
        let config = AppConfig::default();
        let trade_date = config.get_trade_date().unwrap();
        assert_eq!(trade_date, NaiveDate::from_ymd_opt(2024, 9, 23).unwrap());
    }
} 