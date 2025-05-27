"""
输入参数

名称	类型	必选	描述
ts_code	str	N	TS代码
trade_date	str	N	交易日期（格式：YYYYMMDD，下同）
start_date	str	N	开始日期
end_date	str	N	结束日期
freq	str	Y	频率week周，month月


**输出参数**
名称	类型	默认显示	描述
ts_code	str	Y	股票代码
trade_date	str	Y	交易日期
freq	str	Y	频率(周week,月month)
open	float	Y	(周/月)开盘价
high	float	Y	(周/月)最高价
low	float	Y	(周/月)最低价
close	float	Y	(周/月)收盘价
pre_close	float	Y	上一(周/月)收盘价【除权价，前复权】
open_qfq	float	Y	前复权(周/月)开盘价
high_qfq	float	Y	前复权(周/月)最高价
low_qfq	float	Y	前复权(周/月)最低价
close_qfq	float	Y	前复权(周/月)收盘价
open_hfq	float	Y	后复权(周/月)开盘价
high_hfq	float	Y	后复权(周/月)最高价
low_hfq	float	Y	后复权(周/月)最低价
close_hfq	float	Y	后复权(周/月)收盘价
vol	float	Y	(周/月)成交量
amount	float	Y	(周/月)成交额
change	float	Y	(周/月)涨跌额
pct_chg	float	Y	(周/月)涨跌幅 【基于除权后的昨收计算的涨跌幅：（今收-除权昨收）/除权昨收 】
"""

# 导入tushare
import tushare as ts
# 初始化pro接口
pro = ts.pro_api('1fce323835033a1b32b07ca37ff983e1825d74679afa612d66114653')

# 拉取数据
def fetch_week_month_adj(ts_code=None, trade_date=None, start_date=None, end_date=None, freq='week'):
    params = {
        "ts_code": ts_code,
        "trade_date": trade_date,
        "start_date": start_date,
        "end_date": end_date,
        "freq": freq,
        "limit": "",
        "offset": ""
    }
    # 移除值为None的参数，以使用Tushare接口的默认值
    params = {k: v for k, v in params.items() if v is not None}

    df = pro.stk_week_month_adj(**params, fields=[
        "ts_code",
        "trade_date",
        "freq",
        "open",
        "high",
        "low",
        "close",
        "pre_close",
        "open_qfq",
        "high_qfq",
        "low_qfq",
        "close_qfq",
        "open_hfq",
        "high_hfq",
        "low_hfq",
        "close_hfq",
        "vol",
        "amount",
        "change",
        "pct_chg"
    ])
    return df

if __name__ == "__main__":
    # 测试周线数据
    print("测试获取周线数据...")
    week_data = fetch_week_month_adj(ts_code='000001.SZ', start_date='20230101', end_date='20230331', freq='week')
    print(week_data)

    # 测试月线数据
    print("\n测试获取月线数据...")
    month_data = fetch_week_month_adj(ts_code='000001.SZ', start_date='20230101', end_date='20230331', freq='month')
    print(month_data)

    # 测试单个交易日（通常对于周线/月线意义不大，但接口支持）
    print("\n测试获取单个交易日（周/月）数据...")
    single_date_data = fetch_week_month_adj(ts_code='000001.SZ', trade_date='20230315', freq='week')
    print(single_date_data)

    # 测试不指定股票代码（获取所有股票在某个时间点的数据，可能会非常大，Tushare可能有限制）
    # print("\n测试获取某日所有股票周线数据...")
    # all_stocks_week_data = fetch_week_month_adj(trade_date='20230310', freq='week')
    # print(all_stocks_week_data.head()) # 打印前几行

    # 测试不指定日期，获取最近的数据 (Tushare pro.stk_week_month_adj 接口可能不直接支持此方式获取"最新"，通常需要指定日期范围或单个日期)
    # print("\n测试获取最新周线数据...")
    # latest_week_data = fetch_week_month_adj(ts_code='000001.SZ', freq='week')
    # print(latest_week_data)