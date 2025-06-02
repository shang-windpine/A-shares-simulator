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
pub use server::{Server, ServerConfig, ServerStats};
pub use connection_manager::ConnectionId;
pub use app::{App, AppConfig, AppServices};

// Initialize tracing subscriber for the library
pub fn init_tracing() {
    // 使用简单的格式化输出，如果需要更复杂的配置，用户可以自己初始化
    tracing_subscriber::fmt::init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_exports() {
        // 测试所有主要组件都能正确导入
        
        // 这些类型应该都能正确导入
        let _: Option<ConnectionError> = None;
        let _: Option<ConnectionId> = None;
        let _: Option<ServerConfig> = None;
        
        // 测试创建配置
        let config = ServerConfig::default();
        assert!(!config.listen_addr.is_empty());
        assert!(config.max_connections > 0);
    }

    #[tokio::test]
    async fn test_server_creation_through_lib() {
        // 测试通过库接口创建服务器
        let server = Server::new_with_addr("127.0.0.1:0").await;
        assert!(server.is_ok());
        
        let server = server.unwrap();
        let stats = server.get_stats().await;
        assert_eq!(stats.connection_stats.active_count, 0);
    }
} 