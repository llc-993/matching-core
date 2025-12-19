use criterion::{black_box, criterion_group, criterion_main, Criterion};
use matching_core::api::*;
use matching_core::core::orderbook::{DirectOrderBook, DirectOrderBookOptimized, NaiveOrderBook, OrderBook};

fn bench_naive_orderbook(c: &mut Criterion) {
    let spec = CoreSymbolSpecification::default();
    let mut orderbook = NaiveOrderBook::new(spec);

    c.bench_function("NaiveOrderBook_GTC", |b| {
        b.iter(|| {
            let mut cmd = OrderCommand {
                command: OrderCommandType::PlaceOrder,
                uid: black_box(1001),
                order_id: black_box(5001),
                symbol: 100,
                price: black_box(100),
                size: black_box(10),
                action: OrderAction::Ask,
                order_type: OrderType::Gtc,
                ..Default::default()
            };
            orderbook.new_order(&mut cmd);
        });
    });
}

fn bench_direct_orderbook(c: &mut Criterion) {
    let spec = CoreSymbolSpecification::default();
    let mut orderbook = DirectOrderBook::new(spec);

    c.bench_function("DirectOrderBook_GTC", |b| {
        b.iter(|| {
            let mut cmd = OrderCommand {
                command: OrderCommandType::PlaceOrder,
                uid: black_box(1001),
                order_id: black_box(5001),
                symbol: 100,
                price: black_box(100),
                size: black_box(10),
                action: OrderAction::Ask,
                order_type: OrderType::Gtc,
                ..Default::default()
            };
            orderbook.new_order(&mut cmd);
        });
    });
}

fn bench_direct_optimized_orderbook(c: &mut Criterion) {
    let spec = CoreSymbolSpecification::default();
    let mut orderbook = DirectOrderBookOptimized::new(spec);

    c.bench_function("DirectOrderBookOptimized_GTC", |b| {
        b.iter(|| {
            let mut cmd = OrderCommand {
                command: OrderCommandType::PlaceOrder,
                uid: black_box(1001),
                order_id: black_box(5001),
                symbol: 100,
                price: black_box(100),
                size: black_box(10),
                action: OrderAction::Ask,
                order_type: OrderType::Gtc,
                ..Default::default()
            };
            orderbook.new_order(&mut cmd);
        });
    });
}

criterion_group!(
    benches,
    bench_naive_orderbook,
    bench_direct_orderbook,
    bench_direct_optimized_orderbook
);
criterion_main!(benches);

