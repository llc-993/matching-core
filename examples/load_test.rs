use matching_core::api::*;
use matching_core::core::exchange::{ExchangeCore, ExchangeConfig, ProducerType, WaitStrategyType};
use std::time::{Duration, Instant};
use std::sync::Arc;

/// 压力测试配置
struct LoadTestConfig {
    num_orders: usize,
    batch_size: usize,
}

fn main() {
    println!("=== Matching Engine 性能压力测试 (TPS/QPS) ===\n");

    let config = LoadTestConfig {
        num_orders: 1_000_000, // 100万个订单
        batch_size: 1000,
    };

    println!("测试规模: {} 订单", config.num_orders);

    // 1. 测试 DirectOrderBook (核心业务流水线)
    run_load_test("DirectOrderBook + Pipeline", &config);
    
    // 2. 环境说明
    println!("\n测试完成。请注意，性能受硬件环境、CPU 亲和性设置及 L2 数据更新频率影响。");
}

fn run_load_test(name: &str, config: &LoadTestConfig) {
    let exchange_config = ExchangeConfig {
        ring_buffer_size: 64 * 1024,
        matching_engines_num: 1,
        risk_engines_num: 1,
        producer_type: ProducerType::Single,
        wait_strategy: WaitStrategyType::BusySpin,
    };
    
    let mut core = ExchangeCore::new(exchange_config);
    
    // 使用原子计数器跟踪处理进度
    use std::sync::atomic::{AtomicUsize, Ordering};
    let processed_count = Arc::new(AtomicUsize::new(0));
    let count_clone = processed_count.clone();
    
    core.set_result_consumer(Arc::new(move |_cmd| {
        count_clone.fetch_add(1, Ordering::SeqCst);
    }));

    // 初始化交易对 (启动前进行以便同步地设置到 Pipeline)
    core.add_symbol(CoreSymbolSpecification {
        symbol_id: 1,
        symbol_type: SymbolType::CurrencyExchangePair,
        base_currency: 1,
        quote_currency: 2,
        base_scale_k: 100,
        quote_scale_k: 1,
        taker_fee: 10,
        maker_fee: 5,
        ..Default::default()
    });

    // 启动异步流水线
    core.startup();
    
    // 初始化大量用户并充值
    println!("[{}] 正在初始化环境...", name);
    let init_user_count = 10000;
    for uid in 1..=init_user_count {
        core.submit_command(OrderCommand {
            command: OrderCommandType::AddUser,
            uid,
            ..Default::default()
        });
        core.submit_command(OrderCommand {
            command: OrderCommandType::BalanceAdjustment,
            uid,
            symbol: 2,
            price: 10_000_000,
            ..Default::default()
        });
        core.submit_command(OrderCommand {
            command: OrderCommandType::BalanceAdjustment,
            uid,
            symbol: 1,
            price: 10_000_000,
            ..Default::default()
        });
    }

    // 预热阶段
    println!("[{}] 正在预热...", name);
    let warmup_count = 1000;
    for i in 0..warmup_count {
        simulate_order(&mut core, i);
    }

    // 正式测试
    println!("[{}] 正在进行压力测试...", name);
    let start = Instant::now();
    
    for i in 0..config.num_orders {
        simulate_order(&mut core, i as u64);
    }
    
    // 等待所有异步消息处理完毕 (init + warmup + num_orders)
    // 注意：init 包含 3*init_user_count
    let expected = 3 * (init_user_count as usize) + (warmup_count as usize) + (config.num_orders as usize);
    while processed_count.load(Ordering::Acquire) < expected {
        std::hint::spin_loop();
    }
    
    let duration = start.elapsed();
    let tps = config.num_orders as f64 / duration.as_secs_f64();
    let latency_avg = duration.as_nanos() as f64 / config.num_orders as f64;

    println!("\n--- {} 结果 ---", name);
    println!("总用时: {:?}", duration);
    println!("平均吞吐量 (TPS): {:.2}", tps);
    println!("平均延迟: {:.2} ns", latency_avg);
}

#[inline(always)]
fn simulate_order(core: &mut ExchangeCore, i: u64) {
    let uid = (i % 10000) + 1;
    let action = if i % 2 == 0 { OrderAction::Bid } else { OrderAction::Ask };
    
    // 模拟真实的买卖交替，订单 ID 递增
    core.submit_command(OrderCommand {
        command: OrderCommandType::PlaceOrder,
        uid,
        order_id: i + 100_000,
        symbol: 1,
        price: 100 + (i % 10) as i64,
        size: 1,
        action,
        order_type: OrderType::Gtc,
        ..Default::default()
    });
}
