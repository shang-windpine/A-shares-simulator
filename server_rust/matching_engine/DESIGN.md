# 撮合引擎 (Matching Engine) 设计文档

## 1. 引言

本文档定义了A股模拟交易软件核心组件——撮合引擎 (Matching Engine) 的设计。撮合引擎负责接收订单，并根据市场行情数据进行匹配，最终产生交易。其设计目标是实现一个高效、准确、可扩展且易于测试的匹配核心。

本文档基于先前的讨论，特别是强调了对底层存储的抽象以及采用更真实的成交规则。

## 2. 核心职责

*   接收来自上游系统（如订单管理系统）的标准化订单。
*   管理各个交易标的（如股票）的订单簿。
*   根据定义的成交规则（价格优先、时间优先）和市场行情，匹配买卖订单。
*   生成成交记录 (Trades)。
*   更新订单簿以反映已发生的交易。
*   将成交结果通知相关系统。

撮合引擎本身不负责用户如何下单、订单的初步校验（如资金检查），这些由上游模块处理。

### 2.5. 运作模式与架构定位

撮合引擎被设计为一个核心的、自运行的中间件系统，其运作模式具有以下关键特征：

*   **独立与持续运行：** 撮合引擎作为一个独立的服务或进程持续运行，拥有其内部的事件处理循环或主线程。它并非响应单一业务请求后即停止，而是持续监控和处理数据输入。
*   **事件驱动与异步处理：** 核心匹配逻辑的触发通常是事件驱动的（例如，新订单到达、行情变动）。它与上游系统（如订单管理系统）和下游系统（如账户服务、通知服务）通过异步方式交互，例如通过消息队列或共享的内存数据存储（如Redis）。用户的交易请求不会同步阻塞撮合引擎的核心操作。
*   **持续数据输入：** 引擎持续从一个或多个预定义的数据源（通过 `OrderBookStore` 抽象接口访问，具体实现如Redis）获取数据。这些输入数据主要包括：
    *   由订单管理系统（OMS）传入的新订单或撤单请求。
    *   市场行情信息（在MVP阶段，这可能是通过 `OrderBookStore` 间接感知的订单簿变化；未来可扩展为直接消费独立的 `MarketTick` 流）。
*   **数据处理与输出：** 接收输入后，撮合引擎根据既定的匹配规则（如价格优先、时间优先）进行处理。其核心输出包括：
    *   成交记录 (Trades)。
    *   订单状态的更新。
    *   订单簿的实时状态变更（通过 `OrderBookStore` 更新）。
    *   通过 `MatchLifecycleObserver` 等机制向其他系统发出的通知和事件。
*   **解耦特性：** 撮合引擎的核心逻辑与具体的业务请求（如用户下单界面操作）、用户会话管理等完全解耦。它专注于高效、准确地执行订单匹配这一核心任务。这种设计保证了系统的高吞吐量、低延迟和良好的可扩展性。

## 3. 关键抽象与接口 (Traits)

为了提高灵活性、可测试性和可扩展性，撮合引擎将依赖以下抽象接口：

### 3.1. 内存数据存储接口 (`OrderBookStore`)

该 trait 定义了撮合引擎与底层内存数据库（用于存储订单簿和即时行情）交互的契约。这使得引擎不直接依赖于特定数据库实现（如 Redis）。

```rust
// 概念性 Trait 定义
pub trait OrderBookStore {
    type Error; // 存储操作相关的错误类型

    /// 提交新订单到指定股票的订单簿
    fn submit_order(&mut self, order: &Order) -> Result<(), Self::Error>;

    /// 尝试取消订单簿中的某个订单
    /// 返回被成功取消的订单详情，如果订单不存在或无法取消则返回错误。
    fn cancel_order(&mut self, stock_id: &StockId, order_id: &OrderId) -> Result<Order, Self::Error>;

    /// 获取指定股票的订单簿快照（例如，买卖各 N 档深度）
    fn get_order_book_snapshot(&self, stock_id: &StockId, depth: usize) -> Result<OrderBookView, Self::Error>;

    /// 获取指定股票的最优买一价/量和卖一价/量
    /// 返回 (Option<(Price, Quantity)>, Option<(Price, Quantity)>) 代表 (BestBid, BestAsk)
    fn get_best_bid_ask(&self, stock_id: &StockId) -> Result<(Option<OrderBookEntry>, Option<OrderBookEntry>), Self::Error>;

    /// （可选，根据行情数据来源）更新内存中的市场行情tick
    fn update_market_tick(&mut self, tick: &MarketTick) -> Result<(), Self::Error>;

    /// 根据成交结果更新订单簿（移除已成交部分，修改部分成交订单）
    /// 需要传入成交的买单ID、卖单ID以及成交数量和价格
    fn update_order_book_after_trade(
        &mut self,
        stock_id: &StockId,
        trade_confirmation: &TradeConfirmation,
    ) -> Result<(), Self::Error>;

    // 可能需要的其他方法：
    // fn get_order_by_id(&self, stock_id: &StockId, order_id: &OrderId) -> Result<Option<Order>, Self::Error>;
}

// 辅助数据结构 (概念性)
pub struct OrderBookEntry { pub price: Price, pub total_quantity: Quantity, pub order_count: usize }
pub struct OrderBookView { pub bids: Vec<OrderBookEntry>, pub asks: Vec<OrderBookEntry> }
```

### 3.2. 成交流程观察者/钩子 (`MatchLifecycleObserver`)

该 trait (或一组 traits/回调) 允许外部模块在撮合生命周期的关键节点注入逻辑，而无需修改撮合引擎核心。

```rust
// 概念性 Trait 定义
pub trait MatchLifecycleObserver {
    type Error;

    /// 订单尝试匹配前调用
    fn on_before_match_attempt(&self, order: &Order, current_order_book: &OrderBookView) -> Result<(), Self::Error>;

    /// 订单成功匹配并产生交易时调用 (可能多次，如果一个订单与多个对手方订单成交)
    fn on_trade_generated(&self, trade: &Trade, remaining_aggressor_order_qty: Quantity) -> Result<(), Self::Error>;

    /// 订单处理完成（完全成交、部分成交或未成交）后调用
    fn on_after_order_processed(&self, order_id: &OrderId, final_status: OrderStatus) -> Result<(), Self::Error>;
}
```
撮合引擎可以在其内部逻辑中组合一个或多个 `MatchLifecycleObserver` 的实现。

## 4. 核心数据结构 (概念性)

撮合引擎内部或通过接口交互时会用到以下核心数据结构：

*   **`StockId`**: 交易标的唯一标识符 (例如, `String` "SH600036")。
*   **`OrderId`**: 订单唯一标识符 (例如, `Uuid` 或 `u64`)。
*   **`UserId`**: 用户唯一标识符。
*   **`Price`**: 价格类型 (建议使用高精度小数类型，如 `rust_decimal::Decimal`)。
*   **`Quantity`**: 数量类型 (例如, `u64`)。
*   **`Timestamp`**: 时间戳 (例如, `chrono::DateTime<Utc>`)。
*   **`OrderSide`**: 枚举 `Buy` | `Sell`。
*   **`OrderType`**: 枚举 `Limit` (MVP阶段仅支持限价单)。未来可扩展 `Market` 等。
*   **`OrderStatus`**: 枚举 `New`, `PartiallyFilled`, `Filled`, `Cancelled`, `Rejected`。

*   **`Order`**:
    *   `id: OrderId`
    *   `stock_id: StockId`
    *   `user_id: UserId`
    *   `side: OrderSide`
    *   `order_type: OrderType`
    *   `price: Price` (对于限价单)
    *   `quantity: Quantity` (原始委托数量)
    *   `unfilled_quantity: Quantity` (未成交数量)
    *   `timestamp: Timestamp` (订单提交时间)
    *   `status: OrderStatus`

*   **`Trade` / `TradeConfirmation`**:
    *   `id: TradeId` (交易唯一ID)
    *   `stock_id: StockId`
    *   `price: Price` (成交价格)
    *   `quantity: Quantity` (成交数量)
    *   `timestamp: Timestamp` (成交时间)
    *   `aggressor_order_id: OrderId` (主动方订单ID)
    *   `resting_order_id: OrderId` (订单簿上的被动方订单ID)
    *   `buyer_order_id: OrderId`
    *   `seller_order_id: OrderId`
    *   `buyer_user_id: UserId`
    *   `seller_user_id: UserId`

*   **`MarketTick`** (撮合引擎消费的行情数据):
    *   `stock_id: StockId`
    *   `best_bid_price: Option<Price>`
    *   `best_bid_quantity: Option<Quantity>`
    *   `best_ask_price: Option<Price>`
    *   `best_ask_quantity: Option<Quantity>`
    *   `last_traded_price: Option<Price>`
    *   `timestamp: Timestamp`
    *   *注意: MVP阶段，撮合引擎可能直接从 `OrderBookStore` 获取最优买卖价，而不是消费独立的 `MarketTick` 流。但设计上应考虑未来行情数据独立推送的可能，做到低耦合。*

## 5. 核心匹配逻辑 (成交规则)

撮合引擎将严格按照以下规则进行订单匹配：

### 5.1. 优先规则

1.  **价格优先 (Price Priority)**:
    *   **买单 (Bids)**: 价格较高的买单优先于价格较低的买单成交。
    *   **卖单 (Asks)**: 价格较低的卖单优先于价格较高的卖单成交。
2.  **时间优先 (Time Priority)**:
    *   在同一价格上，先提交到订单簿的订单（时间戳较早）优先于后提交的订单成交。

### 5.2. 订单匹配过程 (以新进入的订单为"主动方订单 Aggressor Order")

当一个新的订单 (主动方订单) 进入撮合引擎时：

1.  **确定对手方订单簿**:
    *   如果主动方是买单，则在对应股票的卖单簿 (Ask Book) 中寻找匹配。
    *   如果主动方是卖单，则在对应股票的买单簿 (Bid Book) 中寻找匹配。

2.  **遍历对手方订单簿**:
    *   **对于买入主动方订单**: 从卖单簿中价格最低（最优卖价 Best Ask）的订单开始，向上（价格较高）遍历。
    *   **对于卖出主动方订单**: 从买单簿中价格最高（最优买价 Best Bid）的订单开始，向下（价格较低）遍历。

3.  **检查匹配条件**:
    *   **买入主动方订单 vs 订单簿上的卖单**:
        *   匹配条件: 主动方买单的限价 `aggressor_order.price >= resting_ask_order.price`。
    *   **卖出主动方订单 vs 订单簿上的买单**:
        *   匹配条件: 主动方卖单的限价 `aggressor_order.price <= resting_bid_order.price`。

4.  **确定成交价格 (Price Determination)**:
    *   **成交价为被动方订单的价格 (Resting Order's Price)**。
        *   即，如果买单A (限价10.05) 与卖单X (在订单簿上，限价10.00) 成交，则成交价为 10.00。
        *   如果卖单B (限价9.95) 与买单Y (在订单簿上，限价10.00) 成交，则成交价为 10.00。
    *   这实现了价格改善 (Price Improvement) 的效果：主动方订单可能会以比其限价更有利的价格成交。

5.  **确定成交数量**:
    *   成交数量为 `min(aggressor_order.unfilled_quantity, resting_order.unfilled_quantity)`。

6.  **生成成交记录**:
    *   为每一笔撮合成功的匹配生成一个 `Trade` 记录。
    *   调用 `MatchLifecycleObserver::on_trade_generated`。

7.  **更新订单状态和数量**:
    *   减少主动方订单和被动方订单的 `unfilled_quantity`。
    *   更新订单的 `status` (如 `PartiallyFilled` 或 `Filled`)。

8.  **更新订单簿**:
    *   如果被动方订单完全成交，将其从订单簿中移除。
    *   如果被动方订单部分成交，更新其在订单簿中的剩余数量。
    *   通过 `OrderBookStore::update_order_book_after_trade` 实现。

9.  **循环匹配**:
    *   如果主动方订单仍有未成交数量 (`aggressor_order.unfilled_quantity > 0`) 且订单簿中仍有可匹配的对手方订单 (满足价格条件)，则继续与下一个最优的对手方订单进行匹配 (返回步骤 3)。

10. **主动方订单处理完成**:
    *   如果主动方订单完全成交，其处理结束。
    *   如果主动方订单部分成交或未成交 (因为没有更多可匹配的对手方订单，或其限价无法满足当前市场)，并且是限价单，则该主动方订单（或其剩余部分）将被添加到其自身的订单簿中，成为一个新的被动订单 (Resting Order)，等待未来的匹配。
    *   调用 `MatchLifecycleObserver::on_after_order_processed`。

### 5.3. 订单簿的表示

*   订单簿通常按价格水平聚合。每个价格水平包含该价格的所有订单，按时间顺序排列。
*   `OrderBookStore` 的实现（如使用 Redis Sorted Sets）需要高效支持价格优先和时间优先的查找与更新。

## 6. 与其他模块的交互

*   **订单管理系统 (OMS) / 应用层服务**:
    *   接收来自OMS的、已经过初步校验的订单请求。
    *   将订单提交到对应股票的撮合队列。
*   **`OrderBookStore` 实现 (例如 Redis Adapter)**:
    *   撮合引擎通过 `OrderBookStore` trait 读写订单簿数据和行情快照。
*   **行情数据源 (`SandboxTickGenerator` 或真实行情接入)**:
    *   MVP阶段，`SandboxTickGenerator` 生成的秒级虚拟行情会通过 `OrderBookStore` (例如写入Redis) 更新市场状态，撮合引擎间接消费。
    *   未来可能直接订阅行情流。
*   **成交回报/通知服务**:
    *   通过 `MatchLifecycleObserver` 或直接调用，将生成的 `Trade` 记录和订单状态更新推送给下游服务（如账户管理、风险控制、客户端通知、持久化存储服务）。
*   **持久化存储 (InfluxDB, MySQL)**:
    *   成交记录的最终持久化由下游服务负责，撮合引擎本身不直接写入长期存储，但会通过观察者模式触发这些行为。

## 7. 错误处理

*   `OrderBookStore` 和 `MatchLifecycleObserver` 的 trait 方法都应返回 `Result`，允许传播和处理错误。
*   撮合引擎需要定义自身的错误类型，涵盖如订单无效、匹配逻辑错误等。
*   对于关键错误，应有明确的日志记录和可能的告警机制。

## 8. 性能考量

*   撮合逻辑应尽可能高效，避免不必要的计算和IO。
*   与 `OrderBookStore` 的交互是性能关键点，其实现需要优化。
*   使用异步处理 (`async/await` in Rust) 来管理并发订单处理和IO操作。

## 9. 未来可能的扩展点

*   支持更多订单类型 (市价单、止损单等)。
*   更复杂的匹配算法或可配置的匹配规则。
*   与更复杂的风控模块集成。
*   跨市场撮合。

---

本文档为撮合引擎的初步设计，具体实现细节可能会根据开发过程中的发现进行调整。 