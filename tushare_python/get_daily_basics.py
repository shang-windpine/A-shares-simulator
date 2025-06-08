'''
输入参数

名称	类型	必选	描述
ts_code	str	Y	股票代码（二选一）
trade_date	str	N	交易日期 （二选一）
start_date	str	N	开始日期(YYYYMMDD)
end_date	str	N	结束日期(YYYYMMDD)
注：日期都填YYYYMMDD格式，比如20181010

输出参数

名称	类型	描述
ts_code	str	TS股票代码
trade_date	str	交易日期
close	float	当日收盘价
turnover_rate	float	换手率（%）
turnover_rate_f	float	换手率（自由流通股）
volume_ratio	float	量比
pe	float	市盈率（总市值/净利润， 亏损的PE为空）
pe_ttm	float	市盈率（TTM，亏损的PE为空）
pb	float	市净率（总市值/净资产）
ps	float	市销率
ps_ttm	float	市销率（TTM）
dv_ratio	float	股息率 （%）
dv_ttm	float	股息率（TTM）（%）
total_share	float	总股本 （万股）
float_share	float	流通股本 （万股）
free_share	float	自由流通股本 （万）
total_mv	float	总市值 （万元）
circ_mv	float	流通市值（万元）
'''
# 导入tushare
import tushare as ts
# 初始化pro接口
pro = ts.pro_api('')

# 拉取数据
def fetch_daily_basics(ts_code=None, trade_date=None, start_date=None, end_date=None):
    params = {
        "ts_code": ts_code,
        "trade_date": trade_date,
        "start_date": start_date,
        "end_date": end_date,
        "limit": "",
        "offset": ""
    }
    # 移除值为None的参数，以使用Tushare接口的默认值
    params = {k: v for k, v in params.items() if v is not None}

    df = pro.daily_basic(**params, fields=[
        "ts_code",
        "trade_date",
        "close",
        "turnover_rate",
        "turnover_rate_f",
        "volume_ratio",
        "pe",
        "pe_ttm",
        "pb",
        "ps",
        "ps_ttm",
        "dv_ratio",
        "dv_ttm",
        "total_share",
        "float_share",
        "free_share",
        "total_mv",
        "circ_mv"
    ])
    return df

if __name__ == '__main__':
    # 测试用例1: 获取单个股票在一段时间内的数据
    print("测试用例1: 获取 '000001.SZ' 从 20230101 到 20230110 的数据")
    df_stock_period = fetch_daily_basics(ts_code='000001.SZ', start_date='20230101', end_date='20230110')
    if df_stock_period is not None and not df_stock_period.empty:
        print(f"获取到数据 {df_stock_period.shape[0]} 条")
        print(df_stock_period.head())
    else:
        print("未获取到数据或返回为空")
    print("-" * 50)

    # 测试用例2: 获取特定交易日所有股票的数据
    # 注意：Tushare接口对单次返回数据量有限制，基础积分用户可能无法一次获取所有股票数据
    # 这里我们尝试获取一个交易日的数据，并观察返回的条数
    print("测试用例2: 获取 20230105 当天的所有股票数据")
    df_tradedate = fetch_daily_basics(trade_date='20230105')
    if df_tradedate is not None and not df_tradedate.empty:
        print(f"获取到数据 {df_tradedate.shape[0]} 条")
        print(df_tradedate.head())
        # 检查是否真的只返回了约6000条数据
        if 5000 <= df_tradedate.shape[0] <= 7000 : # Tushare pro API 默认分页是5000 或 6000
             print(f"注意: 获取到的数据条数为 {df_tradedate.shape[0]}，这可能受到了Tushare API单次查询上限的影响。")
             print("Tushare API对单次查询返回的数据量有限制 (通常是5000或6000条)。")
             print("如果需要获取更多数据，需要进行分页查询，即多次调用接口并调整 limit 和 offset 参数。")
             print("当前的 fetch_daily_basics 函数没有实现分页逻辑。")
    else:
        print("未获取到数据或返回为空")
    print("-" * 50)

    # 测试用例3: 获取一个较长的时间跨度，观察数据条数
    print("测试用例3: 获取 '000001.SZ' 从 20220101 到 20231231 的数据")
    df_long_period = fetch_daily_basics(ts_code='000001.SZ', start_date='20220101', end_date='20231231')
    if df_long_period is not None and not df_long_period.empty:
        print(f"获取到数据 {df_long_period.shape[0]} 条")
        print(df_long_period.head())
        actual_trading_days_approx = 250 * 2 # 大约一年的交易日数量
        if df_long_period.shape[0] < actual_trading_days_approx - 50: # 允许一些误差
             print(f"注意: 获取到的数据条数为 {df_long_period.shape[0]}，对于两年的数据来说可能偏少。")
             print("这也可能受到了Tushare API单次查询上限的影响，或者该股票在该期间有较多停牌。")
    else:
        print("未获取到数据或返回为空")
    print("-" * 50)

    # 关于数据量问题：
    # Tushare的pro.daily_basic接口文档中提到，单次提取最多5000条记录（具体以官网为准）。
    # 如果您期望获取超过这个数量的数据，比如全市场某一天的数据（A股大约5000多支股票），
    # 或者单支股票长时间跨度的数据，就需要进行分页查询。
    # 本函数 `fetch_daily_basics` 目前没有实现分页逻辑。
    # 如果您的 `mysql.py` 中调用此函数期望获取大量数据，这可能是数据量不足6000条的原因。
    #
    # 关于日期问题：
    # 请您运行此脚本，并观察打印出来的trade_date是否与您预期的日期一致。
