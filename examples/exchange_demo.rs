use matching_core::api::*;
use matching_core::core::exchange::{ExchangeConfig, ExchangeCore};
use std::sync::Arc;

fn main() {
    let config = ExchangeConfig::default();
    let mut core = ExchangeCore::new(config);

    core.set_result_consumer(Arc::new(|cmd| {
        println!("结果: {:?} - {:?}", cmd.command, cmd.result_code);
    }));

    // 添加交易对
    core.add_symbol(CoreSymbolSpecification {
        symbol_id: 100,
        symbol_type: SymbolType::CurrencyExchangePair,
        base_currency: 2,
        quote_currency: 1,
        base_scale_k: 100,
        quote_scale_k: 1,
        taker_fee: 10,
        maker_fee: 5,
        margin_buy: 0,
        margin_sell: 0,
    });

    // 添加用户
    core.submit_command(OrderCommand {
        command: OrderCommandType::AddUser,
        uid: 1001,
        ..Default::default()
    });
    core.submit_command(OrderCommand {
        command: OrderCommandType::AddUser,
        uid: 1002,
        ..Default::default()
    });

    // 充值
    core.submit_command(OrderCommand {
        command: OrderCommandType::BalanceAdjustment,
        uid: 1001,
        symbol: 1,
        price: 100000,
        order_id: 1,
        ..Default::default()
    });
    core.submit_command(OrderCommand {
        command: OrderCommandType::BalanceAdjustment,
        uid: 1002,
        symbol: 2,
        price: 10000,
        order_id: 2,
        ..Default::default()
    });

    println!("\n=== 测试完整撮合流程 ===\n");

    // 1. 挂多个卖单
    core.submit_command(OrderCommand {
        command: OrderCommandType::PlaceOrder,
        uid: 1002,
        order_id: 5002,
        symbol: 100,
        price: 100,
        size: 5,
        action: OrderAction::Ask,
        order_type: OrderType::Gtc,
        ..Default::default()
    });

    core.submit_command(OrderCommand {
        command: OrderCommandType::PlaceOrder,
        uid: 1002,
        order_id: 5003,
        symbol: 100,
        price: 105,
        size: 5,
        action: OrderAction::Ask,
        order_type: OrderType::Gtc,
        ..Default::default()
    });

    // 2. FOK_BUDGET 订单测试
    println!("测试 FOK_BUDGET 订单:");
    let fok_result = core.submit_command(OrderCommand {
        command: OrderCommandType::PlaceOrder,
        uid: 1001,
        order_id: 5010,
        symbol: 100,
        price: 1050, // 预算：10 * 100 + 0 * 105 = 1000 < 1050 (满足)
        reserve_price: 105,
        size: 10,
        action: OrderAction::Bid,
        order_type: OrderType::FokBudget,
        ..Default::default()
    });
    println!("  结果: {:?}, 事件数: {}", fok_result.result_code, fok_result.matcher_events.len());

    // 3. 测试重复订单 ID
    println!("\n测试重复订单 ID:");
    core.submit_command(OrderCommand {
        command: OrderCommandType::PlaceOrder,
        uid: 1002,
        order_id: 5020,
        symbol: 100,
        price: 110,
        size: 3,
        action: OrderAction::Ask,
        order_type: OrderType::Gtc,
        ..Default::default()
    });

    let dup_result = core.submit_command(OrderCommand {
        command: OrderCommandType::PlaceOrder,
        uid: 1001,
        order_id: 5020, // 重复 ID
        symbol: 100,
        price: 110,
        reserve_price: 110,
        size: 3,
        action: OrderAction::Bid,
        order_type: OrderType::Gtc,
        ..Default::default()
    });
    println!("  重复订单结果: {:?}, 事件数: {}", dup_result.result_code, dup_result.matcher_events.len());

    // 4. 测试移动订单
    println!("\n测试移动订单:");
    core.submit_command(OrderCommand {
        command: OrderCommandType::PlaceOrder,
        uid: 1002,
        order_id: 5030,
        symbol: 100,
        price: 120,
        size: 5,
        action: OrderAction::Ask,
        order_type: OrderType::Gtc,
        ..Default::default()
    });

    let move_result = core.submit_command(OrderCommand {
        command: OrderCommandType::MoveOrder,
        uid: 1002,
        order_id: 5030,
        symbol: 100,
        price: 115, // 移动到新价格
        ..Default::default()
    });
    println!("  移动订单结果: {:?}", move_result.result_code);

    // 5. 测试减少订单
    println!("\n测试减少订单:");
    let reduce_result = core.submit_command(OrderCommand {
        command: OrderCommandType::ReduceOrder,
        uid: 1002,
        order_id: 5030,
        symbol: 100,
        size: 2, // 减少 2
        ..Default::default()
    });
    println!("  减少订单结果: {:?}", reduce_result.result_code);
    // 6. 测试序列化与快照
    println!("\n=== 测试二进制序列化 (Bincode) ===\n");
    let state = core.serialize_state();
    let serialized = bincode::serialize(&state).expect("序列化失败");
    println!("序列化成功，字节大小: {}", serialized.len());

    let deserialized_state: matching_core::core::exchange::ExchangeState = 
        bincode::deserialize(&serialized).expect("反序列化失败");
    
    let mut core2 = ExchangeCore::from_state(deserialized_state);
    println!("从快照恢复成功");

    // 在恢复的核心上继续测试
    core2.set_result_consumer(Arc::new(|cmd| {
        println!("恢复核心结果: {:?} - {:?}", cmd.command, cmd.result_code);
    }));

    println!("在恢复的核心上提交新订单:");
    core2.submit_command(OrderCommand {
        command: OrderCommandType::PlaceOrder,
        uid: 1001,
        order_id: 6001,
        symbol: 100,
        price: 115,
        reserve_price: 115,
        size: 3,
        action: OrderAction::Bid,
        order_type: OrderType::Gtc,
        ..Default::default()
    });

    // 7. 测试 Journaling (WAL)
    println!("\n=== 测试预写日志 (WAL) ===\n");
    let journal_path = "exchange.wal";
    
    // 如果文件已存在则删除，确保干净测试
    let _ = std::fs::remove_file(journal_path);

    let mut core_wal = ExchangeCore::new(ExchangeConfig::default());
    core_wal.add_symbol(CoreSymbolSpecification {
        symbol_id: 200,
        symbol_type: SymbolType::CurrencyExchangePair,
        base_currency: 2,
        quote_currency: 1,
        base_scale_k: 100,
        quote_scale_k: 1,
        taker_fee: 10,
        maker_fee: 5,
        ..Default::default()
    });
    
    core_wal.enable_journaling(journal_path).expect("启用 WAL 失败");
    
    println!("提交 WAL 记录的命令...");
    core_wal.submit_command(OrderCommand {
        command: OrderCommandType::AddUser,
        uid: 2001,
        ..Default::default()
    });
    core_wal.submit_command(OrderCommand {
        command: OrderCommandType::BalanceAdjustment,
        uid: 2001,
        symbol: 1,
        price: 500000,
        ..Default::default()
    });

    println!("从日志恢复到新核心...");
    let mut core_recovered = ExchangeCore::new(ExchangeConfig::default());
    core_recovered.add_symbol(CoreSymbolSpecification {
        symbol_id: 200,
        symbol_type: SymbolType::CurrencyExchangePair,
        base_currency: 2,
        quote_currency: 1,
        base_scale_k: 100,
        quote_scale_k: 1,
        taker_fee: 10,
        maker_fee: 5,
        ..Default::default()
    });

    core_recovered.replay_journal(journal_path).expect("重放 WAL 失败");
    println!("WAL 恢复成功");

    // 清理测试文件
    let _ = std::fs::remove_file(journal_path);

    // 8. 测试 Snapshotting Mechanism
    println!("\n=== 测试状态快照 (Snapshotting) ===\n");
    let snapshot_dir = "snapshots";
    
    // 清理之前的快照目录
    let _ = std::fs::remove_dir_all(snapshot_dir);

    let mut core_snap = ExchangeCore::new(ExchangeConfig::default());
    core_snap.enable_snapshotting(snapshot_dir).expect("启用快照失败");
    
    core_snap.add_symbol(CoreSymbolSpecification {
        symbol_id: 300,
        symbol_type: SymbolType::CurrencyExchangePair,
        base_currency: 2,
        quote_currency: 1,
        base_scale_k: 100,
        quote_scale_k: 1,
        taker_fee: 10,
        maker_fee: 5,
        ..Default::default()
    });

    println!("生成快照 (ID: 1)...");
    core_snap.take_snapshot(1).expect("生成快照失败");

    println!("从快照目录恢复到新核心...");
    let mut core_restored = ExchangeCore::new(ExchangeConfig::default());
    core_restored.enable_snapshotting(snapshot_dir).expect("新核心启用快照失败");
    
    let recovered = core_restored.load_latest_snapshot().expect("加载快照失败");
    assert!(recovered, "未找到有效的快照");
    println!("从快照恢复成功");

    // 清理测试目录
    let _ = std::fs::remove_dir_all(snapshot_dir);
}
