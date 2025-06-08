# 这是一个测试注释，用于检查文件修改是否正常工作
'''
Created on 2020年1月30日

@author: JM
'''
import pandas as pd
import tushare as ts # 新增导入 tushare
import time # 新增导入 time
from sqlalchemy import create_engine
import argparse # 新增导入

# 从各个数据获取脚本中导入获取数据的函数
from .get_all_stocks_basic_info import fetch_all_stocks_basic_info
from .get_daily_basics import fetch_daily_basics
from .get_daily_trade_info import fetch_daily_trade_info
from .get_week_month_adj import fetch_week_month_adj

# Tushare Token (请确保这是有效的)
TUSHARE_TOKEN = ''

# 初始化数据库连接引擎
# 请根据您的实际数据库配置修改连接字符串，特别是 user, mima 和 demos
# 例如: 'mysql+mysqlconnector://username:password@host:port/database_name?charset=utf8'
# 如果密码为空，可以这样写：
engine_ts = create_engine('mysql://root:@127.0.0.1:3306/a_shares_real_history?charset=utf8&use_unicode=1')

def get_trade_dates(start_date_str, end_date_str):
    """获取指定范围内的交易日历"""
    try:
        # 初始化Tushare Pro接口。每个调用此函数的函数都独立初始化，以避免共享状态问题。
        # 如果有全局pro实例，可以考虑使用它。
        local_pro = ts.pro_api(TUSHARE_TOKEN)
        df_cal = local_pro.trade_cal(exchange='', start_date=start_date_str, end_date=end_date_str, is_open='1')
        if df_cal is not None and not df_cal.empty:
            return df_cal['cal_date'].tolist()
        else:
            print(f"未能从Tushare获取 {start_date_str} 到 {end_date_str} 的交易日历。")
            return []
    except Exception as e:
        print(f"获取交易日历时发生错误: {e}")
        return []

def write_stocks_basic_info(df):
    """将股票基础信息写入数据库"""
    try:
        df.to_sql('stocks_basic_info', engine_ts, index=False, if_exists='append', chunksize=5000)
        print("股票基础信息已成功写入数据库。")
    except Exception as e:
        print(f"写入股票基础信息到数据库失败: {e}")

def write_daily_basics(df):
    """将每日基本指标写入数据库"""
    try:
        df.to_sql('daily_basics', engine_ts, index=False, if_exists='append', chunksize=5000)
        print("每日基本指标已成功写入数据库。")
    except Exception as e:
        print(f"写入每日基本指标到数据库失败: {e}")

def write_daily_trade_info(df):
    """将每日交易信息写入数据库"""
    try:
        df.to_sql('daily_trade_info', engine_ts, index=False, if_exists='append', chunksize=5000)
        print("每日交易信息已成功写入数据库。")
    except Exception as e:
        print(f"写入每日交易信息到数据库失败: {e}")

def write_week_month_adj_info(df):
    """将周/月行情调整数据写入数据库"""
    try:
        df.to_sql('week_month_adj_info', engine_ts, index=False, if_exists='append', chunksize=5000)
        print("周/月行情调整数据已成功写入数据库。")
    except Exception as e:
        print(f"写入周/月行情调整数据到数据库失败: {e}")

# 新增：通用的按日数据处理函数
def _process_daily_data_by_date(
    data_name: str,
    fetch_function, # Callable[[str], Optional[pd.DataFrame]]
    write_function, # Callable[[pd.DataFrame], None]
    overall_start_date: str,
    overall_end_date: str,
    api_calls_per_minute_limit: int,
    sleep_interval_after_fetch: float
):
    """
    通用的按日分批数据处理函数。

    参数:
    data_name (str): 数据类型的描述性名称 (例如 "每日基本指标")。
    fetch_function (Callable): 用于获取单日数据的函数，接受 trade_date_str 作为参数。
    write_function (Callable): 用于将获取到的DataFrame写入数据库的函数。
    overall_start_date (str): 处理的总体开始日期 (YYYYMMDD)。
    overall_end_date (str): 处理的总体结束日期 (YYYYMMDD)。
    api_calls_per_minute_limit (int): 每分钟API调用次数上限。
    sleep_interval_after_fetch (float): 每次成功获取数据后休眠的秒数。
    """
    print(f"开始处理{data_name} (分批按日获取)...")

    trade_dates = get_trade_dates(overall_start_date, overall_end_date)
    if not trade_dates:
        print(f"未能获取到交易日历，{data_name}处理终止。")
        return

    print(f"共获取到 {len(trade_dates)} 个交易日 ({overall_start_date} 至 {overall_end_date})，将逐日获取{data_name}。")

    api_call_count = 0
    minute_start_time = time.time()
    total_records_written = 0

    for i, trade_date_str in enumerate(trade_dates):
        # API 频率控制
        if api_call_count >= api_calls_per_minute_limit:
            elapsed_time = time.time() - minute_start_time
            if elapsed_time < 60:
                sleep_duration = 60 - elapsed_time
                print(f"已达API调用频率上限 ({api_calls_per_minute_limit}次/分钟) 获取{data_name}，休眠 {sleep_duration:.2f} 秒...")
                time.sleep(sleep_duration)
            api_call_count = 0
            minute_start_time = time.time()

        print(f"正在获取 {trade_date_str} 的{data_name}数据 (进度: {i+1}/{len(trade_dates)})...")
        try:
            # 注意：假设 fetch_function 在内部处理 Tushare API 初始化和 token
            df = fetch_function(trade_date=trade_date_str)
            api_call_count += 1 # 无论成功与否，只要调用了就计数

            if df is not None and not df.empty:
                write_function(df)
                print(f"已写入 {trade_date_str} 的 {df.shape[0]} 条{data_name}数据。")
                total_records_written += df.shape[0]
            else:
                # 如果 fetch_function 内部已经打印了获取失败或数据为空的信息，这里可以简化
                print(f"未能获取到 {trade_date_str} 的{data_name}数据，或数据为空。")

            time.sleep(sleep_interval_after_fetch)
        except Exception as e:
            print(f"处理 {trade_date_str} {data_name}数据时发生错误: {e}")
            # 发生错误也计入API调用（如果调用已发出），并稍作等待
            time.sleep(1) # 如果API调用中出错，稍微等待长一点

    print(f"{data_name}数据处理完毕。总计写入 {total_records_written} 条记录。")

def process_stocks_basic_info():
    """获取并写入股票基础信息"""
    print("开始处理股票基础信息...")
    df = fetch_all_stocks_basic_info()
    if df is not None and not df.empty:
        write_stocks_basic_info(df)
    else:
        print("未能获取到股票基础信息数据，或数据为空。")

def process_daily_basics():
    """获取并写入每日基本指标 (按日分批)"""
    _process_daily_data_by_date(
        data_name="每日基本指标",
        fetch_function=fetch_daily_basics,
        write_function=write_daily_basics,
        overall_start_date='20211224',
        overall_end_date='20211224', 
        api_calls_per_minute_limit=190, 
        sleep_interval_after_fetch=0.32 
    )

def process_daily_trade_info():
    """获取并写入每日交易信息 (按日分批)"""
    _process_daily_data_by_date(
        data_name="每日交易信息",
        fetch_function=fetch_daily_trade_info,
        write_function=write_daily_trade_info,
        overall_start_date='20190101', 
        overall_end_date='20240923', 
        api_calls_per_minute_limit=480, 
        sleep_interval_after_fetch=0.13
    )

# 新增：用于按日获取周线数据的包装函数
def fetch_week_adj_for_trade_date(trade_date: str):
    """获取指定单日所在周的复权行情数据"""
    # 注意：Tushare的 stk_week_month_adj 接口使用 trade_date 参数时，
    # 会返回该日期所在周/月的数据。对于周线，它返回的是该周最后一天的日期作为 trade_date。
    # 确保这里的调用符合 stk_week_month_adj 接口对 trade_date 的预期。
    return fetch_week_month_adj(trade_date=trade_date, freq='week')

# 新增：用于按日获取月线数据的包装函数
def fetch_month_adj_for_trade_date(trade_date: str):
    """获取指定单日所在月的复权行情数据"""
    return fetch_week_month_adj(trade_date=trade_date, freq='month')

def process_week_month_adj_info():
    """获取并写入周/月行情调整数据 (按日分批)"""
    # 处理周线数据
    _process_daily_data_by_date(
        data_name="周复权行情",
        fetch_function=fetch_week_adj_for_trade_date, 
        write_function=write_week_month_adj_info, 
        overall_start_date='20190101', # 请根据需要修改起止日期
        overall_end_date='20240923',   # 请根据需要修改起止日期
        api_calls_per_minute_limit=190, # stk_week_month_adj 接口限制为200次/分钟 (与adj_factor类似)
        sleep_interval_after_fetch=0.32  
    )
    # 处理月线数据
    _process_daily_data_by_date(
        data_name="月复权行情",
        fetch_function=fetch_month_adj_for_trade_date, 
        write_function=write_week_month_adj_info, 
        overall_start_date='20190101', # 请根据需要修改起止日期
        overall_end_date='20240923',   # 请根据需要修改起止日期
        api_calls_per_minute_limit=190, # stk_week_month_adj 接口限制为200次/分钟
        sleep_interval_after_fetch=0.32  
    )

# 移除了原有的 read_data, write_data, get_data 函数
# 移除了 if __name__ == '__main__': 部分

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description="获取并写入指定的A股市场数据到数据库")
    parser.add_argument(
        '--table', 
        type=str, 
        required=True, 
        choices=[
            'stocks_basic_info', 
            'daily_basics', 
            'daily_trade_info', 
            'week_month_adj_info',
            'all'
        ],
        help="指定要处理的数据表: 'stocks_basic_info', 'daily_basics', 'daily_trade_info', 'week_month_adj_info', 或 'all' 来处理所有表。"
    )
    
    args = parser.parse_args()
    
    if args.table == 'stocks_basic_info':
        process_stocks_basic_info()
    elif args.table == 'daily_basics':
        process_daily_basics()
    elif args.table == 'daily_trade_info':
        process_daily_trade_info()
    elif args.table == 'week_month_adj_info':
        process_week_month_adj_info()
    elif args.table == 'all':
        print("开始处理所有表...")
        process_stocks_basic_info()
        process_daily_basics()
        process_daily_trade_info()
        process_week_month_adj_info()
        print("所有表处理完毕。")
    else:
        print(f"未知的表名: {args.table}")
        parser.print_help()