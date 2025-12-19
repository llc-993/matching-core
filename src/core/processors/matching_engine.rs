use crate::api::*;
use crate::core::orderbook::{OrderBook, OrderBookState};
use ahash::AHashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub struct MatchingEngineState {
    pub shard_id: usize,
    pub shard_mask: i32,
    pub order_books: HashMap<SymbolId, OrderBookState>, // 序列化使用标准 HashMap
}

pub struct MatchingEngineRouter {
    shard_id: usize,
    shard_mask: i32,
    order_books: AHashMap<SymbolId, Box<dyn OrderBook>>,
}

impl MatchingEngineRouter {
    pub fn serialize_state(&self) -> MatchingEngineState {
        let mut books_state = HashMap::new();
        for (symbol_id, book) in &self.order_books {
            books_state.insert(*symbol_id, book.serialize_state());
        }
        MatchingEngineState {
            shard_id: self.shard_id,
            shard_mask: self.shard_mask,
            order_books: books_state,
        }
    }

    pub fn from_state(state: MatchingEngineState) -> Self {
        let mut order_books = AHashMap::new(); // 运行时使用 AHashMap
        for (symbol_id, book_state) in state.order_books {
            let book: Box<dyn OrderBook> = match book_state {
                OrderBookState::Naive(book) => Box::new(book),
                OrderBookState::Direct(book) => Box::new(book),
                OrderBookState::DirectOptimized(book) => Box::new(book),
                OrderBookState::Advanced(book) => Box::new(book),
            };
            order_books.insert(symbol_id, book);
        }
        Self {
            shard_id: state.shard_id,
            shard_mask: state.shard_mask,
            order_books,
        }
    }

    pub fn new(shard_id: usize, num_shards: usize) -> Self {
        assert!(num_shards.is_power_of_two());
        Self {
            shard_id,
            shard_mask: (num_shards - 1) as i32,
            order_books: AHashMap::new(),
        }
    }

    fn symbol_for_this_shard(&self, symbol: SymbolId) -> bool {
        self.shard_mask == 0 || (symbol & self.shard_mask) == self.shard_id as i32
    }

    pub fn add_symbol(&mut self, spec: CoreSymbolSpecification) {
        use crate::core::orderbook::DirectOrderBook;
        self.order_books.insert(spec.symbol_id, Box::new(DirectOrderBook::new(spec)));
    }

    pub fn process_order(&mut self, cmd: &mut OrderCommand) {
        // 如果已经有结果码（测试用），跳过撮合
        if cmd.result_code == CommandResultCode::Success {
            return;
        }

        match cmd.command {
            OrderCommandType::PlaceOrder
            | OrderCommandType::CancelOrder
            | OrderCommandType::MoveOrder
            | OrderCommandType::ReduceOrder => {
                if self.symbol_for_this_shard(cmd.symbol) {
                    self.process_matching_command(cmd);
                }
            }
            _ => {}
        }
    }

    fn process_matching_command(&mut self, cmd: &mut OrderCommand) {
        let Some(book) = self.order_books.get_mut(&cmd.symbol) else {
            cmd.result_code = CommandResultCode::MatchingInvalidOrderBookId;
            return;
        };

        match cmd.command {
            OrderCommandType::PlaceOrder => {
                if cmd.result_code == CommandResultCode::ValidForMatchingEngine {
                    book.new_order(cmd);
                    cmd.result_code = CommandResultCode::Success;
                }
            }
            OrderCommandType::CancelOrder => {
                cmd.result_code = book.cancel_order(cmd);
            }
            OrderCommandType::MoveOrder => {
                cmd.result_code = book.move_order(cmd);
            }
            OrderCommandType::ReduceOrder => {
                cmd.result_code = book.reduce_order(cmd);
            }
            _ => {
                cmd.result_code = CommandResultCode::MatchingUnsupportedCommand;
            }
        }
    }
}
