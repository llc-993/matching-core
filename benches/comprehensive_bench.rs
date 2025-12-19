use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use matching_core::api::*;
use matching_core::core::orderbook::{OrderBook, AdvancedOrderBook, DirectOrderBookOptimized, NaiveOrderBook};
use std::time::Instant;
use std::fs::File;
use std::io::Write;

fn create_symbol_spec() -> CoreSymbolSpecification {
    CoreSymbolSpecification {
        symbol_id: 1,
        symbol_type: SymbolType::CurrencyExchangePair,
        base_currency: 0,
        quote_currency: 1,
        base_scale_k: 1,
        quote_scale_k: 1,
        taker_fee: 0,
        maker_fee: 0,
        margin_buy: 0,
        margin_sell: 0,
    }
}

struct BenchmarkResult {
    name: String,
    orders: usize,
    tps: f64,
    qps: f64,
    memory_mb: f64,
    duration_ms: f64,
}

fn measure_memory() -> f64 {
    // 简化版本：使用系统信息
    // 实际应该使用更精确的内存测量工具
    #[cfg(target_os = "linux")]
    {
        if let Ok(contents) = std::fs::read_to_string("/proc/self/status") {
            for line in contents.lines() {
                if line.starts_with("VmRSS:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<f64>() {
                            return kb / 1024.0; // 转换为 MB
                        }
                    }
                }
            }
        }
    }
    // macOS 或其他系统返回估算值
    0.0
}

fn bench_comprehensive(c: &mut Criterion) {
    let mut results = Vec::new();
    
    let sizes = vec![1000, 5000, 10000, 50000, 100000];
    
    for &size in &sizes {
        // AdvancedOrderBook
        let mut group = c.benchmark_group("advanced_orderbook");
        group.throughput(Throughput::Elements(size as u64));
        
        group.bench_with_input(
            BenchmarkId::new("place_orders", size),
            &size,
            |b, &size| {
                b.iter_custom(|iters| {
                    let mut total_time = std::time::Duration::ZERO;
                    for _ in 0..iters {
                        let start = Instant::now();
                        let mut book = AdvancedOrderBook::new(create_symbol_spec());
                        
                        for i in 0..size {
                            let mut cmd = OrderCommand {
                                uid: 1,
                                order_id: i as u64,
                                symbol: 1,
                                price: 10000 + (i % 100) as i64,
                                size: 10,
                                action: if i % 2 == 0 { OrderAction::Ask } else { OrderAction::Bid },
                                order_type: OrderType::Gtc,
                                reserve_price: 10000 + (i % 100) as i64,
                                timestamp: 1000,
                                ..Default::default()
                            };
                            book.new_order(&mut cmd);
                        }
                        
                        total_time += start.elapsed();
                    }
                    total_time
                });
            },
        );
        
        group.finish();
        
        // 测量实际性能
        let start = Instant::now();
        let mut book = AdvancedOrderBook::new(create_symbol_spec());
        let mut trades = 0;
        
        for i in 0..size {
            let mut cmd = OrderCommand {
                uid: 1,
                order_id: i as u64,
                symbol: 1,
                price: 10000 + (i % 100) as i64,
                size: 10,
                action: if i % 2 == 0 { OrderAction::Ask } else { OrderAction::Bid },
                order_type: OrderType::Gtc,
                reserve_price: 10000 + (i % 100) as i64,
                timestamp: 1000,
                ..Default::default()
            };
            book.new_order(&mut cmd);
            trades += cmd.matcher_events.len();
        }
        
        let duration = start.elapsed();
        let tps = size as f64 / duration.as_secs_f64();
        let qps = trades as f64 / duration.as_secs_f64();
        let memory = measure_memory();
        
        results.push(BenchmarkResult {
            name: "AdvancedOrderBook".to_string(),
            orders: size,
            tps,
            qps,
            memory_mb: memory,
            duration_ms: duration.as_secs_f64() * 1000.0,
        });
    }
    
    // 生成 CSV 报告
    let mut file = File::create("benchmark_results.csv").unwrap();
    writeln!(file, "Name,Orders,TPS,QPS,Memory_MB,Duration_MS").unwrap();
    for r in &results {
        writeln!(file, "{},{},{:.2},{:.2},{:.2},{:.2}", 
            r.name, r.orders, r.tps, r.qps, r.memory_mb, r.duration_ms).unwrap();
    }
    
    // 生成 Python 脚本用于绘制图表
    let mut py_script = File::create("plot_benchmark.py").unwrap();
    writeln!(py_script, r#"
import matplotlib.pyplot as plt
import pandas as pd
import numpy as np

# 读取数据
df = pd.read_csv('benchmark_results.csv')

# 创建图表
fig, axes = plt.subplots(2, 2, figsize=(14, 10))
fig.suptitle('撮合引擎性能指标', fontsize=16, fontweight='bold')

# TPS 折线图
axes[0, 0].plot(df['Orders'], df['TPS'], marker='o', linewidth=2, markersize=8, color='#2E86AB')
axes[0, 0].set_xlabel('订单数量', fontsize=12)
axes[0, 0].set_ylabel('TPS (订单/秒)', fontsize=12)
axes[0, 0].set_title('吞吐量 (TPS)', fontsize=13, fontweight='bold')
axes[0, 0].grid(True, alpha=0.3)
axes[0, 0].set_xscale('log')

# QPS 折线图
axes[0, 1].plot(df['Orders'], df['QPS'], marker='s', linewidth=2, markersize=8, color='#A23B72')
axes[0, 1].set_xlabel('订单数量', fontsize=12)
axes[0, 1].set_ylabel('QPS (成交/秒)', fontsize=12)
axes[0, 1].set_title('成交速率 (QPS)', fontsize=13, fontweight='bold')
axes[0, 1].grid(True, alpha=0.3)
axes[0, 1].set_xscale('log')

# 内存使用折线图
axes[1, 0].plot(df['Orders'], df['Memory_MB'], marker='^', linewidth=2, markersize=8, color='#F18F01')
axes[1, 0].set_xlabel('订单数量', fontsize=12)
axes[1, 0].set_ylabel('内存使用 (MB)', fontsize=12)
axes[1, 0].set_title('内存占用', fontsize=13, fontweight='bold')
axes[1, 0].grid(True, alpha=0.3)
axes[1, 0].set_xscale('log')

# 延迟折线图
axes[1, 1].plot(df['Orders'], df['Duration_MS'], marker='d', linewidth=2, markersize=8, color='#C73E1D')
axes[1, 1].set_xlabel('订单数量', fontsize=12)
axes[1, 1].set_ylabel('处理时间 (毫秒)', fontsize=12)
axes[1, 1].set_title('延迟', fontsize=13, fontweight='bold')
axes[1, 1].grid(True, alpha=0.3)
axes[1, 1].set_xscale('log')

plt.tight_layout()
plt.savefig('benchmark_results.png', dpi=300, bbox_inches='tight')
print('图表已保存到 benchmark_results.png')
"#).unwrap();
}

fn bench_orderbook_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("orderbook_comparison");
    let size = 10000;
    group.throughput(Throughput::Elements(size as u64));
    
    // AdvancedOrderBook
    group.bench_function("AdvancedOrderBook", |b| {
        b.iter(|| {
            let mut book = AdvancedOrderBook::new(create_symbol_spec());
            for i in 0..size {
                let mut cmd = OrderCommand {
                    uid: 1,
                    order_id: i as u64,
                    symbol: 1,
                    price: 10000 + (i % 100) as i64,
                    size: 10,
                    action: if i % 2 == 0 { OrderAction::Ask } else { OrderAction::Bid },
                    order_type: OrderType::Gtc,
                    reserve_price: 10000 + (i % 100) as i64,
                    timestamp: 1000,
                    ..Default::default()
                };
                book.new_order(&mut cmd);
            }
        });
    });
    
    // DirectOrderBookOptimized
    group.bench_function("DirectOrderBookOptimized", |b| {
        b.iter(|| {
            let mut book = DirectOrderBookOptimized::new(create_symbol_spec());
            for i in 0..size {
                let mut cmd = OrderCommand {
                    uid: 1,
                    order_id: i as u64,
                    symbol: 1,
                    price: 10000 + (i % 100) as i64,
                    size: 10,
                    action: if i % 2 == 0 { OrderAction::Ask } else { OrderAction::Bid },
                    order_type: OrderType::Gtc,
                    reserve_price: 10000 + (i % 100) as i64,
                    timestamp: 1000,
                    ..Default::default()
                };
                book.new_order(&mut cmd);
            }
        });
    });
    
    // NaiveOrderBook
    group.bench_function("NaiveOrderBook", |b| {
        b.iter(|| {
            let mut book = NaiveOrderBook::new(create_symbol_spec());
            for i in 0..size {
                let mut cmd = OrderCommand {
                    uid: 1,
                    order_id: i as u64,
                    symbol: 1,
                    price: 10000 + (i % 100) as i64,
                    size: 10,
                    action: if i % 2 == 0 { OrderAction::Ask } else { OrderAction::Bid },
                    order_type: OrderType::Gtc,
                    reserve_price: 10000 + (i % 100) as i64,
                    timestamp: 1000,
                    ..Default::default()
                };
                book.new_order(&mut cmd);
            }
        });
    });
    
    group.finish();
}

criterion_group!(benches, bench_comprehensive, bench_orderbook_comparison);
criterion_main!(benches);

