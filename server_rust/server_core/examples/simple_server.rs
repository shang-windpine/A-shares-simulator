use server_core::{Server, ServerConfig, init_tracing};
use tokio::signal;
use tracing::info;
use std::borrow::Cow;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    init_tracing();
    
    info!("启动 A股模拟交易服务器");

    // 创建服务器配置
    let config = ServerConfig {
        listen_addr: Cow::Borrowed("127.0.0.1:8080"),
        max_connections: 1000, // 设置较小的连接数用于演示
    };

    // 创建服务器实例
    let mut server = Server::new(config).await?;
    
    info!("服务器启动，监听地址: {}", server.listen_addr());
    info!("实际绑定地址: {:?}", server.local_addr()?);

    // 运行服务器
    // 注意：在实际应用中，您可能需要实现 Ctrl+C 信号处理
    // 这里简化处理，服务器将无限期运行直到遇到错误
    info!("服务器开始运行，按 Ctrl+C 停止...");
    
    tokio::select! {
        // 服务器主循环
        result = server.run() => {
            match result {
                Ok(_) => info!("服务器正常关闭"),
                Err(e) => {
                    eprintln!("服务器运行出错: {}", e);
                    return Err(e.into());
                }
            }
        }
        
        // Ctrl+C 信号处理
        _ = signal::ctrl_c() => {
            info!("收到 Ctrl+C 信号，开始关闭服务器...");
            server.shutdown().await;
            info!("关闭信号已发送");
        }
    }

    info!("服务器已关闭");
    Ok(())
} 