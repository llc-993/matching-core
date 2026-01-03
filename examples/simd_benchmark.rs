use matching_core::api::*;
use matching_core::core::orderbook::{OrderBook, DirectOrderBookOptimized};
use std::time::Instant;

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
    println!("=== SIMD 批量撮合性能测试 ===\n");

    let num_orders = 50000;

    // 测试 1: 启用 SIMD
    println!("测试 1: SIMD 优化启用");
    let start = Instant::now();
    let mut book_simd = DirectOrderBookOptimized::new(create_symbol_spec());
    book_simd.set_simd_enabled(true);
    
    for i in 0..num_orders {
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
        book_simd.new_order(&mut cmd);
    }
    
    let simd_time = start.elapsed();
    println!("  完成时间: {:?}", simd_time);
    println!("  TPS: {:.2}\n", num_orders as f64 / simd_time.as_secs_f64());

    // 测试 2: 禁用 SIMD
    println!("测试 2: SIMD 优化禁用");
    let start = Instant::now();
    let mut book_no_simd = DirectOrderBookOptimized::new(create_symbol_spec());
    book_no_simd.set_simd_enabled(false);
    
    for i in 0..num_orders {
        let mut cmd = OrderCommand {
            uid: 1,
            order_id: (num_orders + i) as u64,
            symbol: 1,
            price: 10000 + (i % 100) as i64,
            size: 10,
            action: if i % 2 == 0 { OrderAction::Ask } else { OrderAction::Bid },
            order_type: OrderType::Gtc,
            reserve_price: 10000 + (i % 100) as i64,
            timestamp: 1000,
            ..Default::default()
        };
        book_no_simd.new_order(&mut cmd);
    }
    
    let no_simd_time = start.elapsed();
    println!("  完成时间: {:?}", no_simd_time);
    println!("  TPS: {:.2}\n", num_orders as f64 / no_simd_time.as_secs_f64());

    // 性能提升
    let speedup = no_simd_time.as_secs_f64() / simd_time.as_secs_f64();
    println!("=== 性能提升 ===");
    println!("SIMD 优化提升: {:.2}x", speedup);
    
    if speedup > 1.0 {
        println!("✓ SIMD 优化生效，性能提升 {:.1}%", (speedup - 1.0) * 100.0);
    } else {
        println!("注意：SIMD 优化在小批量场景下可能无明显提升");
    }
}

