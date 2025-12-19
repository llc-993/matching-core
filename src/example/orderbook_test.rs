use crate::api::*;
use crate::core::orderbook::{OrderBook, DirectOrderBook, NaiveOrderBook};

pub fn test_orderbook_basic() {
    let spec = CoreSymbolSpecification {
        symbol_id: 1,
        symbol_type: SymbolType::CurrencyExchangePair,
        base_currency: 1,
        quote_currency: 2,
        base_scale_k: 100,
        quote_scale_k: 1,
        ..Default::default()
    };

    println!("  Testing NaiveOrderBook...");
    let mut naive = NaiveOrderBook::new(spec.clone());
    run_book_logic(&mut naive);

    println!("  Testing DirectOrderBook...");
    let mut direct = DirectOrderBook::new(spec);
    run_book_logic(&mut direct);
}

fn run_book_logic(book: &mut dyn OrderBook) {
    // 1. 测试限价单挂单
    let mut cmd1 = OrderCommand {
        command: OrderCommandType::PlaceOrder,
        uid: 101,
        order_id: 1,
        price: 100,
        size: 10,
        action: OrderAction::Ask,
        order_type: OrderType::Gtc,
        ..Default::default()
    };
    cmd1.result_code = CommandResultCode::ValidForMatchingEngine;
    let res1 = book.new_order(&mut cmd1);
    assert_eq!(res1, CommandResultCode::Success);

    // 2. 测试成交
    let mut cmd2 = OrderCommand {
        command: OrderCommandType::PlaceOrder,
        uid: 102,
        order_id: 2,
        price: 100,
        size: 4,
        action: OrderAction::Bid,
        order_type: OrderType::Gtc,
        ..Default::default()
    };
    cmd2.result_code = CommandResultCode::ValidForMatchingEngine;
    let res2 = book.new_order(&mut cmd2);
    assert_eq!(res2, CommandResultCode::Success);
    assert_eq!(cmd2.matcher_events.len(), 1);
    assert_eq!(cmd2.matcher_events[0].size, 4);

    // 3. 测试移动订单
    let mut cmd_move = OrderCommand {
        command: OrderCommandType::MoveOrder,
        uid: 101,
        order_id: 1,
        price: 110,
        ..Default::default()
    };
    let res_move = book.move_order(&mut cmd_move);
    assert_eq!(res_move, CommandResultCode::Success);

    // 4. 测试减少订单
    let mut cmd_reduce = OrderCommand {
        command: OrderCommandType::ReduceOrder,
        uid: 101,
        order_id: 1,
        size: 2, // 减少 2 个单位
        ..Default::default()
    };
    let res_reduce = book.reduce_order(&mut cmd_reduce);
    assert_eq!(res_reduce, CommandResultCode::Success);

    // 5. 测试 L2 行情
    let l2 = book.get_l2_data(5);
    assert!(!l2.ask_prices.is_empty() || !l2.bid_prices.is_empty());
    println!("    L2 Levels: Asks={}, Bids={}", l2.ask_prices.len(), l2.bid_prices.len());

    // 6. 测试取消订单
    let mut cmd3 = OrderCommand {
        command: OrderCommandType::CancelOrder,
        uid: 101,
        order_id: 1,
        ..Default::default()
    };
    let res = book.cancel_order(&mut cmd3);
    assert_eq!(res, CommandResultCode::Success);
    
    println!("    OrderBook logic passed.");
}
