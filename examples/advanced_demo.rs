use matching_core::api::*;
use matching_core::core::orderbook::{OrderBook, AdvancedOrderBook};

fn main() {
    println!("=== 高级订单类型演示 ===\n");

    // 创建现货交易对
    let spot_spec = CoreSymbolSpecification {
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
    };

    let mut book = AdvancedOrderBook::new(spot_spec);

    // 1. Post-Only 订单
    println!("1. Post-Only 订单（只做 Maker）");
    let mut ask1 = create_order(1, 1, 10000, 10, OrderAction::Ask, OrderType::Gtc);
    book.new_order(&mut ask1);
    println!("   挂卖单 @10000 x 10");

    let mut post_only = create_order(2, 2, 10000, 5, OrderAction::Bid, OrderType::PostOnly);
    book.new_order(&mut post_only);
    if !post_only.matcher_events.is_empty() && post_only.matcher_events[0].event_type == MatcherEventType::Reject {
        println!("   ✓ Post-Only 买单 @10000 被拒绝（会立即成交）");
    }

    let mut post_only2 = create_order(2, 3, 9999, 5, OrderAction::Bid, OrderType::PostOnly);
    book.new_order(&mut post_only2);
    println!("   ✓ Post-Only 买单 @9999 成功挂单\n");

    // 2. 冰山单
    println!("2. 冰山单（Iceberg Order）");
    let mut iceberg = OrderCommand {
        uid: 3,
        order_id: 4,
        symbol: 1,
        price: 9998,
        size: 100,
        action: OrderAction::Bid,
        order_type: OrderType::Iceberg,
        reserve_price: 9998,
        timestamp: 1000,
        visible_size: Some(10),
        ..Default::default()
    };
    book.new_order(&mut iceberg);
    println!("   挂冰山买单 @9998 总量 100，显示 10");
    
    let l2 = book.get_l2_data(5);
    for (i, (price, vol)) in l2.bid_prices.iter().zip(l2.bid_volumes.iter()).enumerate() {
        println!("   买{}: {} x {}", i+1, price, vol);
    }
    println!();

    // 3. FOK 订单
    println!("3. FOK 订单（Fill-or-Kill）");
    let mut fok = create_order(4, 5, 10000, 20, OrderAction::Bid, OrderType::Fok);
    book.new_order(&mut fok);
    if !fok.matcher_events.is_empty() && fok.matcher_events[0].event_type == MatcherEventType::Reject {
        println!("   ✓ FOK 买单被拒绝（无法全部成交 20，只有 10）\n");
    }

    let mut fok2 = create_order(4, 6, 10000, 5, OrderAction::Bid, OrderType::Fok);
    book.new_order(&mut fok2);
    if !fok2.matcher_events.is_empty() && fok2.matcher_events[0].event_type == MatcherEventType::Trade {
        println!("   ✓ FOK 买单成功（全部成交 5）\n");
    }

    // 4. GTD 订单
    println!("4. GTD 订单（Good-Till-Date）");
    let mut gtd = OrderCommand {
        uid: 5,
        order_id: 7,
        symbol: 1,
        price: 10100,
        size: 20,
        action: OrderAction::Ask,
        order_type: OrderType::Gtd(2000),
        reserve_price: 10100,
        timestamp: 1000,
        expire_time: Some(2000),
        ..Default::default()
    };
    book.new_order(&mut gtd);
    println!("   挂 GTD 卖单 @10100 x 20（过期时间 2000）");

    let mut bid_before = create_order(6, 8, 10100, 5, OrderAction::Bid, OrderType::Ioc);
    bid_before.timestamp = 1500;
    book.new_order(&mut bid_before);
    println!("   时间 1500：成交 {} 单位", 
        bid_before.matcher_events.iter().filter(|e| e.event_type == MatcherEventType::Trade)
            .map(|e| e.size).sum::<i64>());

    let mut bid_after = create_order(6, 9, 10100, 5, OrderAction::Bid, OrderType::Ioc);
    bid_after.timestamp = 2500;
    book.new_order(&mut bid_after);
    println!("   时间 2500：成交 {} 单位（订单已过期）\n", 
        bid_after.matcher_events.iter().filter(|e| e.event_type == MatcherEventType::Trade)
            .map(|e| e.size).sum::<i64>());

    // 5. 止损单
    println!("5. 止损单（Stop Order）");
    let mut stop = OrderCommand {
        uid: 7,
        order_id: 10,
        symbol: 1,
        price: 11000,
        size: 10,
        action: OrderAction::Bid,
        order_type: OrderType::StopLimit,
        reserve_price: 11000,
        timestamp: 3000,
        stop_price: Some(10500),
        ..Default::default()
    };
    book.new_order(&mut stop);
    println!("   下止损买单：触发价 10500，限价 11000");
    println!("   （当价格涨到 10500 时自动激活）\n");

    // 6. 永续合约
    println!("6. 永续合约（Perpetual Swap）");
    let perp_spec = CoreSymbolSpecification {
        symbol_id: 2,
        symbol_type: SymbolType::PerpetualSwap,
        base_currency: 0,
        quote_currency: 1,
        base_scale_k: 1,
        quote_scale_k: 1,
        taker_fee: 0,
        maker_fee: 0,
        margin_buy: 0,
        margin_sell: 0,
    };
    let mut perp_book = AdvancedOrderBook::new(perp_spec);
    
    let mut perp_bid = OrderCommand {
        uid: 8,
        order_id: 11,
        symbol: 2,
        price: 50000,
        size: 1,
        action: OrderAction::Bid,
        order_type: OrderType::Gtc,
        reserve_price: 50000,
        timestamp: 4000,
        ..Default::default()
    };
    perp_book.new_order(&mut perp_bid);
    
    let mut perp_ask = OrderCommand {
        uid: 9,
        order_id: 12,
        symbol: 2,
        price: 50000,
        size: 1,
        action: OrderAction::Ask,
        order_type: OrderType::Gtc,
        reserve_price: 50000,
        timestamp: 4001,
        ..Default::default()
    };
    perp_book.new_order(&mut perp_ask);
    println!("   ✓ 永续合约交易：BTC-PERP @50000\n");

    // 7. 期权
    println!("7. 期权（Options）");
    let call_spec = CoreSymbolSpecification {
        symbol_id: 3,
        symbol_type: SymbolType::CallOption,
        base_currency: 0,
        quote_currency: 1,
        base_scale_k: 1,
        quote_scale_k: 1,
        taker_fee: 0,
        maker_fee: 0,
        margin_buy: 0,
        margin_sell: 0,
    };
    let mut option_book = AdvancedOrderBook::new(call_spec);
    
    let mut option_bid = OrderCommand {
        uid: 10,
        order_id: 13,
        symbol: 3,
        price: 500,
        size: 10,
        action: OrderAction::Bid,
        order_type: OrderType::Gtc,
        reserve_price: 500,
        timestamp: 5000,
        ..Default::default()
    };
    option_book.new_order(&mut option_bid);
    
    let mut option_ask = OrderCommand {
        uid: 11,
        order_id: 14,
        symbol: 3,
        price: 500,
        size: 5,
        action: OrderAction::Ask,
        order_type: OrderType::Gtc,
        reserve_price: 500,
        timestamp: 5001,
        ..Default::default()
    };
    option_book.new_order(&mut option_ask);
    println!("   ✓ 看涨期权交易：权利金 500，成交 5 张\n");

    // 8. Day 订单
    println!("8. Day 订单（当日有效）");
    let mut day = create_order(12, 15, 9900, 15, OrderAction::Bid, OrderType::Day);
    book.new_order(&mut day);
    println!("   ✓ Day 订单 @9900 x 15（当日有效）\n");

    // 最终市场深度
    println!("=== 最终市场深度（现货） ===");
    let l2 = book.get_l2_data(10);
    
    println!("卖盘：");
    for (price, vol) in l2.ask_prices.iter().rev().zip(l2.ask_volumes.iter().rev()) {
        println!("  {} x {}", price, vol);
    }
    
    println!("---");
    
    println!("买盘：");
    for (price, vol) in l2.bid_prices.iter().zip(l2.bid_volumes.iter()) {
        println!("  {} x {}", price, vol);
    }
    
    println!("\n总买量: {}", book.get_total_bid_volume());
    println!("总卖量: {}", book.get_total_ask_volume());
}

fn create_order(
    uid: UserId,
    order_id: OrderId,
    price: Price,
    size: Size,
    action: OrderAction,
    order_type: OrderType,
) -> OrderCommand {
    OrderCommand {
        uid,
        order_id,
        symbol: 1,
        price,
        size,
        action,
        order_type,
        reserve_price: price,
        timestamp: 1000,
        ..Default::default()
    }
}

