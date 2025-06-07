# 配置系统说明

A股模拟交易系统现在支持通过YAML配置文件进行配置管理，替代了之前的硬编码配置方式。

## 快速开始

### 1. 生成默认配置文件

```bash
# 生成默认配置文件 config.yml
cargo run --bin generate_config

# 或者指定输出路径
cargo run --bin generate_config my_config.yml
```

### 2. 编辑配置文件

编辑生成的 `config.yml` 文件，根据您的需求调整配置：

```yaml
# 服务器配置
server:
  listen_addr: "127.0.0.1:8080"
  max_connections: 10000

# 数据库配置
database:
  database_url: "mysql://root:password@localhost:3306/a_shares_real_history"
  max_connections: 20

# 其他配置...
```

### 3. 启动服务器

```bash
# 服务器会自动查找并加载配置文件
cargo run --bin server_core
```

## 配置文件查找顺序

系统会按以下顺序查找配置文件：

1. `config.yml`
2. `config.yaml`
3. `config/app.yml`
4. `config/app.yaml`
5. `src/config/app.yml`
6. `src/config/app.yaml`

如果没有找到任何配置文件，系统将使用默认配置。

## 配置项说明

### 服务器配置 (server)

| 配置项 | 类型 | 默认值 | 说明 |
|--------|------|--------|------|
| `listen_addr` | String | "127.0.0.1:8080" | 服务器监听地址 |
| `max_connections` | usize | 10000 | 最大连接数 |
| `max_message_size` | usize | 1048576 | 消息最大尺寸（字节） |
| `buffer_size` | usize | 8192 | 缓冲区大小（字节） |
| `heartbeat_timeout_secs` | u64 | 30 | 心跳超时时间（秒） |

### 订单引擎配置 (order_engine)

| 配置项 | 类型 | 默认值 | 说明 |
|--------|------|--------|------|
| `order_notification_buffer_size` | usize | 10000 | 订单通知缓冲区大小 |
| `match_notification_buffer_size` | usize | 10000 | 匹配通知缓冲区大小 |
| `enable_validation` | bool | true | 是否启用订单验证 |
| `cleanup_interval_seconds` | u64 | 3600 | 订单池清理间隔（秒） |
| `retain_completed_orders_hours` | i64 | 24 | 保留已完成订单时间（小时） |

### 市场数据引擎配置 (market_data_engine)

| 配置项 | 类型 | 默认值 | 说明 |
|--------|------|--------|------|
| `trade_date` | String | "2024-09-23" | 交易日期（YYYY-MM-DD格式） |
| `auto_load_all_market_data` | bool | true | 启动时是否自动加载全市场数据 |
| `notification_buffer_size` | usize | 1000 | 行情通知缓冲区大小 |
| `request_buffer_size` | usize | 100 | 请求响应缓冲区大小 |

### 数据库配置 (database)

| 配置项 | 类型 | 默认值 | 说明 |
|--------|------|--------|------|
| `database_url` | String | "mysql://root:@localhost:3306/a_shares_real_history" | 数据库连接URL |
| `max_connections` | u32 | 10 | 最大连接数 |
| `connect_timeout_secs` | u64 | 30 | 连接超时时间（秒） |

### 日志配置 (logging)

| 配置项 | 类型 | 默认值 | 说明 |
|--------|------|--------|------|
| `level` | String | "info" | 日志级别 (trace/debug/info/warn/error) |
| `console` | bool | true | 是否输出到控制台 |
| `file` | bool | false | 是否输出到文件 |
| `file_path` | String | "logs/app.log" | 日志文件路径 |
| `json_format` | bool | false | 是否使用JSON格式 |

## 环境变量覆盖

您可以使用环境变量覆盖配置文件中的任何设置。环境变量格式为：

```
SHARES_SIM__<SECTION>__<KEY>
```

例如：

```bash
# 覆盖服务器监听地址
export SHARES_SIM__SERVER__LISTEN_ADDR="0.0.0.0:8080"

# 覆盖数据库最大连接数
export SHARES_SIM__DATABASE__MAX_CONNECTIONS="50"

# 覆盖日志级别
export SHARES_SIM__LOGGING__LEVEL="debug"

# 启动服务器
cargo run --bin server_core
```

## 配置验证

系统启动时会验证配置的有效性：

- 交易日期格式必须为 YYYY-MM-DD
- 数据库URL必须是有效的MySQL连接字符串
- 端口号必须在有效范围内
- 缓冲区大小必须大于0

## 生产环境建议

### 安全配置

1. **数据库连接**: 使用强密码，避免在配置文件中明文存储密码，建议使用环境变量
2. **监听地址**: 生产环境建议使用 `0.0.0.0:8080` 或特定IP地址
3. **连接数限制**: 根据服务器性能合理设置最大连接数

### 性能优化

1. **缓冲区大小**: 高频交易场景可适当增大缓冲区
2. **数据库连接池**: 根据数据库性能调整连接池大小
3. **日志级别**: 生产环境建议使用 `info` 或 `warn` 级别

### 示例生产配置

```yaml
server:
  listen_addr: "0.0.0.0:8080"
  max_connections: 50000

database:
  max_connections: 50
  connect_timeout_secs: 10

order_engine:
  order_notification_buffer_size: 50000
  match_notification_buffer_size: 50000

logging:
  level: "info"
  file: true
  json_format: true
```

## 故障排除

### 常见问题

1. **配置文件未找到**: 确保配置文件在正确的路径下
2. **格式错误**: 检查YAML语法是否正确
3. **权限问题**: 确保应用有读取配置文件的权限
4. **环境变量未生效**: 检查环境变量名称格式是否正确

### 调试技巧

1. 使用 `debug` 日志级别查看详细信息
2. 检查启动日志中的配置信息
3. 使用配置生成工具重新生成默认配置进行对比 