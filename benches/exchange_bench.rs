use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use matching_core::api::*;
use matching_core::core::orderbook::{DirectOrderBook, NaiveOrderBook, OrderBook};
use matching_core::core::processors::risk_engine::RiskEngine;

fn bench_orderbook(c: &mut Criterion) {
    let spec = CoreSymbolSpecification {
        symbol_id: 1,
        symbol_type: SymbolType::CurrencyExchangePair,
        base_currency: 1,
        quote_currency: 2,
        base_scale_k: 100,
        quote_scale_k: 1,
        ..Default::default()
    };

    let mut naive = NaiveOrderBook::new(spec.clone());
    let mut direct = DirectOrderBook::new(spec);

    let mut group = c.benchmark_group("OrderBook_new_order");

    group.bench_function("NaiveOrderBook_GTC", |b| {
        let mut i = 0u64;
        b.iter(|| {
            i += 1;
            naive.new_order(&mut OrderCommand {
                command: OrderCommandType::PlaceOrder,
                uid: 1,
                order_id: i,
                price: 100,
                size: 1,
                action: OrderAction::Ask,
                order_type: OrderType::Gtc,
                ..Default::default()
            });
        })
    });

    group.bench_function("DirectOrderBook_GTC", |b| {
        let mut i = 0u64;
        b.iter(|| {
            i += 1;
            direct.new_order(&mut OrderCommand {
                command: OrderCommandType::PlaceOrder,
                uid: 1,
                order_id: i + 1_000_000,
                price: 100,
                size: 1,
                action: OrderAction::Ask,
                order_type: OrderType::Gtc,
                ..Default::default()
            });
        })
    });

    group.finish();
}

fn bench_risk_engine(c: &mut Criterion) {
    let mut engine = RiskEngine::new(0, 1);
    engine.add_symbol(CoreSymbolSpecification {
        symbol_id: 1,
        base_currency: 1,
        quote_currency: 2,
        ..Default::default()
    });
    engine.pre_process(&mut OrderCommand {
        command: OrderCommandType::AddUser,
        uid: 1,
        ..Default::default()
    });
    engine.pre_process(&mut OrderCommand {
        command: OrderCommandType::BalanceAdjustment,
        uid: 1,
        symbol: 1,
        price: 1_000_000,
        ..Default::default()
    });

    c.bench_function("RiskEngine_pre_process_PlaceOrder_Ask", |b| {
        b.iter(|| {
            let mut cmd = OrderCommand {
                command: OrderCommandType::PlaceOrder,
                uid: 1,
                symbol: 1,
                price: 100,
                size: 1,
                action: OrderAction::Ask,
                ..Default::default()
            };
            engine.pre_process(&mut cmd);
        })
    });
}

criterion_group!(benches, bench_orderbook, bench_risk_engine);
criterion_main!(benches);
