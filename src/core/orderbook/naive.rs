use crate::api::*;
use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// 订单记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub order_id: OrderId,
    pub uid: UserId,
    pub price: Price,
    pub size: Size,
    pub filled: Size,
    pub action: OrderAction,
    pub reserve_price: Price,
    pub timestamp: i64,
    pub user_cookie: i64, // 用户自定义标记（对标 exchange-core）
}

impl Order {
    pub fn remaining(&self) -> Size {
        self.size - self.filled
    }
}

/// 价格档位（存储同价订单，栈内存优化：大部分价格层 <= 8 个订单）
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrdersBucket {
    price: Price,
    orders: SmallVec<[Order; 8]>,
    total_volume: Size,
}

impl OrdersBucket {
    fn new(price: Price) -> Self {
        Self {
            price,
            orders: SmallVec::new(),
            total_volume: 0,
        }
    }

    fn add(&mut self, order: Order) {
        self.total_volume += order.remaining();
        self.orders.push(order);
    }

    fn remove(&mut self, order_id: OrderId) -> Option<Order> {
        if let Some(pos) = self.orders.iter().position(|o| o.order_id == order_id) {
            let order = self.orders.remove(pos);
            self.total_volume -= order.remaining();
            Some(order)
        } else {
            None
        }
    }

    /// 撮合：返回成交量和事件
    fn match_order(&mut self, taker_size: Size, _taker_uid: UserId) -> (Size, SmallVec<[MatcherTradeEvent; 4]>) {
        let mut matched_size = 0;
        let mut events = SmallVec::new();
        let mut to_remove = SmallVec::<[OrderId; 4]>::new();

        for order in &mut self.orders {
            let remaining = order.remaining();
            let match_size = remaining.min(taker_size - matched_size);

            if match_size > 0 {
                order.filled += match_size;
                matched_size += match_size;

                events.push(MatcherTradeEvent::new_trade(
                    match_size,
                    self.price,
                    order.order_id,
                    order.uid,
                    order.reserve_price,
                ));

                if order.filled == order.size {
                    to_remove.push(order.order_id);
                }

                if matched_size == taker_size {
                    break;
                }
            }
        }

        // 移除完全成交的订单
        for oid in to_remove {
            self.remove(oid);
        }

        (matched_size, events)
    }
}

use ahash::AHashMap;

/// 简单订单簿实现（性能优化版）
#[derive(Clone, Serialize, Deserialize)]
pub struct NaiveOrderBook {
    symbol_spec: CoreSymbolSpecification,
    ask_buckets: BTreeMap<Price, OrdersBucket>, // 卖单（价格升序）
    bid_buckets: BTreeMap<Price, OrdersBucket>, // 买单（价格降序）
    order_map: AHashMap<OrderId, (Price, OrderAction)>, // 运行时使用 AHashMap
    
    // 性能优化：缓存最优价格
    best_ask_price: Option<Price>,
    best_bid_price: Option<Price>,
}

impl NaiveOrderBook {
    pub fn new(spec: CoreSymbolSpecification) -> Self {
        Self {
            symbol_spec: spec,
            ask_buckets: BTreeMap::new(),
            bid_buckets: BTreeMap::new(),
            order_map: AHashMap::with_capacity(1024), // 预分配容量
            best_ask_price: None,
            best_bid_price: None,
        }
    }
    
    #[inline]
    fn update_best_prices(&mut self) {
        self.best_ask_price = self.ask_buckets.keys().next().copied();
        self.best_bid_price = self.bid_buckets.keys().next_back().copied();
    }

    /// GTC 下单
    fn place_gtc(&mut self, cmd: &mut OrderCommand) {
        // 检查重复订单 ID
        if self.order_map.contains_key(&cmd.order_id) {
            let filled = self.try_match(cmd);
            if filled < cmd.size {
                cmd.matcher_events.push(MatcherTradeEvent::new_reject(cmd.size - filled, cmd.price));
            }
            return;
        }

        // 1. 尝试立即撮合
        let filled = self.try_match(cmd);

        // 2. 如果未完全成交，挂单
        if filled < cmd.size {
            let order = Order {
                order_id: cmd.order_id,
                uid: cmd.uid,
                price: cmd.price,
                size: cmd.size,
                filled,
                action: cmd.action,
                reserve_price: cmd.reserve_price,
                timestamp: cmd.timestamp,
                user_cookie: 0,
            };

            self.order_map.insert(cmd.order_id, (cmd.price, cmd.action));

            match cmd.action {
                OrderAction::Ask => {
                    self.ask_buckets
                        .entry(cmd.price)
                        .or_insert_with(|| OrdersBucket::new(cmd.price))
                        .add(order);
                    if self.best_ask_price.is_none() || cmd.price < self.best_ask_price.unwrap() {
                        self.best_ask_price = Some(cmd.price);
                    }
                }
                OrderAction::Bid => {
                    self.bid_buckets
                        .entry(cmd.price)
                        .or_insert_with(|| OrdersBucket::new(cmd.price))
                        .add(order);
                    if self.best_bid_price.is_none() || cmd.price > self.best_bid_price.unwrap() {
                        self.best_bid_price = Some(cmd.price);
                    }
                }
            }
        }
    }

    /// IOC 下单
    fn place_ioc(&mut self, cmd: &mut OrderCommand) {
        let filled = self.try_match(cmd);
        let rejected = cmd.size - filled;

        if rejected > 0 {
            cmd.matcher_events.push(MatcherTradeEvent::new_reject(rejected, cmd.price));
        }
    }

    /// FOK_BUDGET 下单（预算限制的全部成交或取消）
    fn place_fok_budget(&mut self, cmd: &mut OrderCommand) {
        // 计算需要的预算
        let budget = self.check_budget_to_fill(cmd.size, cmd.action);

        if let Some(calculated_budget) = budget {
            // 检查预算是否满足
            if self.is_budget_satisfied(cmd.action, calculated_budget, cmd.price) {
                self.try_match(cmd);
            } else {
                cmd.matcher_events.push(MatcherTradeEvent::new_reject(cmd.size, cmd.price));
            }
        } else {
            // 流动性不足
            cmd.matcher_events.push(MatcherTradeEvent::new_reject(cmd.size, cmd.price));
        }
    }

    /// 检查预算是否满足
    fn is_budget_satisfied(&self, action: OrderAction, calculated: i64, limit: i64) -> bool {
        calculated == limit || (action == OrderAction::Bid) != (calculated > limit)
    }

    /// 计算填充订单所需的预算
    fn check_budget_to_fill(&self, mut size: Size, action: OrderAction) -> Option<i64> {
        let buckets = match action {
            OrderAction::Ask => &self.bid_buckets,
            OrderAction::Bid => &self.ask_buckets,
        };

        let mut budget: i64 = 0;

        for (price, bucket) in buckets.iter() {
            let available = bucket.total_volume;

            if size > available {
                size -= available;
                budget += (available * price) as i64;
            } else {
                budget += (size * price) as i64;
                return Some(budget);
            }
        }

        None // 流动性不足
    }

    /// 尝试撮合（性能优化版：减少Vec分配）
    fn try_match(&mut self, cmd: &mut OrderCommand) -> Size {
        let mut filled = 0;
        let is_budget_order = matches!(cmd.order_type, OrderType::FokBudget | OrderType::IocBudget);

        match cmd.action {
            OrderAction::Bid => {
                // 买单：匹配卖单（从低到高）
                // 优化：提前检查最优价格，避免不必要的迭代
                if !is_budget_order {
                    if let Some(best_ask) = self.best_ask_price {
                        if best_ask > cmd.price {
                            return 0; // 无法成交
                        }
                    } else {
                        return 0; // 无卖单
                    }
                }

                let prices_to_match: Vec<Price> = if is_budget_order {
                    self.ask_buckets.keys().copied().collect()
                } else {
                    self.ask_buckets.range(..=cmd.price).map(|(p, _)| *p).collect()
                };

                for price in prices_to_match {
                    if filled >= cmd.size {
                        break;
                    }

                    if let Some(bucket) = self.ask_buckets.get_mut(&price) {
                        let (matched, events) = bucket.match_order(cmd.size - filled, cmd.uid);
                        filled += matched;
                        cmd.matcher_events.extend(events);

                        if bucket.total_volume == 0 {
                            self.ask_buckets.remove(&price);
                        }
                    }
                }
                self.update_best_prices();
            }
            OrderAction::Ask => {
                // 卖单：匹配买单（从高到低）
                if !is_budget_order {
                    if let Some(best_bid) = self.best_bid_price {
                        if best_bid < cmd.price {
                            return 0; // 无法成交
                        }
                    } else {
                        return 0; // 无买单
                    }
                }

                let prices_to_match: Vec<Price> = if is_budget_order {
                    self.bid_buckets.keys().rev().copied().collect()
                } else {
                    self.bid_buckets.range(cmd.price..).rev().map(|(p, _)| *p).collect()
                };

                for price in prices_to_match {
                    if filled >= cmd.size {
                        break;
                    }

                    if let Some(bucket) = self.bid_buckets.get_mut(&price) {
                        let (matched, events) = bucket.match_order(cmd.size - filled, cmd.uid);
                        filled += matched;
                        cmd.matcher_events.extend(events);

                        if bucket.total_volume == 0 {
                            self.bid_buckets.remove(&price);
                        }
                    }
                }
                self.update_best_prices();
            }
        }

        filled
    }
}

impl super::OrderBook for NaiveOrderBook {
    fn new_order(&mut self, cmd: &mut OrderCommand) -> CommandResultCode {
        match cmd.order_type {
            OrderType::Gtc => {
                self.place_gtc(cmd);
            }
            OrderType::Ioc => {
                self.place_ioc(cmd);
            }
            OrderType::FokBudget => {
                self.place_fok_budget(cmd);
            }
            _ => {
                return CommandResultCode::MatchingUnsupportedCommand;
            }
        }
        CommandResultCode::Success
    }

    fn cancel_order(&mut self, cmd: &mut OrderCommand) -> CommandResultCode {
        let Some((price, action)) = self.order_map.remove(&cmd.order_id) else {
            return CommandResultCode::MatchingUnknownOrderId;
        };

        let buckets = match action {
            OrderAction::Ask => &mut self.ask_buckets,
            OrderAction::Bid => &mut self.bid_buckets,
        };

        if let Some(bucket) = buckets.get_mut(&price) {
            if let Some(order) = bucket.remove(cmd.order_id) {
                cmd.matcher_events.push(MatcherTradeEvent::new_reject(order.remaining(), price));
                cmd.action = action;

                if bucket.total_volume == 0 {
                    buckets.remove(&price);
                    self.update_best_prices(); // 更新最优价格缓存
                }

                return CommandResultCode::Success;
            }
        }

        CommandResultCode::MatchingUnknownOrderId
    }

    fn move_order(&mut self, cmd: &mut OrderCommand) -> CommandResultCode {
        let Some((old_price, action)) = self.order_map.get(&cmd.order_id).copied() else {
            return CommandResultCode::MatchingUnknownOrderId;
        };

        // 风险检查：买单不能超过预留价格
        if self.symbol_spec.symbol_type == SymbolType::CurrencyExchangePair
            && action == OrderAction::Bid
        {
            let buckets = &self.bid_buckets;
            if let Some(bucket) = buckets.get(&old_price) {
                if let Some(order) = bucket.orders.iter().find(|o| o.order_id == cmd.order_id) {
                    if cmd.price > order.reserve_price {
                        return CommandResultCode::RiskInvalidReserveBidPrice;
                    }
                }
            }
        }

        // 取出订单
        let buckets = match action {
            OrderAction::Ask => &mut self.ask_buckets,
            OrderAction::Bid => &mut self.bid_buckets,
        };

        let mut order = if let Some(bucket) = buckets.get_mut(&old_price) {
            if let Some(order) = bucket.remove(cmd.order_id) {
                if bucket.total_volume == 0 {
                    buckets.remove(&old_price);
                }
                order
            } else {
                return CommandResultCode::MatchingUnknownOrderId;
            }
        } else {
            return CommandResultCode::MatchingUnknownOrderId;
        };

        // 更新价格
        self.order_map.insert(cmd.order_id, (cmd.price, action));
        order.price = cmd.price;
        cmd.action = action;

        // 尝试撮合
        let filled_before = order.filled;
        let mut temp_cmd = OrderCommand {
            uid: order.uid,
            order_id: order.order_id,
            symbol: cmd.symbol,
            price: cmd.price,
            size: order.size,
            action: order.action,
            reserve_price: order.reserve_price,
            order_type: OrderType::Gtc,
            ..Default::default()
        };

        let filled = self.try_match(&mut temp_cmd);
        order.filled = filled_before + filled;
        cmd.matcher_events.extend(temp_cmd.matcher_events);

        // 如果未完全成交，重新挂单
        if order.filled < order.size {
            match action {
                OrderAction::Ask => {
                    self.ask_buckets
                        .entry(cmd.price)
                        .or_insert_with(|| OrdersBucket::new(cmd.price))
                        .add(order);
                }
                OrderAction::Bid => {
                    self.bid_buckets
                        .entry(cmd.price)
                        .or_insert_with(|| OrdersBucket::new(cmd.price))
                        .add(order);
                }
            }
            self.update_best_prices();
        } else {
            self.order_map.remove(&cmd.order_id);
        }

        CommandResultCode::Success
    }

    fn reduce_order(&mut self, cmd: &mut OrderCommand) -> CommandResultCode {
        let Some((price, action)) = self.order_map.get(&cmd.order_id).copied() else {
            return CommandResultCode::MatchingUnknownOrderId;
        };

        let buckets = match action {
            OrderAction::Ask => &mut self.ask_buckets,
            OrderAction::Bid => &mut self.bid_buckets,
        };

        if let Some(bucket) = buckets.get_mut(&price) {
            if let Some(order) = bucket.orders.iter_mut().find(|o| o.order_id == cmd.order_id) {
                let remaining = order.remaining();
                let reduce_by = remaining.min(cmd.size);

                if reduce_by == remaining {
                    // 完全移除
                    let _order = bucket.remove(cmd.order_id).unwrap();
                    cmd.matcher_events.push(MatcherTradeEvent::new_reject(reduce_by, price));
                    cmd.action = action;
                    self.order_map.remove(&cmd.order_id);

                    if bucket.total_volume == 0 {
                        buckets.remove(&price);
                        self.update_best_prices();
                    }
                } else {
                    // 部分减少
                    order.size -= reduce_by;
                    bucket.total_volume -= reduce_by;
                    cmd.matcher_events.push(MatcherTradeEvent::new_reject(reduce_by, price));
                    cmd.action = action;
                }

                return CommandResultCode::Success;
            }
        }

        CommandResultCode::MatchingUnknownOrderId
    }

    fn get_symbol_spec(&self) -> &CoreSymbolSpecification {
        &self.symbol_spec
    }

    fn get_l2_data(&self, depth: usize) -> L2MarketData {
        let mut data = L2MarketData::new(depth);

        // 卖单（从低到高）
        for (price, bucket) in self.ask_buckets.iter().take(depth) {
            data.ask_prices.push(*price);
            data.ask_volumes.push(bucket.total_volume);
        }

        // 买单（从高到低）
        for (price, bucket) in self.bid_buckets.iter().take(depth) {
            data.bid_prices.push(*price);
            data.bid_volumes.push(bucket.total_volume);
        }

        data
    }

    fn get_order_by_id(&self, order_id: OrderId) -> Option<(Price, OrderAction)> {
        self.order_map.get(&order_id).copied()
    }

    fn get_total_ask_volume(&self) -> Size {
        self.ask_buckets.values().map(|b| b.total_volume).sum()
    }

    fn get_total_bid_volume(&self) -> Size {
        self.bid_buckets.values().map(|b| b.total_volume).sum()
    }

    fn get_ask_buckets_count(&self) -> usize {
        self.ask_buckets.len()
    }

    fn get_bid_buckets_count(&self) -> usize {
        self.bid_buckets.len()
    }

    fn serialize_state(&self) -> crate::core::orderbook::OrderBookState {
        crate::core::orderbook::OrderBookState::Naive(self.clone())
    }
}
