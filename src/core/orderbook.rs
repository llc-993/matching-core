use crate::api::*;
use serde::{Deserialize, Serialize};

pub mod naive;
pub mod direct;
pub mod direct_optimized;
pub mod advanced;
pub mod simd_utils;

pub use naive::NaiveOrderBook;
pub use direct::DirectOrderBook;
pub use direct_optimized::DirectOrderBookOptimized;
pub use advanced::AdvancedOrderBook;

#[derive(Serialize, Deserialize)]
pub enum OrderBookState {
    Naive(NaiveOrderBook),
    Direct(DirectOrderBook),
    DirectOptimized(DirectOrderBookOptimized),
    Advanced(AdvancedOrderBook),
}

pub trait OrderBook: Send {
    fn new_order(&mut self, cmd: &mut OrderCommand) -> CommandResultCode;
    fn cancel_order(&mut self, cmd: &mut OrderCommand) -> CommandResultCode;
    fn move_order(&mut self, cmd: &mut OrderCommand) -> CommandResultCode;
    fn reduce_order(&mut self, cmd: &mut OrderCommand) -> CommandResultCode;
    fn get_symbol_spec(&self) -> &CoreSymbolSpecification;
    fn get_l2_data(&self, depth: usize) -> L2MarketData;
    
    // 查询方法
    fn get_order_by_id(&self, order_id: OrderId) -> Option<(Price, OrderAction)>;
    fn get_total_ask_volume(&self) -> Size;
    fn get_total_bid_volume(&self) -> Size;
    fn get_ask_buckets_count(&self) -> usize;
    fn get_bid_buckets_count(&self) -> usize;

    // 序列化支持
    fn serialize_state(&self) -> OrderBookState;
}
