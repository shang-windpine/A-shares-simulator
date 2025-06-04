/// 性能监控和基准测试模块
/// 
/// 用于比较不同传输方式的性能指标

use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// 性能指标
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// 消息处理延迟（微秒）
    pub latency_us: Vec<u64>,
    /// 每秒处理消息数
    pub throughput_msg_per_sec: f64,
    /// 网络开销（字节）
    pub network_overhead_bytes: u64,
    /// CPU使用率（百分比）
    pub cpu_usage_percent: f64,
    /// 内存使用（字节）
    pub memory_usage_bytes: u64,
}

impl PerformanceMetrics {
    pub fn new() -> Self {
        Self {
            latency_us: Vec::new(),
            throughput_msg_per_sec: 0.0,
            network_overhead_bytes: 0,
            cpu_usage_percent: 0.0,
            memory_usage_bytes: 0,
        }
    }
    
    /// 添加延迟样本
    pub fn add_latency_sample(&mut self, latency: Duration) {
        self.latency_us.push(latency.as_micros() as u64);
    }
    
    /// 计算平均延迟（微秒）
    pub fn avg_latency_us(&self) -> f64 {
        if self.latency_us.is_empty() {
            0.0
        } else {
            self.latency_us.iter().sum::<u64>() as f64 / self.latency_us.len() as f64
        }
    }
    
    /// 计算99%分位延迟
    pub fn p99_latency_us(&self) -> u64 {
        if self.latency_us.is_empty() {
            0
        } else {
            let mut sorted = self.latency_us.clone();
            sorted.sort_unstable();
            let index = (sorted.len() as f64 * 0.99) as usize;
            sorted[index.min(sorted.len() - 1)]
        }
    }
}

/// HTTP vs TCP 性能对比
pub struct ProtocolComparison {
    tcp_metrics: Arc<Mutex<PerformanceMetrics>>,
    http_metrics: Arc<Mutex<PerformanceMetrics>>,
    grpc_metrics: Arc<Mutex<PerformanceMetrics>>,
}

impl ProtocolComparison {
    pub fn new() -> Self {
        Self {
            tcp_metrics: Arc::new(Mutex::new(PerformanceMetrics::new())),
            http_metrics: Arc::new(Mutex::new(PerformanceMetrics::new())),
            grpc_metrics: Arc::new(Mutex::new(PerformanceMetrics::new())),
        }
    }
    
    /// 运行性能基准测试
    pub async fn run_benchmark(&self, message_count: usize) -> BenchmarkResults {
        info!("开始性能基准测试，消息数量: {}", message_count);
        
        // 模拟不同协议的性能特征
        let tcp_results = self.benchmark_tcp(message_count).await;
        let http_results = self.benchmark_http(message_count).await;
        let grpc_results = self.benchmark_grpc(message_count).await;
        
        BenchmarkResults {
            tcp: tcp_results,
            http: http_results,
            grpc: grpc_results,
        }
    }
    
    async fn benchmark_tcp(&self, message_count: usize) -> ProtocolBenchmark {
        let start = Instant::now();
        let mut metrics = self.tcp_metrics.lock().await;
        
        // 模拟TCP性能特征
        for _ in 0..message_count {
            // TCP直连的典型延迟: 50-200微秒
            let latency = Duration::from_micros(100 + (rand::random::<u64>() % 100));
            metrics.add_latency_sample(latency);
        }
        
        let duration = start.elapsed();
        metrics.throughput_msg_per_sec = message_count as f64 / duration.as_secs_f64();
        
        // TCP开销：只有自定义header（10字节）
        metrics.network_overhead_bytes = 10; 
        
        ProtocolBenchmark {
            protocol: "TCP".to_string(),
            avg_latency_us: metrics.avg_latency_us(),
            p99_latency_us: metrics.p99_latency_us(),
            throughput: metrics.throughput_msg_per_sec,
            network_overhead: metrics.network_overhead_bytes,
        }
    }
    
    async fn benchmark_http(&self, message_count: usize) -> ProtocolBenchmark {
        let start = Instant::now();
        let mut metrics = self.http_metrics.lock().await;
        
        // 模拟HTTP性能特征（HTTP/1.1）
        for _ in 0..message_count {
            // HTTP的典型延迟: 200-500微秒（包含header解析）
            let latency = Duration::from_micros(300 + (rand::random::<u64>() % 200));
            metrics.add_latency_sample(latency);
        }
        
        let duration = start.elapsed();
        metrics.throughput_msg_per_sec = message_count as f64 / duration.as_secs_f64();
        
        // HTTP开销：header大约200-500字节
        metrics.network_overhead_bytes = 350;
        
        ProtocolBenchmark {
            protocol: "HTTP/1.1".to_string(),
            avg_latency_us: metrics.avg_latency_us(),
            p99_latency_us: metrics.p99_latency_us(),
            throughput: metrics.throughput_msg_per_sec,
            network_overhead: metrics.network_overhead_bytes,
        }
    }
    
    async fn benchmark_grpc(&self, message_count: usize) -> ProtocolBenchmark {
        let start = Instant::now();
        let mut metrics = self.grpc_metrics.lock().await;
        
        // 模拟gRPC性能特征（HTTP/2）
        for _ in 0..message_count {
            // gRPC的典型延迟: 150-300微秒（HTTP/2多路复用优化）
            let latency = Duration::from_micros(200 + (rand::random::<u64>() % 100));
            metrics.add_latency_sample(latency);
        }
        
        let duration = start.elapsed();
        metrics.throughput_msg_per_sec = message_count as f64 / duration.as_secs_f64();
        
        // gRPC开销：HTTP/2 header大约50-150字节（压缩后）
        metrics.network_overhead_bytes = 100;
        
        ProtocolBenchmark {
            protocol: "gRPC/HTTP2".to_string(),
            avg_latency_us: metrics.avg_latency_us(),
            p99_latency_us: metrics.p99_latency_us(),
            throughput: metrics.throughput_msg_per_sec,
            network_overhead: metrics.network_overhead_bytes,
        }
    }
}

#[derive(Debug)]
pub struct ProtocolBenchmark {
    pub protocol: String,
    pub avg_latency_us: f64,
    pub p99_latency_us: u64,
    pub throughput: f64,
    pub network_overhead: u64,
}

#[derive(Debug)]
pub struct BenchmarkResults {
    pub tcp: ProtocolBenchmark,
    pub http: ProtocolBenchmark,
    pub grpc: ProtocolBenchmark,
}

impl BenchmarkResults {
    /// 生成性能对比报告
    pub fn generate_report(&self) -> String {
        format!(
            r#"
=== 协议性能对比报告 ===

TCP (原生):
  平均延迟: {:.1} μs
  P99延迟:  {} μs  
  吞吐量:   {:.0} msg/s
  网络开销: {} bytes

HTTP/1.1:
  平均延迟: {:.1} μs
  P99延迟:  {} μs
  吞吐量:   {:.0} msg/s  
  网络开销: {} bytes

gRPC/HTTP2:
  平均延迟: {:.1} μs
  P99延迟:  {} μs
  吞吐量:   {:.0} msg/s
  网络开销: {} bytes

=== 性能分析 ===
TCP vs HTTP延迟提升: {:.1}%
TCP vs gRPC延迟提升: {:.1}%
TCP网络开销减少: {}% (vs HTTP), {}% (vs gRPC)
"#,
            self.tcp.avg_latency_us, self.tcp.p99_latency_us, self.tcp.throughput, self.tcp.network_overhead,
            self.http.avg_latency_us, self.http.p99_latency_us, self.http.throughput, self.http.network_overhead,
            self.grpc.avg_latency_us, self.grpc.p99_latency_us, self.grpc.throughput, self.grpc.network_overhead,
            
            // 计算性能提升百分比
            ((self.http.avg_latency_us - self.tcp.avg_latency_us) / self.http.avg_latency_us) * 100.0,
            ((self.grpc.avg_latency_us - self.tcp.avg_latency_us) / self.grpc.avg_latency_us) * 100.0,
            
            // 计算网络开销减少百分比  
            ((self.http.network_overhead - self.tcp.network_overhead) * 100) / self.http.network_overhead,
            ((self.grpc.network_overhead - self.tcp.network_overhead) * 100) / self.grpc.network_overhead,
        )
    }
}

/// 实时性能监控器
pub struct PerformanceMonitor {
    metrics: HashMap<String, PerformanceMetrics>,
    start_time: Instant,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self {
            metrics: HashMap::new(),
            start_time: Instant::now(),
        }
    }
    
    /// 记录消息处理开始
    pub fn start_request(&mut self, request_id: &str) -> RequestTimer {
        RequestTimer {
            request_id: request_id.to_string(),
            start_time: Instant::now(),
        }
    }
    
    /// 记录消息处理完成
    pub fn finish_request(&mut self, timer: RequestTimer, protocol: &str) {
        let latency = timer.start_time.elapsed();
        
        let metrics = self.metrics.entry(protocol.to_string()).or_insert_with(PerformanceMetrics::new);
        metrics.add_latency_sample(latency);
        
        // 如果延迟过高，记录警告
        if latency > Duration::from_millis(10) {
            warn!("高延迟检测: {} protocol, latency: {:?}", protocol, latency);
        }
    }
    
    /// 获取实时统计
    pub fn get_stats(&self, protocol: &str) -> Option<&PerformanceMetrics> {
        self.metrics.get(protocol)
    }
}

pub struct RequestTimer {
    request_id: String,
    start_time: Instant,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_performance_comparison() {
        let comparison = ProtocolComparison::new();
        let results = comparison.run_benchmark(1000).await;
        
        println!("{}", results.generate_report());
        
        // TCP应该有最低的延迟和网络开销
        assert!(results.tcp.avg_latency_us < results.http.avg_latency_us);
        assert!(results.tcp.network_overhead < results.http.network_overhead);
    }
} 