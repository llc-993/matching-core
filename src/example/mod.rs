pub mod orderbook_test;
pub mod risk_test;
pub mod integration_test;

pub fn run_all_tests() {
    println!("--- Running OrderBook Tests ---");
    orderbook_test::test_orderbook_basic();
    
    println!("\n--- Running Risk Engine Tests ---");
    risk_test::test_risk_engine_basic();
    
    println!("\n--- Running Integration Tests ---");
    integration_test::test_full_flow();
}
