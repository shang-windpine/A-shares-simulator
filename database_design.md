# MySQL 数据库设计文档

## 表1: 股票基础信息 (stocks_basic_info)

根据 `get_all_stocks_basic_info.py` 设计。

| 字段名         | 数据类型    | 是否主键 | 描述                     |
| -------------- | ----------- | -------- | ------------------------ |
| ts_code        | VARCHAR(10) | 是       | TS代码                   |
| symbol         | VARCHAR(10) |          | 股票代码                 |
| name           | VARCHAR(50) |          | 股票名称                 |
| area           | VARCHAR(50) |          | 地域                     |
| industry       | VARCHAR(50) |          | 所属行业                 |
| fullname       | VARCHAR(100)|          | 股票全称                 |
| enname         | VARCHAR(100)|          | 英文全称                 |
| cnspell        | VARCHAR(50) |          | 拼音缩写                 |
| market         | VARCHAR(10) |          | 市场类型（主板/创业板/科创板/CDR） |
| exchange       | VARCHAR(10) |          | 交易所代码               |
| curr_type      | VARCHAR(5)  |          | 交易货币                 |
| list_status    | CHAR(1)     |          | 上市状态 L上市 D退市 P暂停上市 |
| list_date      | DATE        |          | 上市日期                 |
| delist_date    | DATE        |          | 退市日期                 |
| is_hs          | CHAR(1)     |          | 是否沪深港通标的，N否 H沪股通 S深股通 |
| act_name       | VARCHAR(100)|          | 实控人名称               |
| act_ent_type   | VARCHAR(50) |          | 实控人企业性质           |

## 表2: 每日基本指标 (daily_basics)

根据 `get_daily_basics.py` 设计。

| 字段名            | 数据类型       | 是否主键 | 描述                                  |
| ----------------- | -------------- | -------- | ------------------------------------- |
| ts_code           | VARCHAR(10)    | 是       | TS股票代码                              |
| trade_date        | DATE           | 是       | 交易日期                                |
| close             | DECIMAL(10,2)  |          | 当日收盘价                              |
| turnover_rate     | DECIMAL(10,4)  |          | 换手率（%）                             |
| turnover_rate_f   | DECIMAL(10,4)  |          | 换手率（自由流通股）                    |
| volume_ratio      | DECIMAL(10,2)  |          | 量比                                  |
| pe                | DECIMAL(10,2)  |          | 市盈率（总市值/净利润， 亏损的PE为空）      |
| pe_ttm            | DECIMAL(10,2)  |          | 市盈率（TTM，亏损的PE为空）             |
| pb                | DECIMAL(10,2)  |          | 市净率（总市值/净资产）                   |
| ps                | DECIMAL(10,2)  |          | 市销率                                  |
| ps_ttm            | DECIMAL(10,2)  |          | 市销率（TTM）                           |
| dv_ratio          | DECIMAL(10,4)  |          | 股息率 （%）                            |
| dv_ttm            | DECIMAL(10,4)  |          | 股息率（TTM）（%）                      |
| total_share       | DECIMAL(20,2)  |          | 总股本 （万股）                         |
| float_share       | DECIMAL(20,2)  |          | 流通股本 （万股）                         |
| free_share        | DECIMAL(20,2)  |          | 自由流通股本 （万）                     |
| total_mv          | DECIMAL(20,2)  |          | 总市值 （万元）                         |
| circ_mv           | DECIMAL(20,2)  |          | 流通市值（万元）                        |

## 表3: 每日交易信息 (daily_trade_info)

根据 `get_daily_trade_info.py` 设计。

| 字段名     | 数据类型      | 是否主键 | 描述                                       |
| ---------- | ------------- | -------- | ------------------------------------------ |
| ts_code    | VARCHAR(10)   | 是       | 股票代码                                   |
| trade_date | DATE          | 是       | 交易日期                                   |
| open       | DECIMAL(10,2) |          | 开盘价                                     |
| high       | DECIMAL(10,2) |          | 最高价                                     |
| low        | DECIMAL(10,2) |          | 最低价                                     |
| close      | DECIMAL(10,2) |          | 收盘价                                     |
| pre_close  | DECIMAL(10,2) |          | 昨收价【除权价，前复权】                     |
| change     | DECIMAL(10,2) |          | 涨跌额                                     |
| pct_chg    | DECIMAL(10,4) |          | 涨跌幅 【基于除权后的昨收计算的涨跌幅】        |
| vol        | DECIMAL(20,2) |          | 成交量 （手）                              |
| amount     | DECIMAL(20,2) |          | 成交额 （千元）                            |

## 表4: 周/月行情调整数据 (week_month_adj_info)

根据 `get_week_month_adj.py` 设计。

| 字段名     | 数据类型      | 是否主键 | 描述                                  |
| ---------- | ------------- | -------- | ------------------------------------- |
| ts_code    | VARCHAR(10)   | 是       | 股票代码                              |
| trade_date | DATE          | 是       | 交易日期                              |
| freq       | VARCHAR(5)    | 是       | 频率(周week,月month)                   |
| open       | DECIMAL(10,2) |          | (周/月)开盘价                         |
| high       | DECIMAL(10,2) |          | (周/月)最高价                         |
| low        | DECIMAL(10,2) |          | (周/月)最低价                         |
| close      | DECIMAL(10,2) |          | (周/月)收盘价                         |
| pre_close  | DECIMAL(10,2) |          | 上一(周/月)收盘价【除权价，前复权】     |
| open_qfq   | DECIMAL(10,2) |          | 前复权(周/月)开盘价                   |
| high_qfq   | DECIMAL(10,2) |          | 前复权(周/月)最高价                   |
| low_qfq    | DECIMAL(10,2) |          | 前复权(周/月)最低价                   |
| close_qfq  | DECIMAL(10,2) |          | 前复权(周/月)收盘价                   |
| open_hfq   | DECIMAL(10,2) |          | 后复权(周/月)开盘价                   |
| high_hfq   | DECIMAL(10,2) |          | 后复权(周/月)最高价                   |
| low_hfq    | DECIMAL(10,2) |          | 后复权(周/月)最低价                   |
| close_hfq  | DECIMAL(10,2) |          | 后复权(周/月)收盘价                   |
| vol        | DECIMAL(20,2) |          | (周/月)成交量                         |
| amount     | DECIMAL(20,2) |          | (周/月)成交额                         |
| change     | DECIMAL(10,2) |          | (周/月)涨跌额                         |
| pct_chg    | DECIMAL(10,4) |          | (周/月)涨跌幅 【基于除权后的昨收计算】  |