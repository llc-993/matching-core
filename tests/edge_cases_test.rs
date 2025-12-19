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

#[test]
fn test_iceberg_multiple_refresh() {
    // 测试冰山单多次刷新
    let mut book = AdvancedOrderBook::new(create_symbol_spec());
    
    let mut iceberg = OrderCommand {
        uid: 1,
        order_id: 1,
        symbol: 1,
        price: 10000,
        size: 50,
        action: OrderAction::Ask,
        order_type: OrderType::Iceberg,
        reserve_price: 10000,
        timestamp: 1000,
        visible_size: Some(5),
        ..Default::default()
    };
    book.new_order(&mut iceberg);
    
    // 第一次成交 5
    let mut bid1 = OrderCommand {
        uid: 2,
        order_id: 2,
        symbol: 1,
        price: 10000,
        size: 5,
        action: OrderAction::Bid,
        order_type: OrderType::Ioc,
        reserve_price: 10000,
        timestamp: 1001,
        ..Default::default()
    };
    book.new_order(&mut bid1);
    assert_eq!(bid1.matcher_events[0].size, 5);
    assert_eq!(book.get_total_ask_volume(), 45);
    
    // 第二次成交 5
    let mut bid2 = OrderCommand {
        uid: 2,
        order_id: 3,
        symbol: 1,
        price: 10000,
        size: 5,
        action: OrderAction::Bid,
        order_type: OrderType::Ioc,
        reserve_price: 10000,
        timestamp: 1002,
        ..Default::default()
    };
    book.new_order(&mut bid2);
    assert_eq!(bid2.matcher_events[0].size, 5);
    assert_eq!(book.get_total_ask_volume(), 40);
    
    // 第三次成交 10（应该成交 10）
    let mut bid3 = OrderCommand {
        uid: 2,
        order_id: 4,
        symbol: 1,
        price: 10000,
        size: 10,
        action: OrderAction::Bid,
        order_type: OrderType::Ioc,
        reserve_price: 10000,
        timestamp: 1003,
        ..Default::default()
    };
    book.new_order(&mut bid3);
    assert_eq!(bid3.matcher_events[0].size, 10);
    assert_eq!(book.get_total_ask_volume(), 30);
}

#[test]
fn test_stop_order_trigger() {
    // 测试止损单触发机制
    let mut book = AdvancedOrderBook::new(create_symbol_spec());
    
    // 挂卖单 @10000
    let mut ask1 = OrderCommand {
        uid: 1,
        order_id: 1,
        symbol: 1,
        price: 10000,
        size: 100,
        action: OrderAction::Ask,
        order_type: OrderType::Gtc,
        reserve_price: 10000,
        timestamp: 1000,
        ..Default::default()
    };
    book.new_order(&mut ask1);
    
    // 下止损买单：触发价 10500
    let mut stop = OrderCommand {
        uid: 2,
        order_id: 2,
        symbol: 1,
        price: 10600,
        size: 50,
        action: OrderAction::Bid,
        order_type: OrderType::StopLimit,
        reserve_price: 10600,
        timestamp: 1001,
        stop_price: Some(10500),
        ..Default::default()
    };
    book.new_order(&mut stop);
    
    // 止损单不应该立即进入订单簿
    assert_eq!(book.get_total_bid_volume(), 0);
    
    // 买单成交 @10000（未触发止损）
    let mut bid1 = OrderCommand {
        uid: 3,
        order_id: 3,
        symbol: 1,
        price: 10000,
        size: 10,
        action: OrderAction::Bid,
        order_type: OrderType::Ioc,
        reserve_price: 10000,
        timestamp: 1002,
        ..Default::default()
    };
    book.new_order(&mut bid1);
    
    // 止损单仍未触发
    assert_eq!(book.get_total_bid_volume(), 0);
}

#[test]
fn test_gtd_partial_fill() {
    // 测试 GTD 订单部分成交后过期
    let mut book = AdvancedOrderBook::new(create_symbol_spec());
    
    let mut gtd = OrderCommand {
        uid: 1,
        order_id: 1,
        symbol: 1,
        price: 10000,
        size: 100,
        action: OrderAction::Ask,
        order_type: OrderType::Gtd(2000),
        reserve_price: 10000,
        timestamp: 1000,
        expire_time: Some(2000),
        ..Default::default()
    };
    book.new_order(&mut gtd);
    
    // 部分成交
    let mut bid1 = OrderCommand {
        uid: 2,
        order_id: 2,
        symbol: 1,
        price: 10000,
        size: 30,
        action: OrderAction::Bid,
        order_type: OrderType::Ioc,
        reserve_price: 10000,
        timestamp: 1500,
        ..Default::default()
    };
    book.new_order(&mut bid1);
    assert_eq!(bid1.matcher_events[0].size, 30);
    assert_eq!(book.get_total_ask_volume(), 70);
    
    // 过期后尝试成交
    let mut bid2 = OrderCommand {
        uid: 2,
        order_id: 3,
        symbol: 1,
        price: 10000,
        size: 30,
        action: OrderAction::Bid,
        order_type: OrderType::Ioc,
        reserve_price: 10000,
        timestamp: 2500,
        ..Default::default()
    };
    book.new_order(&mut bid2);
    
    // 应该无法成交（订单已过期）
    assert!(bid2.matcher_events.is_empty() || 
            bid2.matcher_events[0].event_type == MatcherEventType::Reject);
}

#[test]
fn test_post_only_with_same_price() {
    // 测试 Post-Only 在同价位的行为
    let mut book = AdvancedOrderBook::new(create_symbol_spec());
    
    // 挂卖单 @10000
    let mut ask1 = OrderCommand {
        uid: 1,
        order_id: 1,
        symbol: 1,
        price: 10000,
        size: 10,
        action: OrderAction::Ask,
        order_type: OrderType::Gtc,
        reserve_price: 10000,
        timestamp: 1000,
        ..Default::default()
    };
    book.new_order(&mut ask1);
    
    // Post-Only 买单 @10000（应该被拒绝）
    let mut post_only = OrderCommand {
        uid: 2,
        order_id: 2,
        symbol: 1,
        price: 10000,
        size: 5,
        action: OrderAction::Bid,
        order_type: OrderType::PostOnly,
        reserve_price: 10000,
        timestamp: 1001,
        ..Default::default()
    };
    book.new_order(&mut post_only);
    
    assert_eq!(post_only.matcher_events.len(), 1);
    assert_eq!(post_only.matcher_events[0].event_type, MatcherEventType::Reject);
    
    // 卖单应该保持不变
    assert_eq!(book.get_total_ask_volume(), 10);
}

#[test]
fn test_fok_exact_match() {
    // 测试 FOK 完全匹配
    let mut book = AdvancedOrderBook::new(create_symbol_spec());
    
    // 挂三个卖单
    let mut ask1 = OrderCommand {
        uid: 1,
        order_id: 1,
        symbol: 1,
        price: 10000,
        size: 10,
        action: OrderAction::Ask,
        order_type: OrderType::Gtc,
        reserve_price: 10000,
        timestamp: 1000,
        ..Default::default()
    };
    book.new_order(&mut ask1);
    
    let mut ask2 = OrderCommand {
        uid: 1,
        order_id: 2,
        symbol: 1,
        price: 10001,
        size: 15,
        action: OrderAction::Ask,
        order_type: OrderType::Gtc,
        reserve_price: 10001,
        timestamp: 1001,
        ..Default::default()
    };
    book.new_order(&mut ask2);
    
    let mut ask3 = OrderCommand {
        uid: 1,
        order_id: 3,
        symbol: 1,
        price: 10002,
        size: 5,
        action: OrderAction::Ask,
        order_type: OrderType::Gtc,
        reserve_price: 10002,
        timestamp: 1002,
        ..Default::default()
    };
    book.new_order(&mut ask3);
    
    // FOK 买单正好 30（10 + 15 + 5）
    let mut fok = OrderCommand {
        uid: 2,
        order_id: 4,
        symbol: 1,
        price: 10002,
        size: 30,
        action: OrderAction::Bid,
        order_type: OrderType::Fok,
        reserve_price: 10002,
        timestamp: 1003,
        ..Default::default()
    };
    book.new_order(&mut fok);
    
    // 应该完全成交
    assert_eq!(fok.matcher_events.len(), 3);
    let total_matched: i64 = fok.matcher_events.iter()
        .filter(|e| e.event_type == MatcherEventType::Trade)
        .map(|e| e.size)
        .sum();
    assert_eq!(total_matched, 30);
    
    // 订单簿应该清空
    assert_eq!(book.get_total_ask_volume(), 0);
}

#[test]
fn test_mixed_order_types() {
    // 测试多种订单类型混合
    let mut book = AdvancedOrderBook::new(create_symbol_spec());
    
    // 1. 普通 GTC 订单
    let mut gtc = OrderCommand {
        uid: 1,
        order_id: 1,
        symbol: 1,
        price: 10000,
        size: 20,
        action: OrderAction::Ask,
        order_type: OrderType::Gtc,
        reserve_price: 10000,
        timestamp: 1000,
        ..Default::default()
    };
    book.new_order(&mut gtc);
    
    // 2. 冰山单
    let mut iceberg = OrderCommand {
        uid: 2,
        order_id: 2,
        symbol: 1,
        price: 10001,
        size: 50,
        action: OrderAction::Ask,
        order_type: OrderType::Iceberg,
        reserve_price: 10001,
        timestamp: 1001,
        visible_size: Some(10),
        ..Default::default()
    };
    book.new_order(&mut iceberg);
    
    // 3. GTD 订单
    let mut gtd = OrderCommand {
        uid: 3,
        order_id: 3,
        symbol: 1,
        price: 10002,
        size: 30,
        action: OrderAction::Ask,
        order_type: OrderType::Gtd(3000),
        reserve_price: 10002,
        timestamp: 1002,
        expire_time: Some(3000),
        ..Default::default()
    };
    book.new_order(&mut gtd);
    
    assert_eq!(book.get_total_ask_volume(), 100);
    
    // IOC 买单成交部分
    let mut ioc = OrderCommand {
        uid: 4,
        order_id: 4,
        symbol: 1,
        price: 10001,
        size: 35,
        action: OrderAction::Bid,
        order_type: OrderType::Ioc,
        reserve_price: 10001,
        timestamp: 1003,
        ..Default::default()
    };
    book.new_order(&mut ioc);
    
    // 应该成交 20 + 10（冰山显示部分） = 30
    let total_matched: i64 = ioc.matcher_events.iter()
        .filter(|e| e.event_type == MatcherEventType::Trade)
        .map(|e| e.size)
        .sum();
    assert!(total_matched >= 30);
    
    // 剩余总量应该减少
    assert!(book.get_total_ask_volume() < 100);
}

#[test]
fn test_cancel_stop_order() {
    // 测试取消止损单
    let mut book = AdvancedOrderBook::new(create_symbol_spec());
    
    // 下止损买单
    let mut stop = OrderCommand {
        uid: 1,
        order_id: 1,
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
    book.new_order(&mut stop);
    
    // 取消止损单
    let mut cancel = OrderCommand {
        uid: 1,
        order_id: 1,
        symbol: 1,
        price: 0,
        size: 0,
        action: OrderAction::Bid,
        order_type: OrderType::Gtc,
        reserve_price: 0,
        timestamp: 1001,
        ..Default::default()
    };
    let result = book.cancel_order(&mut cancel);
    
    assert_eq!(result, CommandResultCode::Success);
    assert_eq!(cancel.matcher_events.len(), 1);
    assert_eq!(cancel.matcher_events[0].event_type, MatcherEventType::Reject);
}

#[test]
fn test_all_symbol_types() {
    // 测试所有交易品种类型
    let types = vec![
        SymbolType::CurrencyExchangePair,
        SymbolType::FuturesContract,
        SymbolType::PerpetualSwap,
        SymbolType::CallOption,
        SymbolType::PutOption,
    ];
    
    for (i, symbol_type) in types.iter().enumerate() {
        let spec = CoreSymbolSpecification {
            symbol_id: i as i32,
            symbol_type: *symbol_type,
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
        
        let mut ask = OrderCommand {
            uid: 1,
            order_id: (i * 2) as u64,
            symbol: i as i32,
            price: 10000,
            size: 10,
            action: OrderAction::Ask,
            order_type: OrderType::Gtc,
            reserve_price: 10000,
            timestamp: 1000,
            ..Default::default()
        };
        book.new_order(&mut ask);
        
        let mut bid = OrderCommand {
            uid: 2,
            order_id: (i * 2 + 1) as u64,
            symbol: i as i32,
            price: 10000,
            size: 5,
            action: OrderAction::Bid,
            order_type: OrderType::Ioc,
            reserve_price: 10000,
            timestamp: 1001,
            ..Default::default()
        };
        book.new_order(&mut bid);
        
        assert_eq!(bid.matcher_events.len(), 1);
        assert_eq!(bid.matcher_events[0].event_type, MatcherEventType::Trade);
        assert_eq!(bid.matcher_events[0].size, 5);
    }
}

