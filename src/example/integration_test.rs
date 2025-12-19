use crate::api::*;
use crate::core::exchange::{ExchangeConfig, ExchangeCore};

pub fn test_full_flow() {
    let config = ExchangeConfig::default();
    let mut core = ExchangeCore::new(config);
    
    // 1. 初始化交易对和用户
    core.add_symbol(CoreSymbolSpecification {
        symbol_id: 1,
        symbol_type: SymbolType::CurrencyExchangePair,
        base_currency: 1,
        quote_currency: 2,
        base_scale_k: 100,
        quote_scale_k: 1,
        ..Default::default()
    });

    core.submit_command(OrderCommand {
        command: OrderCommandType::AddUser,
        uid: 1,
        ..Default::default()
    });

    core.submit_command(OrderCommand {
        command: OrderCommandType::BalanceAdjustment,
        uid: 1,
        symbol: 2,
        price: 1000000,
        ..Default::default()
    });

    // 2. 测试 WAL 和快照组合
    let journal_path = "integration.wal";
    let snapshot_dir = "integration_snapshots";
    let _ = std::fs::remove_file(journal_path);
    let _ = std::fs::remove_dir_all(snapshot_dir);

    core.enable_journaling(journal_path).unwrap();
    core.enable_snapshotting(snapshot_dir).unwrap();

    println!("    Placing orders with WAL...");
    core.submit_command(OrderCommand {
        command: OrderCommandType::PlaceOrder,
        uid: 1,
        order_id: 1,
        price: 1000,
        size: 10,
        action: OrderAction::Bid,
        ..Default::default()
    });

    println!("    Taking snapshot...");
    core.take_snapshot(1).unwrap();

    println!("    Recovering into new core...");
    let mut core2 = ExchangeCore::new(ExchangeConfig::default());
    core2.enable_snapshotting(snapshot_dir).unwrap();
    let recovered = core2.load_latest_snapshot().unwrap();
    assert!(recovered);

    // 清理
    let _ = std::fs::remove_file(journal_path);
    let _ = std::fs::remove_dir_all(snapshot_dir);
    
    println!("    Integration test passed.");
}
