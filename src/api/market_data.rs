use crate::api::*;

/// L2 市场深度数据
#[derive(Debug, Clone)]
pub struct L2MarketData {
    pub ask_prices: Vec<Price>,
    pub ask_volumes: Vec<Size>,
    pub bid_prices: Vec<Price>,
    pub bid_volumes: Vec<Size>,
}

impl L2MarketData {
    pub fn new(depth: usize) -> Self {
        Self {
            ask_prices: Vec::with_capacity(depth),
            ask_volumes: Vec::with_capacity(depth),
            bid_prices: Vec::with_capacity(depth),
            bid_volumes: Vec::with_capacity(depth),
        }
    }
}
