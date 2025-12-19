use crate::api::*;
use crate::core::processors::risk_engine::RiskEngine;

pub fn test_risk_engine_basic() {
    let mut engine = RiskEngine::new(0, 1);
    
    // 1. 添加交易对
    engine.add_symbol(CoreSymbolSpecification {
        symbol_id: 1,
        base_currency: 1,
        quote_currency: 2,
        ..Default::default()
    });

    // 2. 添加用户和充值
    let mut cmd_user = OrderCommand {
        command: OrderCommandType::AddUser,
        uid: 1001,
        ..Default::default()
    };
    engine.pre_process(&mut cmd_user);
    assert_eq!(cmd_user.result_code, CommandResultCode::Success);

    let mut cmd_balance = OrderCommand {
        command: OrderCommandType::BalanceAdjustment,
        uid: 1001,
        symbol: 2, // 充值报价币
        price: 10000,
        ..Default::default()
    };
    cmd_balance.result_code = CommandResultCode::ValidForMatchingEngine;
    engine.pre_process(&mut cmd_balance);
    assert_eq!(cmd_balance.result_code, CommandResultCode::Success);

    // 3. 测试余额不足
    let mut cmd_order_nsf = OrderCommand {
        command: OrderCommandType::PlaceOrder,
        uid: 1001,
        symbol: 1,
        price: 200,
        size: 100, // 200 * 100 = 20000 > 10000
        action: OrderAction::Bid,
        reserve_price: 200,
        ..Default::default()
    };
    cmd_order_nsf.result_code = CommandResultCode::ValidForMatchingEngine;
    engine.pre_process(&mut cmd_order_nsf);
    assert_eq!(cmd_order_nsf.result_code, CommandResultCode::RiskNsf);
    // 4. 测试手动结算 (PostProcess)
    let mut cmd_settle = OrderCommand {
        command: OrderCommandType::PlaceOrder,
        uid: 1001,
        symbol: 1,
        price: 150,
        size: 10,
        action: OrderAction::Ask,
        ..Default::default()
    };
    // 模拟成交 5 个
    cmd_settle.matcher_events.push(MatcherTradeEvent {
        event_type: MatcherEventType::Trade,
        size: 5,
        price: 150,
        ..Default::default()
    });
    engine.post_process(&mut cmd_settle);
    assert_eq!(cmd_settle.result_code, CommandResultCode::Success);

    // 5. 测试不存在的用户
    let mut cmd_invalid = OrderCommand {
        command: OrderCommandType::PlaceOrder,
        uid: 9999,
        ..Default::default()
    };
    cmd_invalid.result_code = CommandResultCode::ValidForMatchingEngine;
    engine.pre_process(&mut cmd_invalid);
    assert_eq!(cmd_invalid.result_code, CommandResultCode::AuthInvalidUser);
    
    println!("    RiskEngine logic passed.");
}
