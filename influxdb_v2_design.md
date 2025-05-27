 # InfluxDB v2 数据结构设计 (A股模拟交易 MVP)

## 1. 存储桶 (Bucket)

我们将在 InfluxDB 中创建一个统一的 Bucket 来存储所有 MVP 沙盒会话期间的实时数据。

*   **Bucket 名称:** `trading_sandbox_data`
*   **数据保留策略 (Retention Policy):** 根据实际需求设定，例如，可以设置为保留最近7天或30天的数据，因为每日收盘后核心数据会整合到 MySQL。MVP阶段可以先使用默认的永久保留策略，后续再调整。

## 2. 测量 (Measurements), 标签 (Tags) 和字段 (Fields)

以下是针对不同类型实时数据的详细设计：

#### 2.1. 秒级虚拟行情 (Stock Ticks)

*   **Measurement 名称:** `stock_ticks`
*   **描述:** 存储由 `SandboxTickGenerator` 生成或用户手动步进产生的秒级虚拟股票行情数据。
*   **Tags (标签):**
    *   `stock_code` (String): 股票代码。例如："SH600036", "SZ000001"。用于唯一标识股票。
    *   `source` (String, 可选): 行情来源。例如："random_walk_v1", "manual_feed", "oscillator_v1"。用于区分不同的虚拟行情生成机制。
*   **Fields (字段):**
    *   `ask_price_1` (Float): 虚拟卖一价。例如：15.00。
    *   `bid_price_1` (Float): 虚拟买一价。例如：14.99。
    *   `last_price` (Float): 虚拟最新价。例如：15.00。
    *   `ask_volume_1` (Integer, 可选): 虚拟卖一量。例如：10000。 (MVP文档未明确要求，但行情通常包含，可酌情添加)
    *   `bid_volume_1` (Integer, 可选): 虚拟买一量。例如：5000。 (MVP文档未明确要求，但行情通常包含，可酌情添加)
*   **Timestamp (时间戳):** 数据点写入时的纳秒级时间戳，代表该行情tick的产生时间。
*   **示例数据点 (Line Protocol):**
    ```
    stock_ticks,stock_code=SZ000001,source=random_walk_v1 ask_price_1=15.00,bid_price_1=14.99,last_price=15.00 1672531200123456789
    ```

#### 2.2. 订单状态变更流 (Order Events)

*   **Measurement 名称:** `order_events`
*   **描述:** 记录沙盒模拟交易中订单生命周期内的所有状态变更事件。
*   **Tags (标签):**
    *   `order_id` (String): 系统的唯一订单ID。例如："ORD_20231027_00001"。
    *   `user_id` (String): 用户ID。MVP阶段可为固定值，如 "default_user"。
    *   `stock_code` (String): 股票代码。例如："SZ000001"。
    *   `order_type` (String): 订单类型。例如："LIMIT_BUY", "LIMIT_SELL"。
    *   `status` (String): 订单在该事件发生后的状态。例如："PENDING_NEW", "ACCEPTED", "PARTIALLY_FILLED", "FILLED", "PENDING_CANCEL", "CANCELLED", "REJECTED"。
*   **Fields (字段):**
    *   `price` (Float): 委托价格。例如：15.05。对于市价单（若未来支持），此字段可能特殊处理。
    *   `quantity` (Integer): 原始委托数量。例如：100。
    *   `filled_quantity` (Integer): 此事件发生时，订单已成交的总数量。例如：0, 50, 100。
    *   `avg_fill_price` (Float, 可选): 此事件发生时，订单已成交部分的平均价格。
    *   `reason_message` (String, 可选): 状态变更的原因文本。例如："T+1 rule violation", "Insufficient funds", "Cancelled by user"。
*   **Timestamp (时间戳):** 订单状态变更事件发生的纳秒级时间戳。
*   **示例数据点 (Line Protocol):**
    ```
    order_events,order_id=ORD_XYZ_001,user_id=default_user,stock_code=SZ000001,order_type=LIMIT_BUY,status=ACCEPTED price=15.05,quantity=100,filled_quantity=0 1672531205000000000
    order_events,order_id=ORD_XYZ_001,user_id=default_user,stock_code=SZ000001,order_type=LIMIT_BUY,status=FILLED price=15.05,quantity=100,filled_quantity=100,avg_fill_price=15.00 1672531210000000000
    ```

#### 2.3. 实时成交流 (Trade Events)

*   **Measurement 名称:** `trades`
*   **描述:** 记录沙盒模拟交易中每一笔成功的成交。
*   **Tags (标签):**
    *   `trade_id` (String): 系统的唯一成交ID。例如："TRADE_20231027_00050"。
    *   `order_id` (String): 产生此成交的订单ID。
    *   `user_id` (String): 用户ID。MVP阶段可为固定值，如 "default_user"。
    *   `stock_code` (String): 股票代码。例如："SZ000001"。
    *   `direction` (String): 交易方向。例如："BUY", "SELL"。
*   **Fields (字段):**
    *   `trade_price` (Float): 成交价格。例如：15.00。
    *   `trade_quantity` (Integer): 成交数量。例如：100。
    *   `trade_amount` (Float): 成交金额 (trade_price * trade_quantity)。例如：1500.00。
    *   `commission` (Float): 该笔交易产生的手续费。例如：0.75。
*   **Timestamp (时间戳):** 成交发生的纳秒级时间戳。
*   **示例数据点 (Line Protocol):**
    ```
    trades,trade_id=TRD_ABC_001,order_id=ORD_XYZ_001,user_id=default_user,stock_code=SZ000001,direction=BUY trade_price=15.00,trade_quantity=100,trade_amount=1500.00,commission=0.75 1672531210000000000
    ```

## 3. 设计说明与考量

*   **标签 vs. 字段 (Tags vs. Fields):**
    *   所有经常用于查询 `WHERE` 子句、`GROUP BY` 子句或作为数据系列标识符的属性都设计为 **Tags**。Tags 的值是字符串并且被索引，查询效率高。
    *   实际的度量数值、不常用于查询过滤但需要随时间记录的值被设计为 **Fields**。Fields 可以是多种数据类型，但通常不被索引（或索引方式不同于Tags）。
*   **时间戳 (Timestamp):** 所有数据点都将拥有一个纳秒级精度的时间戳，这是 InfluxDB 的核心特性，用于高效的时间序列数据管理和查询。
*   **数据类型:** Field 的数据类型在首次写入时确定，后续写入必须保持一致。上述设计中已注明建议的数据类型。对于金额等需要高精度计算的值，虽然 InfluxDB 存储为 float64，但在 Rust 服务器端应使用如 `rust_decimal` 这样的库进行精确计算，写入 InfluxDB 的值为计算结果。
*   **订单事件 (`order_events`):** 采用事件流的方式记录订单的每次状态变化，而不是只记录订单的最终状态。这样可以更完整地追溯订单的处理过程。`filled_quantity` 和 `avg_fill_price` 在 `order_events` 中反映的是 *截至该事件发生时* 的累计成交量和平均成交价。
*   **灵活性与扩展性:** 此设计基于 MVP 需求，但考虑了一定的扩展性（如 `user_id`，行情 `source`）。未来如果引入更复杂的行情数据或订单类型，可以在此基础上增加新的 Tags 或 Fields，或者创建新的 Measurements。
