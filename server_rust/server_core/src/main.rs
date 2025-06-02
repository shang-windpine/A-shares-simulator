use server_core::{App, init_tracing};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    init_tracing();
    
    info!("启动A股模拟交易服务器");

    // 创建应用程序实例
    let mut app = App::with_defaults();
    
    // 初始化服务
    app.initialize_services().await?;
    
    info!("服务初始化完成，开始运行服务器");
    
    // 运行应用程序
    if let Err(e) = app.run().await {
        eprintln!("服务器运行出错: {}", e);
        return Err(e.into());
    }

    info!("服务器正常关闭");
    Ok(())
} 