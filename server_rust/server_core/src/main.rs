use server_core::{App, init_tracing};
use tracing::{info, error};
use core_entities::app_config::AppConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {    
    // 加载配置
    let config = match AppConfig::load() {
        Ok(config) => {
            println!("成功加载配置文件");
            config
        }
        Err(e) => {
            println!("加载配置文件失败，使用默认配置: {}", e);
            AppConfig::default()
        }
    };

    // 初始化日志（使用配置中的日志设置）
    init_tracing(&config.logging);
    
    info!("启动A股模拟交易服务器");
    info!("配置信息: 监听地址={}, 数据库URL={}", 
          config.server.listen_addr, 
          config.database.database_url);

    // 创建应用程序实例
    let mut app = App::new(config);
    
    // 初始化服务
    app.initialize_services().await?;
    
    info!("服务初始化完成，开始运行服务器");    
    // 运行应用程序
    if let Err(e) = app.run().await {
        error!("服务器运行出错: {}", e);
        return Err(e.into());
    }
    info!("服务器正常关闭");
    Ok(())
} 