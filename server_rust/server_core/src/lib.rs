pub mod error;
pub mod frame_decoder;
pub mod connection_manager;
pub mod message_dispatcher;
pub mod client_connection;
pub mod server;
pub mod app;

// Re-export key components
pub use error::{ConnectionError};
pub use frame_decoder::FrameDecoder;
pub use connection_manager::{ConnectionManager, ConnectionStats};
pub use message_dispatcher::MessageDispatcher;
pub use client_connection::{ClientConnection, ConnectionInfo};
pub use server::{Server, ServerStats};
pub use connection_manager::ConnectionId;
pub use app::{App, AppServices};

// 导入配置类型
use core_entities::app_config::LoggingConfig;

// tracing-subscriber 配置
pub fn init_tracing(config: &LoggingConfig) {
    use tracing::Level;
    use tracing_subscriber::fmt;
    use std::fs::OpenOptions;
    
    // 解析日志级别
    let level = match config.level.as_ref() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO, // 默认为 info
    };
    
    // 根据配置决定输出位置和格式
    if config.console && !config.file {
        // 只输出到控制台
        fmt()
            .with_max_level(level)
            .with_target(false)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true)
            .with_thread_names(true)
            .with_level(true)
            .with_ansi(true)
            .with_writer(std::io::stdout)
            .init();
    } else if !config.console && config.file {
        // 只输出到文件
        if let Some(parent) = std::path::Path::new(&*config.file_path).parent() {
            std::fs::create_dir_all(parent).expect("无法创建日志目录");
        }
        
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(&*config.file_path)
            .expect("无法创建日志文件");
            
        fmt()
            .with_max_level(level)
            .with_target(false)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true)
            .with_thread_names(true)
            .with_level(true)
            .with_ansi(false) // 文件输出时禁用颜色
            .with_writer(std::sync::Mutex::new(file))
            .init();
    } else {
        // 默认或同时输出的情况，输出到控制台
        // 注意：tracing-subscriber 的单个 subscriber 只能有一个 writer
        // 如果需要同时输出到多个目标，需要使用更复杂的配置
        fmt()
            .with_max_level(level)
            .with_target(false)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true)
            .with_thread_names(true)
            .with_level(true)
            .with_ansi(true)
            .with_writer(std::io::stdout)
            .init();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_entities::app_config::{ServerConfig, AppConfig};

    #[test]
    fn test_library_exports() {
        // 测试所有主要组件都能正确导入
        
        // 这些类型应该都能正确导入
        let _: Option<ConnectionError> = None;
        let _: Option<ConnectionId> = None;
        let _: Option<ServerConfig> = None;
        
        // 测试创建配置
        let app_config = AppConfig::default();
        let config = app_config.server;
        assert!(!config.listen_addr.is_empty());
        assert!(config.max_connections > 0);
    }
} 