use matching_core::api::*;
use matching_core::core::orderbook::{OrderBook, AdvancedOrderBook};
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

fn main() {
    println!("=== 生成基准测试数据 ===\n");
    
    let mut file = File::create("benchmark_results.csv").unwrap();
    writeln!(file, "Orders,TPS,QPS,Memory_MB,Duration_MS").unwrap();
    
    let sizes = vec![1000, 5000, 10000, 50000, 100000];
    
    for &size in &sizes {
        println!("测试规模: {} 订单", size);
        
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
        let memory = estimate_memory(size);
        let duration_ms = duration.as_secs_f64() * 1000.0;
        
        writeln!(file, "{},{:.2},{:.2},{:.2},{:.2}", 
            size, tps, qps, memory, duration_ms).unwrap();
        
        println!("  TPS: {:.2}, QPS: {:.2}, 内存: {:.2} MB, 耗时: {:.2} ms\n",
            tps, qps, memory, duration_ms);
    }
    
    println!("数据已保存到 benchmark_results.csv");
    println!("运行 'python3 scripts/plot_benchmark.py' 生成图表");
}

fn estimate_memory(orders: usize) -> f64 {
    // 估算内存使用（MB）
    // AdvancedOrderBook 每个订单约 200 字节
    let bytes_per_order = 200;
    (orders * bytes_per_order) as f64 / (1024.0 * 1024.0)
}

