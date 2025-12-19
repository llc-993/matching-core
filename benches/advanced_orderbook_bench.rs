use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use matching_core::api::*;
use matching_core::core::orderbook::{OrderBook, AdvancedOrderBook};

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

fn bench_post_only_orders(c: &mut Criterion) {
    let mut group = c.benchmark_group("post_only");
    
    for size in [100, 1000, 10000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                let mut book = AdvancedOrderBook::new(create_symbol_spec());
                
                // 挂卖单
                for i in 0..size {
                    let mut cmd = OrderCommand {
                        uid: 1,
                        order_id: i as u64,
                        symbol: 1,
                        price: 10000 + i,
                        size: 10,
                        action: OrderAction::Ask,
                        order_type: OrderType::Gtc,
                        reserve_price: 10000 + i,
                        timestamp: 1000,
                        ..Default::default()
                    };
                    book.new_order(&mut cmd);
                }
                
                // Post-Only 买单
                for i in 0..size {
                    let mut cmd = OrderCommand {
                        uid: 2,
                        order_id: (size + i) as u64,
                        symbol: 1,
                        price: 9999 - i,
                        size: 10,
                        action: OrderAction::Bid,
                        order_type: OrderType::PostOnly,
                        reserve_price: 9999 - i,
                        timestamp: 1001,
                        ..Default::default()
                    };
                    book.new_order(&mut cmd);
                }
                
                black_box(book);
            });
        });
    }
    
    group.finish();
}

fn bench_iceberg_orders(c: &mut Criterion) {
    let mut group = c.benchmark_group("iceberg");
    
    for size in [100, 1000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                let mut book = AdvancedOrderBook::new(create_symbol_spec());
                
                // 挂冰山卖单
                for i in 0..size {
                    let mut cmd = OrderCommand {
                        uid: 1,
                        order_id: i as u64,
                        symbol: 1,
                        price: 10000,
                        size: 100,
                        action: OrderAction::Ask,
                        order_type: OrderType::Iceberg,
                        reserve_price: 10000,
                        timestamp: 1000,
                        visible_size: Some(10),
                        ..Default::default()
                    };
                    book.new_order(&mut cmd);
                }
                
                // 买单成交
                for i in 0..size {
                    let mut cmd = OrderCommand {
                        uid: 2,
                        order_id: (size + i) as u64,
                        symbol: 1,
                        price: 10000,
                        size: 10,
                        action: OrderAction::Bid,
                        order_type: OrderType::Ioc,
                        reserve_price: 10000,
                        timestamp: 1001,
                        ..Default::default()
                    };
                    book.new_order(&mut cmd);
                }
                
                black_box(book);
            });
        });
    }
    
    group.finish();
}

fn bench_fok_orders(c: &mut Criterion) {
    let mut group = c.benchmark_group("fok");
    
    for size in [100, 1000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                let mut book = AdvancedOrderBook::new(create_symbol_spec());
                
                // 挂卖单
                for i in 0..size {
                    let mut cmd = OrderCommand {
                        uid: 1,
                        order_id: i as u64,
                        symbol: 1,
                        price: 10000,
                        size: 10,
                        action: OrderAction::Ask,
                        order_type: OrderType::Gtc,
                        reserve_price: 10000,
                        timestamp: 1000,
                        ..Default::default()
                    };
                    book.new_order(&mut cmd);
                }
                
                // FOK 买单
                for i in 0..size {
                    let mut cmd = OrderCommand {
                        uid: 2,
                        order_id: (size + i) as u64,
                        symbol: 1,
                        price: 10000,
                        size: 5,
                        action: OrderAction::Bid,
                        order_type: OrderType::Fok,
                        reserve_price: 10000,
                        timestamp: 1001,
                        ..Default::default()
                    };
                    book.new_order(&mut cmd);
                }
                
                black_box(book);
            });
        });
    }
    
    group.finish();
}

fn bench_gtd_orders(c: &mut Criterion) {
    let mut group = c.benchmark_group("gtd");
    
    for size in [100, 1000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                let mut book = AdvancedOrderBook::new(create_symbol_spec());
                
                // 挂 GTD 卖单
                for i in 0..size {
                    let mut cmd = OrderCommand {
                        uid: 1,
                        order_id: i as u64,
                        symbol: 1,
                        price: 10000,
                        size: 10,
                        action: OrderAction::Ask,
                        order_type: OrderType::Gtd(2000),
                        reserve_price: 10000,
                        timestamp: 1000,
                        expire_time: Some(2000),
                        ..Default::default()
                    };
                    book.new_order(&mut cmd);
                }
                
                // 部分成交
                for i in 0..size / 2 {
                    let mut cmd = OrderCommand {
                        uid: 2,
                        order_id: (size + i) as u64,
                        symbol: 1,
                        price: 10000,
                        size: 5,
                        action: OrderAction::Bid,
                        order_type: OrderType::Ioc,
                        reserve_price: 10000,
                        timestamp: 1500,
                        ..Default::default()
                    };
                    book.new_order(&mut cmd);
                }
                
                // 过期后尝试成交
                for i in 0..size / 2 {
                    let mut cmd = OrderCommand {
                        uid: 2,
                        order_id: (size + size / 2 + i) as u64,
                        symbol: 1,
                        price: 10000,
                        size: 5,
                        action: OrderAction::Bid,
                        order_type: OrderType::Ioc,
                        reserve_price: 10000,
                        timestamp: 2500,
                        ..Default::default()
                    };
                    book.new_order(&mut cmd);
                }
                
                black_box(book);
            });
        });
    }
    
    group.finish();
}

fn bench_stop_orders(c: &mut Criterion) {
    let mut group = c.benchmark_group("stop_orders");
    
    for size in [100, 1000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                let mut book = AdvancedOrderBook::new(create_symbol_spec());
                
                // 下止损单
                for i in 0..size {
                    let mut cmd = OrderCommand {
                        uid: 1,
                        order_id: i as u64,
                        symbol: 1,
                        price: 10500,
                        size: 10,
                        action: OrderAction::Bid,
                        order_type: OrderType::StopLimit,
                        reserve_price: 10500,
                        timestamp: 1000,
                        stop_price: Some(10300),
                        ..Default::default()
                    };
                    book.new_order(&mut cmd);
                }
                
                black_box(book);
            });
        });
    }
    
    group.finish();
}

fn bench_mixed_orders(c: &mut Criterion) {
    c.bench_function("mixed_orders_1000", |b| {
        b.iter(|| {
            let mut book = AdvancedOrderBook::new(create_symbol_spec());
            
            // 混合订单类型
            for i in 0..250 {
                // GTC
                let mut gtc = OrderCommand {
                    uid: 1,
                    order_id: i as u64,
                    symbol: 1,
                    price: 10000 + i,
                    size: 10,
                    action: OrderAction::Ask,
                    order_type: OrderType::Gtc,
                    reserve_price: 10000 + i,
                    timestamp: 1000,
                    ..Default::default()
                };
                book.new_order(&mut gtc);
                
                // Iceberg
                let mut iceberg = OrderCommand {
                    uid: 1,
                    order_id: (250 + i) as u64,
                    symbol: 1,
                    price: 10000 + i,
                    size: 100,
                    action: OrderAction::Ask,
                    order_type: OrderType::Iceberg,
                    reserve_price: 10000 + i,
                    timestamp: 1000,
                    visible_size: Some(10),
                    ..Default::default()
                };
                book.new_order(&mut iceberg);
                
                // GTD
                let mut gtd = OrderCommand {
                    uid: 1,
                    order_id: (500 + i) as u64,
                    symbol: 1,
                    price: 10000 + i,
                    size: 15,
                    action: OrderAction::Ask,
                    order_type: OrderType::Gtd(2000),
                    reserve_price: 10000 + i,
                    timestamp: 1000,
                    expire_time: Some(2000),
                    ..Default::default()
                };
                book.new_order(&mut gtd);
                
                // Post-Only
                let mut post_only = OrderCommand {
                    uid: 2,
                    order_id: (750 + i) as u64,
                    symbol: 1,
                    price: 9999 - i,
                    size: 10,
                    action: OrderAction::Bid,
                    order_type: OrderType::PostOnly,
                    reserve_price: 9999 - i,
                    timestamp: 1001,
                    ..Default::default()
                };
                book.new_order(&mut post_only);
            }
            
            black_box(book);
        });
    });
}

criterion_group!(
    benches,
    bench_post_only_orders,
    bench_iceberg_orders,
    bench_fok_orders,
    bench_gtd_orders,
    bench_stop_orders,
    bench_mixed_orders
);

criterion_main!(benches);

