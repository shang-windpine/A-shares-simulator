# Workspace 依赖整合说明

## 概述

本项目已将所有第三方依赖统一管理到workspace级别，实现版本对齐和统一管理。

## 整合后的依赖结构

### Workspace级别依赖 (`Cargo.toml`)

```toml
[workspace.dependencies]
# 异步运行时
tokio = { version = "1.45.1", features = ["full"] }

# 时间处理
chrono = { version = "0.4.41", features = ["serde"] }

# 数字精度
rust_decimal = "1.37.1"
rust_decimal_macros = "1.37.1"

# 序列化
serde = { version = "1.0.219", features = ["derive"] }
serde_json = "1.0.140"

# 错误处理
anyhow = "1.0.98"
thiserror = "2.0.12"

# 日志
tracing = "0.1.41"
tracing-subscriber = { version = "0.3.19", features = ["env-filter"] }

# 网络和字节处理
bytes = "1.10.1"

# protobuf
prost = "0.13.5"
prost-build = "0.13.5"

# 数据库
sqlx = { version = "0.8.6", features = ["runtime-tokio-rustls", "mysql", "chrono", "rust_decimal"] }

# 高并发集合
dashmap = "6.1.0"

# 集合
indexmap = "2.9.0"

# 异步trait
async-trait = "0.1.88"

# UUID生成
uuid = { version = "1.17.0", features = ["v4", "fast-rng", "macro-diagnostics"] }

# 锁
parking_lot = "0.12.3"

# Redis
redis = { version = "0.31.0", features = ["tokio-comp"] }

# 随机数
rand = "0.8.5"

# Workspace内部依赖
core_entities = { path = "core_entities" }
trade_protocal_lite = { path = "trade_protocal_lite" }
order_engine = { path = "order_engine" }
matching_engine = { path = "matching_engine" }
market_data_engine = { path = "market_data_engine" }
account_management = { path = "account_management" }
market_rules = { path = "market_rules" }
tick_generator = { path = "tick_generator" }
server_core = { path = "server_core" }
```

## 版本统一情况

### 原有版本差异及统一结果

1. **tokio**: 统一为 `1.45.1`
   - 原来: `1.39.0`, `1.45.1`
   - 统一后: `1.45.1`

2. **chrono**: 统一为 `0.4.41`
   - 原来: `0.4`, `0.4.30`, `0.4.41`
   - 统一后: `0.4.41`

3. **rust_decimal**: 统一为 `1.37.1`
   - 原来: `1.36.0`, `1.37.1`
   - 统一后: `1.37.1`

4. **tracing**: 统一为 `0.1.41`
   - 原来: `0.1.40`, `0.1.41`
   - 统一后: `0.1.41`

5. **tracing-subscriber**: 统一为 `0.3.19`
   - 原来: `0.3.18`, `0.3.19`
   - 统一后: `0.3.19`

6. **uuid**: 统一为 `1.17.0`
   - 原来: `1.0`, `1.17.0`
   - 统一后: `1.17.0`

## 各包使用情况

### core_entities
- chrono, rust_decimal, rust_decimal_macros, serde, anyhow

### trade_protocal_lite
- bytes, prost, thiserror
- **build-dependencies**: prost-build

### matching_engine
- anyhow, async-trait, chrono, redis, rust_decimal, rust_decimal_macros, serde, thiserror, tokio, tracing, tracing-subscriber, uuid
- **内部依赖**: order_engine, core_entities

### order_engine
- chrono, rust_decimal, rust_decimal_macros, tokio, dashmap, uuid, tracing, tracing-subscriber, parking_lot
- **内部依赖**: core_entities

### market_data_engine
- tokio, chrono, rust_decimal, rust_decimal_macros, tracing, tracing-subscriber, serde, serde_json, sqlx, thiserror, dashmap, indexmap, async-trait
- **内部依赖**: core_entities

### server_core
- tokio, bytes, prost, thiserror, anyhow, rust_decimal, rust_decimal_macros, tracing, tracing-subscriber, chrono, async-trait, rand
- **内部依赖**: trade_protocal_lite, order_engine, matching_engine, market_data_engine, core_entities

### account_management
- **内部依赖**: core_entities

### market_rules
- **内部依赖**: core_entities

### tick_generator
- 目前无依赖（预留了redis、tokio、core_entities的注释）

## 使用方式

在各个包的 `Cargo.toml` 中，使用 `{ workspace = true }` 语法引用workspace级别的依赖：

```toml
[dependencies]
tokio = { workspace = true }
chrono = { workspace = true }
core_entities = { workspace = true }
```

## 优势

1. **版本统一**: 所有包使用相同版本的第三方依赖，避免版本冲突
2. **易于维护**: 只需在workspace级别更新依赖版本
3. **构建优化**: 减少重复编译，提高构建速度
4. **依赖管理**: 集中管理所有依赖，便于安全审查和许可证检查

## 特殊情况处理

如果某个包需要使用特定版本或features，可以在该包的 `Cargo.toml` 中单独声明：

```toml
[dependencies]
# 使用workspace版本
tokio = { workspace = true }

# 特殊版本或features
special_crate = { version = "1.0", features = ["special-feature"] }
```

## 验证

使用以下命令验证workspace配置：

```bash
cargo check --workspace
cargo build --workspace
cargo test --workspace
``` 