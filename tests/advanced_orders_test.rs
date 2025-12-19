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
fn test_post_only_order() {
    let mut book = AdvancedOrderBook::new(create_symbol_spec());
    
    // 挂卖单 价格 10000
    let mut ask_cmd = OrderCommand {
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
    book.new_order(&mut ask_cmd);
    
    // Post-Only 买单价格 10000（会立即成交，应被拒绝）
    let mut bid_cmd = OrderCommand {
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
    book.new_order(&mut bid_cmd);
    
    // 应该产生拒绝事件
    assert_eq!(bid_cmd.matcher_events.len(), 1);
    assert_eq!(bid_cmd.matcher_events[0].event_type, MatcherEventType::Reject);
    assert_eq!(bid_cmd.matcher_events[0].size, 5);
    
    // Post-Only 买单价格 9999（不会成交，应成功挂单）
    let mut bid_cmd2 = OrderCommand {
        uid: 2,
        order_id: 3,
        symbol: 1,
        price: 9999,
        size: 5,
        action: OrderAction::Bid,
        order_type: OrderType::PostOnly,
        reserve_price: 9999,
        timestamp: 1002,
        ..Default::default()
    };
    book.new_order(&mut bid_cmd2);
    
    // 应该没有事件（成功挂单）
    assert_eq!(bid_cmd2.matcher_events.len(), 0);
    assert_eq!(book.get_total_bid_volume(), 5);
}

#[test]
fn test_stop_limit_order() {
    let mut book = AdvancedOrderBook::new(create_symbol_spec());
    
    // 下止损买单：触发价 10500
    let mut stop_cmd = OrderCommand {
        uid: 1,
        order_id: 1,
        symbol: 1,
        price: 10600,  // 限价
        size: 10,
        action: OrderAction::Bid,
        order_type: OrderType::StopLimit,
        reserve_price: 10600,
        timestamp: 1000,
        stop_price: Some(10500),  // 触发价
        ..Default::default()
    };
    book.new_order(&mut stop_cmd);
    
    // 止损单不应该立即进入订单簿
    assert_eq!(book.get_total_bid_volume(), 0);
    
    // 挂卖单 10400
    let mut ask_cmd = OrderCommand {
        uid: 2,
        order_id: 2,
        symbol: 1,
        price: 10400,
        size: 5,
        action: OrderAction::Ask,
        order_type: OrderType::Gtc,
        reserve_price: 10400,
        timestamp: 1001,
        ..Default::default()
    };
    book.new_order(&mut ask_cmd);
    
    // 买单成交价 10400，触发止损单
    let mut bid_cmd = OrderCommand {
        uid: 3,
        order_id: 3,
        symbol: 1,
        price: 10600,
        size: 5,
        action: OrderAction::Bid,
        order_type: OrderType::Gtc,
        reserve_price: 10600,
        timestamp: 1002,
        ..Default::default()
    };
    book.new_order(&mut bid_cmd);
    
    // 应该有成交事件
    assert!(!bid_cmd.matcher_events.is_empty());
    
    // 注意：止损单触发后应该转为限价单进入订单簿
    // 由于成交价 < 止损触发价，止损单不应触发（买止损需要价格上涨）
}

#[test]
fn test_iceberg_order() {
    let mut book = AdvancedOrderBook::new(create_symbol_spec());
    
    // 冰山卖单：总量 100，显示 10
    let mut iceberg_cmd = OrderCommand {
        uid: 1,
        order_id: 1,
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
    book.new_order(&mut iceberg_cmd);
    
    // 查询市场深度，应该只显示 10
    let l2 = book.get_l2_data(1);
    assert_eq!(l2.ask_volumes.len(), 1);
    assert_eq!(l2.ask_volumes[0], 10);  // 只显示 10
    
    // 买单成交 10
    let mut bid_cmd = OrderCommand {
        uid: 2,
        order_id: 2,
        symbol: 1,
        price: 10000,
        size: 10,
        action: OrderAction::Bid,
        order_type: OrderType::Ioc,
        reserve_price: 10000,
        timestamp: 1001,
        ..Default::default()
    };
    book.new_order(&mut bid_cmd);
    
    // 应该成交 10
    assert_eq!(bid_cmd.matcher_events.len(), 1);
    assert_eq!(bid_cmd.matcher_events[0].size, 10);
    
    // 订单簿应该还有 90（刷新后显示 10）
    assert_eq!(book.get_total_ask_volume(), 90);
}

#[test]
fn test_fok_order() {
    let mut book = AdvancedOrderBook::new(create_symbol_spec());
    
    // 挂卖单 5 个
    let mut ask_cmd = OrderCommand {
        uid: 1,
        order_id: 1,
        symbol: 1,
        price: 10000,
        size: 5,
        action: OrderAction::Ask,
        order_type: OrderType::Gtc,
        reserve_price: 10000,
        timestamp: 1000,
        ..Default::default()
    };
    book.new_order(&mut ask_cmd);
    
    // FOK 买单 10 个（无法全部成交）
    let mut fok_cmd = OrderCommand {
        uid: 2,
        order_id: 2,
        symbol: 1,
        price: 10000,
        size: 10,
        action: OrderAction::Bid,
        order_type: OrderType::Fok,
        reserve_price: 10000,
        timestamp: 1001,
        ..Default::default()
    };
    book.new_order(&mut fok_cmd);
    
    // 应该全部被拒绝
    assert_eq!(fok_cmd.matcher_events.len(), 1);
    assert_eq!(fok_cmd.matcher_events[0].event_type, MatcherEventType::Reject);
    assert_eq!(fok_cmd.matcher_events[0].size, 10);
    
    // 订单簿应该保持不变
    assert_eq!(book.get_total_ask_volume(), 5);
}

#[test]
fn test_gtd_order() {
    let mut book = AdvancedOrderBook::new(create_symbol_spec());
    
    // GTD 卖单，过期时间 2000
    let mut gtd_cmd = OrderCommand {
        uid: 1,
        order_id: 1,
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
    book.new_order(&mut gtd_cmd);
    
    assert_eq!(book.get_total_ask_volume(), 10);
    
    // 时间未过期，买单应该能成交
    let mut bid_cmd = OrderCommand {
        uid: 2,
        order_id: 2,
        symbol: 1,
        price: 10000,
        size: 5,
        action: OrderAction::Bid,
        order_type: OrderType::Ioc,
        reserve_price: 10000,
        timestamp: 1500,  // 未过期
        ..Default::default()
    };
    book.new_order(&mut bid_cmd);
    
    assert_eq!(bid_cmd.matcher_events.len(), 1);
    assert_eq!(bid_cmd.matcher_events[0].size, 5);
    
    // 时间过期后，买单应该无法成交
    let mut bid_cmd2 = OrderCommand {
        uid: 2,
        order_id: 3,
        symbol: 1,
        price: 10000,
        size: 5,
        action: OrderAction::Bid,
        order_type: OrderType::Ioc,
        reserve_price: 10000,
        timestamp: 2500,  // 已过期
        ..Default::default()
    };
    book.new_order(&mut bid_cmd2);
    
    // 应该被拒绝（因为订单已过期）
    assert_eq!(bid_cmd2.matcher_events.len(), 1);
    assert_eq!(bid_cmd2.matcher_events[0].event_type, MatcherEventType::Reject);
}

#[test]
fn test_perpetual_swap() {
    let mut spec = create_symbol_spec();
    spec.symbol_type = SymbolType::PerpetualSwap;
    
    let mut book = AdvancedOrderBook::new(spec);
    
    // 永续合约买单
    let mut bid_cmd = OrderCommand {
        uid: 1,
        order_id: 1,
        symbol: 1,
        price: 50000,
        size: 1,
        action: OrderAction::Bid,
        order_type: OrderType::Gtc,
        reserve_price: 50000,
        timestamp: 1000,
        ..Default::default()
    };
    book.new_order(&mut bid_cmd);
    
    // 永续合约卖单
    let mut ask_cmd = OrderCommand {
        uid: 2,
        order_id: 2,
        symbol: 1,
        price: 50000,
        size: 1,
        action: OrderAction::Ask,
        order_type: OrderType::Gtc,
        reserve_price: 50000,
        timestamp: 1001,
        ..Default::default()
    };
    book.new_order(&mut ask_cmd);
    
    // 应该成交
    assert_eq!(ask_cmd.matcher_events.len(), 1);
    assert_eq!(ask_cmd.matcher_events[0].event_type, MatcherEventType::Trade);
}

#[test]
fn test_call_option() {
    let mut spec = create_symbol_spec();
    spec.symbol_type = SymbolType::CallOption;
    
    let mut book = AdvancedOrderBook::new(spec);
    
    // 看涨期权买单
    let mut bid_cmd = OrderCommand {
        uid: 1,
        order_id: 1,
        symbol: 1,
        price: 500,  // 权利金
        size: 10,
        action: OrderAction::Bid,
        order_type: OrderType::Gtc,
        reserve_price: 500,
        timestamp: 1000,
        ..Default::default()
    };
    book.new_order(&mut bid_cmd);
    
    // 看涨期权卖单
    let mut ask_cmd = OrderCommand {
        uid: 2,
        order_id: 2,
        symbol: 1,
        price: 500,
        size: 5,
        action: OrderAction::Ask,
        order_type: OrderType::Gtc,
        reserve_price: 500,
        timestamp: 1001,
        ..Default::default()
    };
    book.new_order(&mut ask_cmd);
    
    // 应该成交 5
    assert_eq!(ask_cmd.matcher_events.len(), 1);
    assert_eq!(ask_cmd.matcher_events[0].size, 5);
    assert_eq!(book.get_total_bid_volume(), 5);
}

#[test]
fn test_put_option() {
    let mut spec = create_symbol_spec();
    spec.symbol_type = SymbolType::PutOption;
    
    let mut book = AdvancedOrderBook::new(spec);
    
    // 看跌期权交易
    let mut ask_cmd = OrderCommand {
        uid: 1,
        order_id: 1,
        symbol: 1,
        price: 300,
        size: 20,
        action: OrderAction::Ask,
        order_type: OrderType::Gtc,
        reserve_price: 300,
        timestamp: 1000,
        ..Default::default()
    };
    book.new_order(&mut ask_cmd);
    
    let mut bid_cmd = OrderCommand {
        uid: 2,
        order_id: 2,
        symbol: 1,
        price: 300,
        size: 10,
        action: OrderAction::Bid,
        order_type: OrderType::Ioc,
        reserve_price: 300,
        timestamp: 1001,
        ..Default::default()
    };
    book.new_order(&mut bid_cmd);
    
    // 应该成交 10
    assert_eq!(bid_cmd.matcher_events.len(), 1);
    assert_eq!(bid_cmd.matcher_events[0].size, 10);
    assert_eq!(book.get_total_ask_volume(), 10);
}

#[test]
fn test_day_order() {
    let mut book = AdvancedOrderBook::new(create_symbol_spec());
    
    // Day 订单（当日有效）
    let mut day_cmd = OrderCommand {
        uid: 1,
        order_id: 1,
        symbol: 1,
        price: 10000,
        size: 10,
        action: OrderAction::Bid,
        order_type: OrderType::Day,
        reserve_price: 10000,
        timestamp: 1000,
        ..Default::default()
    };
    book.new_order(&mut day_cmd);
    
    assert_eq!(book.get_total_bid_volume(), 10);
    
    // Day 订单应该能被取消
    let mut cancel_cmd = OrderCommand {
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
    let result = book.cancel_order(&mut cancel_cmd);
    
    assert_eq!(result, CommandResultCode::Success);
    assert_eq!(book.get_total_bid_volume(), 0);
}

