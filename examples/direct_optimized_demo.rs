use matching_core::api::*;
use matching_core::core::orderbook::{DirectOrderBookOptimized, OrderBook};

fn main() {
    println!("=== DirectOrderBookOptimized 测试 ===\n");

    let spec = CoreSymbolSpecification {
        symbol_id: 100,
        symbol_type: SymbolType::CurrencyExchangePair,
        base_currency: 2,
        quote_currency: 1,
        base_scale_k: 100,
        quote_scale_k: 1,
        taker_fee: 10,
        maker_fee: 5,
        ..Default::default()
    };

    let mut orderbook = DirectOrderBookOptimized::new(spec);

    // 测试 1: 挂卖单
    println!("测试 1: 挂卖单");
    let mut cmd1 = OrderCommand {
        command: OrderCommandType::PlaceOrder,
        uid: 1001,
        order_id: 5001,
        symbol: 100,
        price: 100,
        size: 10,
        action: OrderAction::Ask,
        order_type: OrderType::Gtc,
        ..Default::default()
    };
    orderbook.new_order(&mut cmd1);
    println!("  卖单挂单成功，价格: 100, 数量: 10");

    // 测试 2: 挂买单（立即成交）
    println!("\n测试 2: 买单立即成交");
    let mut cmd2 = OrderCommand {
        command: OrderCommandType::PlaceOrder,
        uid: 1002,
        order_id: 5002,
        symbol: 100,
        price: 100,
        reserve_price: 100,
        size: 5,
        action: OrderAction::Bid,
        order_type: OrderType::Gtc,
        ..Default::default()
    };
    orderbook.new_order(&mut cmd2);
    println!("  成交事件数: {}", cmd2.matcher_events.len());
    for event in &cmd2.matcher_events {
        println!("    成交: 价格={}, 数量={}", event.price, event.size);
    }

    // 测试 3: L2 数据
    println!("\n测试 3: L2 市场深度");
    let l2 = orderbook.get_l2_data(5);
    println!("  卖单价格: {:?}", l2.ask_prices);
    println!("  卖单数量: {:?}", l2.ask_volumes);
    println!("  买单价格: {:?}", l2.bid_prices);
    println!("  买单数量: {:?}", l2.bid_volumes);

    // 测试 4: IOC 订单
    println!("\n测试 4: IOC 订单");
    let mut cmd3 = OrderCommand {
        command: OrderCommandType::PlaceOrder,
        uid: 1003,
        order_id: 5003,
        symbol: 100,
        price: 100,
        reserve_price: 100,
        size: 10,
        action: OrderAction::Bid,
        order_type: OrderType::Ioc,
        ..Default::default()
    };
    orderbook.new_order(&mut cmd3);
    println!("  成交事件数: {}", cmd3.matcher_events.len());
    for event in &cmd3.matcher_events {
        match event.event_type {
            MatcherEventType::Trade => println!("    成交: 数量={}", event.size),
            MatcherEventType::Reject => println!("    拒绝: 数量={}", event.size),
            _ => {}
        }
    }

    // 测试 5: 性能统计
    println!("\n测试 5: 性能统计");
    println!("  总卖单量: {}", orderbook.get_total_ask_volume());
    println!("  总买单量: {}", orderbook.get_total_bid_volume());
    println!("  卖单价格层数: {}", orderbook.get_ask_buckets_count());
    println!("  买单价格层数: {}", orderbook.get_bid_buckets_count());

    println!("\n=== 测试完成 ===");
    println!("优化特性: SOA 内存布局 + 订单池预分配 + SIMD 批量撮合");
}

