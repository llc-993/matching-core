use crate::api::*;
use ahash::AHashMap;
use slab::Slab;
use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

type OrderIdx = usize;
type BucketIdx = usize;

/// 直接订单（使用 Slab 索引实现的双向链表，避免 Rc/RefCell 开销）
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DirectOrder {
    order_id: OrderId,
    uid: UserId,
    price: Price,
    size: Size,
    filled: Size,
    action: OrderAction,
    reserve_price: Price,
    timestamp: i64,
    next: Option<OrderIdx>,
    prev: Option<OrderIdx>,
    parent: BucketIdx,
}

/// 价格档位（桶），存储相同价格的一组订单
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Bucket {
    price: Price,
    volume: Size,
    num_orders: usize,
    tail: OrderIdx,
}

/// 高性能撮合引擎实现 (Direct 实现)
/// 逻辑参考 exchange-core，使用原始指针/索引链表实现极低延迟
#[derive(Clone, Serialize, Deserialize)]
pub struct DirectOrderBook {
    symbol_spec: CoreSymbolSpecification, // 交易对配置
    
    // 内存池，预分配订单和桶，减少分配开销
    orders: Slab<DirectOrder>,
    buckets: Slab<Bucket>,
    
    // 价格索引，快速定位最优价格
    ask_price_buckets: BTreeMap<Price, BucketIdx>, // 卖单：价格升序
    bid_price_buckets: BTreeMap<Price, BucketIdx>, // 买单：价格降序
    
    // 订单 ID 快速索引
    order_id_index: AHashMap<OrderId, OrderIdx>,
    
    // 最优订单快捷引用，类似 LMAX Disruptor 的快速路径
    best_ask_order: Option<OrderIdx>, // 卖一订单
    best_bid_order: Option<OrderIdx>, // 买一订单
}

impl DirectOrderBook {
    pub fn new(spec: CoreSymbolSpecification) -> Self {
        Self {
            symbol_spec: spec,
            orders: Slab::with_capacity(1024),
            buckets: Slab::with_capacity(128),
            ask_price_buckets: BTreeMap::new(),
            bid_price_buckets: BTreeMap::new(),
            order_id_index: AHashMap::new(),
            best_ask_order: None,
            best_bid_order: None,
        }
    }

    /// GTC 下单
    fn place_gtc(&mut self, cmd: &mut OrderCommand) {
        // 检查重复订单
        if self.order_id_index.contains_key(&cmd.order_id) {
            let filled = self.try_match(cmd);
            if filled < cmd.size {
                cmd.matcher_events.push(MatcherTradeEvent::new_reject(cmd.size - filled, cmd.price));
            }
            return;
        }

        // 尝试撮合
        let filled = self.try_match(cmd);

        // 未完全成交，挂单
        if filled < cmd.size {
            let order_idx = self.orders.insert(DirectOrder {
                order_id: cmd.order_id,
                uid: cmd.uid,
                price: cmd.price,
                size: cmd.size,
                filled,
                action: cmd.action,
                reserve_price: cmd.reserve_price,
                timestamp: cmd.timestamp,
                next: None,
                prev: None,
                parent: 0, // 临时值
            });

            self.order_id_index.insert(cmd.order_id, order_idx);
            self.insert_order(order_idx);
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

    /// FOK_BUDGET 下单
    fn place_fok_budget(&mut self, cmd: &mut OrderCommand) {
        let budget = self.check_budget_to_fill(cmd.size, cmd.action);

        if let Some(calculated) = budget {
            if self.is_budget_satisfied(cmd.action, calculated, cmd.price) {
                self.try_match(cmd);
            } else {
                cmd.matcher_events.push(MatcherTradeEvent::new_reject(cmd.size, cmd.price));
            }
        } else {
            cmd.matcher_events.push(MatcherTradeEvent::new_reject(cmd.size, cmd.price));
        }
    }

    fn is_budget_satisfied(&self, action: OrderAction, calculated: i64, limit: i64) -> bool {
        calculated != i64::MAX && (calculated == limit || (action == OrderAction::Bid) != (calculated > limit))
    }

    fn check_budget_to_fill(&self, mut size: Size, action: OrderAction) -> Option<i64> {
        let mut maker_idx = match action {
            OrderAction::Bid => self.best_ask_order,
            OrderAction::Ask => self.best_bid_order,
        };

        let mut budget: i64 = 0;

        while let Some(idx) = maker_idx {
            let order = &self.orders[idx];
            let bucket = &self.buckets[order.parent];
            let available = bucket.volume;
            let price = order.price;

            if size > available {
                size -= available;
                budget += (available * price) as i64;
                // 移动到桶尾部订单的前一个订单
                let tail_idx = bucket.tail;
                maker_idx = self.orders[tail_idx].prev;
            } else {
                return Some(budget + (size * price) as i64);
            }
        }

        None
    }

    /// 尝试撮合
    fn try_match(&mut self, cmd: &mut OrderCommand) -> Size {
        let is_bid = cmd.action == OrderAction::Bid;
        let limit_price = cmd.price;

        let mut maker_idx = if is_bid {
            self.best_ask_order
        } else {
            self.best_bid_order
        };

        // 检查是否有可撮合订单
        if let Some(idx) = maker_idx {
            let maker_price = self.orders[idx].price;
            if is_bid && maker_price > limit_price {
                return 0;
            }
            if !is_bid && maker_price < limit_price {
                return 0;
            }
        } else {
            return 0;
        }

        let mut filled = 0;
        let taker_size = cmd.size;
        let taker_reserve = cmd.reserve_price;

        while let Some(idx) = maker_idx {
            let remaining = taker_size - filled;
            if remaining == 0 {
                break;
            }

            let (maker_price, maker_filled, maker_size, maker_parent, maker_prev) = {
                let order = &self.orders[idx];
                if is_bid && order.price > limit_price {
                    break;
                }
                if !is_bid && order.price < limit_price {
                    break;
                }
                (order.price, order.filled, order.size, order.parent, order.prev)
            };

            let trade_size = remaining.min(maker_size - maker_filled);

            // 更新 maker 订单
            {
                let order = &mut self.orders[idx];
                order.filled += trade_size;
            }

            // 更新桶
            self.buckets[maker_parent].volume -= trade_size;
            filled += trade_size;

            let maker_completed = maker_filled + trade_size == maker_size;
            if maker_completed {
                self.buckets[maker_parent].num_orders -= 1;
            }

            // 生成事件
            let event = MatcherTradeEvent::new_trade(
                trade_size,
                maker_price,
                self.orders[idx].order_id,
                self.orders[idx].uid,
                if is_bid { taker_reserve } else { self.orders[idx].reserve_price },
            );
            cmd.matcher_events.push(event);

            if !maker_completed {
                break;
            }

            // 移除完成的 maker 订单
            let bucket_tail = self.buckets[maker_parent].tail;
            let should_remove_bucket = idx == bucket_tail;

            let next_maker = maker_prev;
            self.order_id_index.remove(&self.orders[idx].order_id);
            self.orders.remove(idx);

            if should_remove_bucket {
                let price = maker_price;
                if is_bid {
                    self.ask_price_buckets.remove(&price);
                } else {
                    self.bid_price_buckets.remove(&price);
                }
                self.buckets.remove(maker_parent);
            }

            maker_idx = next_maker;
        }

        // 更新最优订单
        if is_bid {
            self.best_ask_order = maker_idx;
        } else {
            self.best_bid_order = maker_idx;
        }

        filled
    }

    /// 插入订单到链表
    fn insert_order(&mut self, order_idx: OrderIdx) {
        let (price, action) = {
            let order = &self.orders[order_idx];
            (order.price, order.action)
        };

        let is_ask = action == OrderAction::Ask;
        let buckets_map = if is_ask { &mut self.ask_price_buckets } else { &mut self.bid_price_buckets };

        if let Some(&bucket_idx) = buckets_map.get(&price) {
            // 桶已存在，添加到尾部
            let old_tail = self.buckets[bucket_idx].tail;
            let prev_order = self.orders[old_tail].prev;

            self.buckets[bucket_idx].tail = order_idx;
            self.buckets[bucket_idx].volume += self.orders[order_idx].size - self.orders[order_idx].filled;
            self.buckets[bucket_idx].num_orders += 1;

            self.orders[old_tail].prev = Some(order_idx);
            if let Some(prev_idx) = prev_order {
                self.orders[prev_idx].next = Some(order_idx);
            }

            self.orders[order_idx].next = Some(old_tail);
            self.orders[order_idx].prev = prev_order;
            self.orders[order_idx].parent = bucket_idx;
        } else {
            // 创建新桶
            let bucket_idx = self.buckets.insert(Bucket {
                price,
                volume: self.orders[order_idx].size - self.orders[order_idx].filled,
                num_orders: 1,
                tail: order_idx,
            });

            buckets_map.insert(price, bucket_idx);
            self.orders[order_idx].parent = bucket_idx;

            // 链接到链表
            let lower_bucket_idx = if is_ask {
                buckets_map.range(..price).next_back().map(|(_, &idx)| idx)
            } else {
                buckets_map.range((price + 1)..).next().map(|(_, &idx)| idx)
            };

            if let Some(lower_idx) = lower_bucket_idx {
                let lower_tail = self.buckets[lower_idx].tail;
                let prev_order = self.orders[lower_tail].prev;

                self.orders[lower_tail].prev = Some(order_idx);
                if let Some(prev_idx) = prev_order {
                    self.orders[prev_idx].next = Some(order_idx);
                }

                self.orders[order_idx].next = Some(lower_tail);
                self.orders[order_idx].prev = prev_order;
            } else {
                // 更新最优订单
                let old_best = if is_ask { self.best_ask_order } else { self.best_bid_order };
                if let Some(old_idx) = old_best {
                    self.orders[old_idx].next = Some(order_idx);
                }

                if is_ask {
                    self.best_ask_order = Some(order_idx);
                } else {
                    self.best_bid_order = Some(order_idx);
                }

                self.orders[order_idx].next = None;
                self.orders[order_idx].prev = old_best;
            }
        }
    }
    /// 移除订单
    fn remove_order(&mut self, order_idx: OrderIdx) -> bool {
        let (bucket_idx, remaining, next, prev, price, action) = {
            let order = &self.orders[order_idx];
            (
                order.parent,
                order.size - order.filled,
                order.next,
                order.prev,
                order.price,
                order.action,
            )
        };

        // 更新桶
        self.buckets[bucket_idx].volume -= remaining;
        self.buckets[bucket_idx].num_orders -= 1;

        let mut should_remove_bucket = false;

        // 如果是尾部订单
        if self.buckets[bucket_idx].tail == order_idx {
            if let Some(next_idx) = next {
                if self.orders[next_idx].parent != bucket_idx {
                    should_remove_bucket = true;
                } else {
                    self.buckets[bucket_idx].tail = next_idx;
                }
            } else {
                should_remove_bucket = true;
            }
        }

        // 更新邻居订单
        if let Some(next_idx) = next {
            self.orders[next_idx].prev = prev;
        }
        if let Some(prev_idx) = prev {
            self.orders[prev_idx].next = next;
        }

        // 更新最优订单
        if Some(order_idx) == self.best_ask_order {
            self.best_ask_order = prev;
        } else if Some(order_idx) == self.best_bid_order {
            self.best_bid_order = prev;
        }

        // 移除桶
        if should_remove_bucket {
            if action == OrderAction::Ask {
                self.ask_price_buckets.remove(&price);
            } else {
                self.bid_price_buckets.remove(&price);
            }
            self.buckets.remove(bucket_idx);
        }

        should_remove_bucket
    }
}

impl super::OrderBook for DirectOrderBook {
    fn new_order(&mut self, cmd: &mut OrderCommand) -> CommandResultCode {
        match cmd.order_type {
            OrderType::Gtc => {
                self.place_gtc(cmd);
                CommandResultCode::Success
            }
            OrderType::Ioc => {
                self.place_ioc(cmd);
                CommandResultCode::Success
            }
            OrderType::FokBudget => {
                self.place_fok_budget(cmd);
                CommandResultCode::Success
            }
            _ => {
                return CommandResultCode::MatchingUnsupportedCommand;
            }
        }
    }

    fn cancel_order(&mut self, cmd: &mut OrderCommand) -> CommandResultCode {
        let Some(&order_idx) = self.order_id_index.get(&cmd.order_id) else {
            return CommandResultCode::MatchingUnknownOrderId;
        };

        let (action, remaining, price) = {
            let order = &self.orders[order_idx];
            if order.uid != cmd.uid {
                return CommandResultCode::MatchingUnknownOrderId;
            }
            (order.action, order.size - order.filled, order.price)
        };

        self.order_id_index.remove(&cmd.order_id);
        self.remove_order(order_idx);
        self.orders.remove(order_idx);

        cmd.action = action;
        cmd.matcher_events.push(MatcherTradeEvent::new_reject(remaining, price));

        CommandResultCode::Success
    }

    fn move_order(&mut self, cmd: &mut OrderCommand) -> CommandResultCode {
        let Some(&order_idx) = self.order_id_index.get(&cmd.order_id) else {
            return CommandResultCode::MatchingUnknownOrderId;
        };

        let (uid, action, reserve_price, size) = {
            let order = &self.orders[order_idx];
            if order.uid != cmd.uid {
                return CommandResultCode::MatchingUnknownOrderId;
            }
            (order.uid, order.action, order.reserve_price, order.size)
        };

        // 风险检查
        if self.symbol_spec.symbol_type == SymbolType::CurrencyExchangePair
            && action == OrderAction::Bid
            && cmd.price > reserve_price
        {
            return CommandResultCode::RiskInvalidReserveBidPrice;
        }

        // 移除订单
        self.remove_order(order_idx);

        // 更新价格
        self.orders[order_idx].price = cmd.price;
        cmd.action = action;

        // 尝试撮合
        let mut temp_cmd = OrderCommand {
            uid,
            order_id: cmd.order_id,
            symbol: cmd.symbol,
            price: cmd.price,
            size,
            action,
            reserve_price,
            ..Default::default()
        };

        let filled = self.try_match(&mut temp_cmd);
        cmd.matcher_events.extend(temp_cmd.matcher_events);

        if filled == self.orders[order_idx].size {
            // 完全成交
            self.order_id_index.remove(&cmd.order_id);
            self.orders.remove(order_idx);
        } else {
            // 部分成交，重新挂单
            self.orders[order_idx].filled = filled;
            self.insert_order(order_idx);
        }

        CommandResultCode::Success
    }

    fn reduce_order(&mut self, cmd: &mut OrderCommand) -> CommandResultCode {
        let Some(&order_idx) = self.order_id_index.get(&cmd.order_id) else {
            return CommandResultCode::MatchingUnknownOrderId;
        };

        if cmd.size <= 0 {
            return CommandResultCode::MatchingInvalidOrderSize;
        }

        let (action, remaining, price, parent_idx) = {
            let order = &self.orders[order_idx];
            if order.uid != cmd.uid {
                return CommandResultCode::MatchingUnknownOrderId;
            }
            (order.action, order.size - order.filled, order.price, order.parent)
        };

        let reduce_by = remaining.min(cmd.size);
        let can_remove = reduce_by == remaining;

        if can_remove {
            self.order_id_index.remove(&cmd.order_id);
            self.remove_order(order_idx);
            self.orders.remove(order_idx);
        } else {
            let order = &mut self.orders[order_idx];
            order.size -= reduce_by;
            self.buckets[parent_idx].volume -= reduce_by;
        }

        cmd.action = action;
        cmd.matcher_events.push(MatcherTradeEvent::new_reject(reduce_by, price));

        CommandResultCode::Success
    }

    fn get_symbol_spec(&self) -> &CoreSymbolSpecification {
        &self.symbol_spec
    }

    fn get_l2_data(&self, depth: usize) -> L2MarketData {
        let mut data = L2MarketData::new(depth);

        for (price, &bucket_idx) in self.ask_price_buckets.iter().take(depth) {
            data.ask_prices.push(*price);
            data.ask_volumes.push(self.buckets[bucket_idx].volume);
        }

        for (price, &bucket_idx) in self.bid_price_buckets.iter().rev().take(depth) {
            data.bid_prices.push(*price);
            data.bid_volumes.push(self.buckets[bucket_idx].volume);
        }

        data
    }

    fn get_order_by_id(&self, order_id: OrderId) -> Option<(Price, OrderAction)> {
        self.order_id_index.get(&order_id).map(|&idx| {
            let order = &self.orders[idx];
            (order.price, order.action)
        })
    }

    fn get_total_ask_volume(&self) -> Size {
        self.ask_price_buckets.values().map(|&idx| self.buckets[idx].volume).sum()
    }

    fn get_total_bid_volume(&self) -> Size {
        self.bid_price_buckets.values().map(|&idx| self.buckets[idx].volume).sum()
    }

    fn get_ask_buckets_count(&self) -> usize {
        self.ask_price_buckets.len()
    }

    fn get_bid_buckets_count(&self) -> usize {
        self.bid_price_buckets.len()
    }

    fn serialize_state(&self) -> crate::core::orderbook::OrderBookState {
        crate::core::orderbook::OrderBookState::Direct(self.clone())
    }
}
