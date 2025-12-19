use matching_core::api::*;
use matching_core::core::orderbook::{OrderBook, AdvancedOrderBook, DirectOrderBookOptimized, NaiveOrderBook};
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
    println!("=== 撮合引擎综合测试套件 ===\n");

    // 1. 基础功能测试
    test_basic_matching();
    
    // 2. 高级订单类型测试
    test_advanced_order_types();
    
    // 3. 多交易品种测试
    test_multiple_symbol_types();
    
    // 4. 性能对比测试
    test_performance_comparison();
    
    // 5. 压力测试
    test_stress_scenarios();
    
    // 6. 边界条件测试
    test_edge_cases();
    
    println!("\n=== 所有测试完成 ===");
}

fn test_basic_matching() {
    println!("1. 基础撮合功能测试");
    
    let mut book = AdvancedOrderBook::new(create_symbol_spec());
    
    // 挂卖单
    let mut ask = create_order(1, 1, 10000, 100, OrderAction::Ask, OrderType::Gtc);
    book.new_order(&mut ask);
    assert_eq!(book.get_total_ask_volume(), 100);
    
    // 买单成交
    let mut bid = create_order(2, 2, 10000, 50, OrderAction::Bid, OrderType::Ioc);
    book.new_order(&mut bid);
    assert_eq!(bid.matcher_events.len(), 1);
    assert_eq!(bid.matcher_events[0].size, 50);
    assert_eq!(book.get_total_ask_volume(), 50);
    
    println!("   ✓ 基础撮合正常\n");
}

fn test_advanced_order_types() {
    println!("2. 高级订单类型测试");
    
    let mut book = AdvancedOrderBook::new(create_symbol_spec());
    
    // Post-Only
    let mut ask1 = create_order(1, 1, 10000, 10, OrderAction::Ask, OrderType::Gtc);
    book.new_order(&mut ask1);
    
    let mut post_only = create_order(2, 2, 10000, 5, OrderAction::Bid, OrderType::PostOnly);
    book.new_order(&mut post_only);
    assert_eq!(post_only.matcher_events[0].event_type, MatcherEventType::Reject);
    
    // Iceberg
    let mut iceberg = OrderCommand {
        uid: 3,
        order_id: 3,
        symbol: 1,
        price: 9999,
        size: 100,
        action: OrderAction::Bid,
        order_type: OrderType::Iceberg,
        reserve_price: 9999,
        timestamp: 1000,
        visible_size: Some(10),
        ..Default::default()
    };
    book.new_order(&mut iceberg);
    let l2 = book.get_l2_data(5);
    assert!(l2.bid_volumes.iter().any(|&v| v == 10));
    
    // FOK
    let mut fok = create_order(4, 4, 10000, 20, OrderAction::Bid, OrderType::Fok);
    book.new_order(&mut fok);
    assert_eq!(fok.matcher_events[0].event_type, MatcherEventType::Reject);
    
    // GTD
    let mut gtd = OrderCommand {
        uid: 5,
        order_id: 5,
        symbol: 1,
        price: 10001,
        size: 20,
        action: OrderAction::Ask,
        order_type: OrderType::Gtd(2000),
        reserve_price: 10001,
        timestamp: 1000,
        expire_time: Some(2000),
        ..Default::default()
    };
    book.new_order(&mut gtd);
    assert_eq!(book.get_total_ask_volume(), 30); // 10 (ask1) + 20 (gtd)
    
    println!("   ✓ Post-Only、Iceberg、FOK、GTD 测试通过\n");
}

fn test_multiple_symbol_types() {
    println!("3. 多交易品种测试");
    
    let types = vec![
        (SymbolType::CurrencyExchangePair, "现货"),
        (SymbolType::FuturesContract, "期货"),
        (SymbolType::PerpetualSwap, "永续"),
        (SymbolType::CallOption, "看涨期权"),
        (SymbolType::PutOption, "看跌期权"),
    ];
    
    for (symbol_type, name) in types {
        let spec = CoreSymbolSpecification {
            symbol_id: 1,
            symbol_type,
            base_currency: 0,
            quote_currency: 1,
            base_scale_k: 1,
            quote_scale_k: 1,
            taker_fee: 0,
            maker_fee: 0,
            margin_buy: 0,
            margin_sell: 0,
        };
        
        let mut book = AdvancedOrderBook::new(spec);
        
        let mut ask = create_order(1, 1, 10000, 10, OrderAction::Ask, OrderType::Gtc);
        book.new_order(&mut ask);
        
        let mut bid = create_order(2, 2, 10000, 5, OrderAction::Bid, OrderType::Ioc);
        book.new_order(&mut bid);
        
        assert_eq!(bid.matcher_events[0].event_type, MatcherEventType::Trade);
        println!("   ✓ {} 撮合正常", name);
    }
    println!();
}

fn test_performance_comparison() {
    println!("4. 性能对比测试");
    
    let num_orders = 10000;
    
    // AdvancedOrderBook
    let start = Instant::now();
    let mut advanced = AdvancedOrderBook::new(create_symbol_spec());
    for i in 0..num_orders {
        let mut cmd = create_order(1, i, 10000 + (i % 100) as i64, 10, 
            if i % 2 == 0 { OrderAction::Ask } else { OrderAction::Bid }, 
            OrderType::Gtc);
        advanced.new_order(&mut cmd);
    }
    let advanced_time = start.elapsed();
    
    // DirectOrderBookOptimized
    let start = Instant::now();
    let mut optimized = DirectOrderBookOptimized::new(create_symbol_spec());
    for i in 0..num_orders {
        let mut cmd = create_order(1, i + num_orders as u64, 10000 + (i % 100) as i64, 10,
            if i % 2 == 0 { OrderAction::Ask } else { OrderAction::Bid },
            OrderType::Gtc);
        optimized.new_order(&mut cmd);
    }
    let optimized_time = start.elapsed();
    
    // NaiveOrderBook
    let start = Instant::now();
    let mut naive = NaiveOrderBook::new(create_symbol_spec());
    for i in 0..num_orders {
        let mut cmd = create_order(1, i + (num_orders * 2) as u64, 10000 + (i % 100) as i64, 10,
            if i % 2 == 0 { OrderAction::Ask } else { OrderAction::Bid },
            OrderType::Gtc);
        naive.new_order(&mut cmd);
    }
    let naive_time = start.elapsed();
    
    println!("   AdvancedOrderBook: {:?} ({:.2} ops/s)", 
        advanced_time, num_orders as f64 / advanced_time.as_secs_f64());
    println!("   DirectOrderBookOptimized: {:?} ({:.2} ops/s)", 
        optimized_time, num_orders as f64 / optimized_time.as_secs_f64());
    println!("   NaiveOrderBook: {:?} ({:.2} ops/s)", 
        naive_time, num_orders as f64 / naive_time.as_secs_f64());
    println!();
}

fn test_stress_scenarios() {
    println!("5. 压力测试场景");
    
    let mut book = AdvancedOrderBook::new(create_symbol_spec());
    
    // 场景1: 大量冰山单
    println!("   场景1: 1000个冰山单");
    let start = Instant::now();
    for i in 0..1000 {
        let mut cmd = OrderCommand {
            uid: 1,
            order_id: i,
            symbol: 1,
            price: 10000,
            size: 1000,
            action: OrderAction::Ask,
            order_type: OrderType::Iceberg,
            reserve_price: 10000,
            timestamp: 1000,
            visible_size: Some(10),
            ..Default::default()
        };
        book.new_order(&mut cmd);
    }
    println!("      完成时间: {:?}", start.elapsed());
    
    // 场景2: 大量成交
    println!("   场景2: 10000次成交");
    let start = Instant::now();
    for i in 0..10000 {
        let mut cmd = create_order(2, 1000 + i, 10000, 1, OrderAction::Bid, OrderType::Ioc);
        book.new_order(&mut cmd);
    }
    println!("      完成时间: {:?}", start.elapsed());
    
    // 场景3: 混合订单类型
    println!("   场景3: 混合订单类型 (5000个)");
    let start = Instant::now();
    for i in 0..5000 {
        let order_type = match i % 5 {
            0 => OrderType::Gtc,
            1 => OrderType::Ioc,
            2 => OrderType::PostOnly,
            3 => OrderType::Fok,
            _ => OrderType::Day,
        };
        let mut cmd = create_order(3, 11000 + i, 10000 + (i % 10) as i64, 10,
            if i % 2 == 0 { OrderAction::Ask } else { OrderAction::Bid },
            order_type);
        book.new_order(&mut cmd);
    }
    println!("      完成时间: {:?}", start.elapsed());
    println!();
}

fn test_edge_cases() {
    println!("6. 边界条件测试");
    
    let mut book = AdvancedOrderBook::new(create_symbol_spec());
    
    // 零数量订单
    let mut zero = create_order(1, 1, 10000, 0, OrderAction::Ask, OrderType::Gtc);
    book.new_order(&mut zero);
    assert_eq!(book.get_total_ask_volume(), 0);
    
    // 极高价格
    let mut high = create_order(1, 2, i64::MAX / 2, 10, OrderAction::Ask, OrderType::Gtc);
    book.new_order(&mut high);
    assert_eq!(book.get_total_ask_volume(), 10);
    
    // 大量订单ID（使用新的 book 避免之前的订单影响）
    let mut book2 = AdvancedOrderBook::new(create_symbol_spec());
    for i in 0..100 {
        let mut cmd = create_order(1, u64::MAX - 100 + i, 10000 + i as i64, 1, 
            OrderAction::Ask, OrderType::Gtc);
        book2.new_order(&mut cmd);
    }
    assert_eq!(book2.get_ask_buckets_count(), 100);
    
    // 取消不存在的订单
    let mut cancel = create_order(1, 99999, 0, 0, OrderAction::Bid, OrderType::Gtc);
    let result = book.cancel_order(&mut cancel);
    assert_eq!(result, CommandResultCode::MatchingUnknownOrderId);
    
    println!("   ✓ 边界条件测试通过\n");
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

