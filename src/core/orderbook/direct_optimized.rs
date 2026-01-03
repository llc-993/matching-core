use crate::api::*;
use crate::core::orderbook::simd_utils::*;
use ahash::AHashMap;
use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

type OrderIdx = usize;

/// SOA 内存布局：订单热数据（缓存友好）
#[derive(Clone, Serialize, Deserialize)]
struct OrderHotData {
    order_ids: Vec<OrderId>,    // 订单 ID
    prices: Vec<Price>,         // 价格
    sizes: Vec<Size>,           // 数量
    filled: Vec<Size>,          // 已成交
    next: Vec<Option<OrderIdx>>, // 链表后继
    prev: Vec<Option<OrderIdx>>, // 链表前驱
    active: Vec<bool>,          // 激活标记
}

/// 订单冷数据（低频访问）
#[derive(Clone, Serialize, Deserialize)]
struct OrderColdData {
    uid: UserId,
    action: OrderAction,
    reserve_price: Price,
    timestamp: i64,
}

/// 预分配订单池（零分配）
#[derive(Clone, Serialize, Deserialize)]
struct OrderPool {
    hot: OrderHotData,
    cold: Vec<OrderColdData>,
    free_list: Vec<OrderIdx>,
    capacity: usize,
}

impl OrderPool {
    fn new(capacity: usize) -> Self {
        let mut free_list = Vec::with_capacity(capacity);
        for i in (0..capacity).rev() {
            free_list.push(i);
        }
        
        Self {
            hot: OrderHotData {
                order_ids: vec![0; capacity],
                prices: vec![0; capacity],
                sizes: vec![0; capacity],
                filled: vec![0; capacity],
                next: vec![None; capacity],
                prev: vec![None; capacity],
                active: vec![false; capacity],
            },
            cold: vec![
                OrderColdData {
                    uid: 0,
                    action: OrderAction::Bid,
                    reserve_price: 0,
                    timestamp: 0,
                };
                capacity
            ],
            free_list,
            capacity,
        }
    }

    #[inline]
    fn alloc(&mut self) -> Option<OrderIdx> {
        self.free_list.pop()
    }

    #[inline]
    fn dealloc(&mut self, idx: OrderIdx) {
        self.hot.active[idx] = false;
        self.free_list.push(idx);
    }
}

/// 价格桶（简化版）
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PriceBucket {
    price: Price,
    volume: Size,
    head: OrderIdx, // 链表头（最早订单）
}

/// 高性能撮合引擎（深度优化版）
#[derive(Clone, Serialize, Deserialize)]
pub struct DirectOrderBookOptimized {
    symbol_spec: CoreSymbolSpecification,
    
    // SOA 订单池（预分配）
    order_pool: OrderPool,
    
    // 价格索引（ART 可替换 BTreeMap）
    ask_buckets: BTreeMap<Price, PriceBucket>,
    bid_buckets: BTreeMap<Price, PriceBucket>,
    
    // SIMD 优化开关
    #[serde(skip)]
    use_simd: bool,
    
    // 订单 ID 索引
    order_index: AHashMap<OrderId, OrderIdx>,
    
    // 最优价格缓存
    best_ask: Option<Price>,
    best_bid: Option<Price>,
}

impl DirectOrderBookOptimized {
    pub fn new(spec: CoreSymbolSpecification) -> Self {
        Self {
            symbol_spec: spec,
            order_pool: OrderPool::new(100_000), // 预分配 10 万订单
            ask_buckets: BTreeMap::new(),
            bid_buckets: BTreeMap::new(),
            order_index: AHashMap::with_capacity(100_000),
            best_ask: None,
            best_bid: None,
            use_simd: true, // 默认启用 SIMD
        }
    }
    
    /// 设置 SIMD 优化开关
    pub fn set_simd_enabled(&mut self, enabled: bool) {
        self.use_simd = enabled;
    }

    /// GTC 下单
    fn place_gtc(&mut self, cmd: &mut OrderCommand) {
        if self.order_index.contains_key(&cmd.order_id) {
            let filled = if self.use_simd {
                self.try_match_simd_batch(cmd)
            } else {
                self.try_match(cmd)
            };
            if filled < cmd.size {
                cmd.matcher_events.push(MatcherTradeEvent::new_reject(cmd.size - filled, cmd.price));
            }
            return;
        }

        let filled = if self.use_simd {
            self.try_match_simd_batch(cmd)
        } else {
            self.try_match(cmd)
        };

        if filled < cmd.size {
            if let Some(idx) = self.order_pool.alloc() {
                // 写入热数据
                self.order_pool.hot.order_ids[idx] = cmd.order_id;
                self.order_pool.hot.prices[idx] = cmd.price;
                self.order_pool.hot.sizes[idx] = cmd.size;
                self.order_pool.hot.filled[idx] = filled;
                self.order_pool.hot.active[idx] = true;
                
                // 写入冷数据
                self.order_pool.cold[idx] = OrderColdData {
                    uid: cmd.uid,
                    action: cmd.action,
                    reserve_price: cmd.reserve_price,
                    timestamp: cmd.timestamp,
                };

                self.order_index.insert(cmd.order_id, idx);
                self.insert_to_bucket(idx, cmd.price, cmd.action);
            }
        }
    }

    /// IOC 下单
    fn place_ioc(&mut self, cmd: &mut OrderCommand) {
        let filled = if self.use_simd {
            self.try_match_simd_batch(cmd)
        } else {
            self.try_match(cmd)
        };
        if filled < cmd.size {
            cmd.matcher_events.push(MatcherTradeEvent::new_reject(cmd.size - filled, cmd.price));
        }
    }

    /// SIMD 批量撮合（优化版）
    #[cfg(target_arch = "aarch64")]
    fn try_match(&mut self, cmd: &mut OrderCommand) -> Size {
        let is_bid = cmd.action == OrderAction::Bid;
        let limit_price = cmd.price;
        let mut filled = 0;

        // 快速路径：检查最优价格
        let best_price = if is_bid { self.best_ask } else { self.best_bid };
        if let Some(best) = best_price {
            if (is_bid && best > limit_price) || (!is_bid && best < limit_price) {
                return 0;
            }
        } else {
            return 0;
        }

        let prices_to_match: Vec<Price> = if is_bid {
            self.ask_buckets.range(..=limit_price).map(|(p, _)| *p).collect()
        } else {
            self.bid_buckets.range(limit_price..).rev().map(|(p, _)| *p).collect()
        };

        let mut need_update_best = false;

        for price in prices_to_match {
            if filled >= cmd.size {
                break;
            }

            let buckets = if is_bid { &mut self.ask_buckets } else { &mut self.bid_buckets };
            
            if let Some(bucket) = buckets.get_mut(&price) {
                let mut current_idx = bucket.head;
                
                while filled < cmd.size && self.order_pool.hot.active[current_idx] {
                    let remaining = cmd.size - filled;
                    let order_remaining = self.order_pool.hot.sizes[current_idx] - self.order_pool.hot.filled[current_idx];
                    let trade_size = remaining.min(order_remaining);

                    // 更新成交
                    self.order_pool.hot.filled[current_idx] += trade_size;
                    bucket.volume -= trade_size;
                    filled += trade_size;

                    // 生成事件
                    let maker_uid = self.order_pool.cold[current_idx].uid;
                    let reserve = if is_bid {
                        cmd.reserve_price
                    } else {
                        self.order_pool.cold[current_idx].reserve_price
                    };
                    
                    cmd.matcher_events.push(MatcherTradeEvent::new_trade(
                        trade_size,
                        price,
                        self.order_pool.hot.order_ids[current_idx],
                        maker_uid,
                        reserve,
                    ));

                    // 订单完成
                    if self.order_pool.hot.filled[current_idx] >= self.order_pool.hot.sizes[current_idx] {
                        let order_id = self.order_pool.hot.order_ids[current_idx];
                        self.order_index.remove(&order_id);
                        self.order_pool.dealloc(current_idx);
                    }

                    if let Some(next) = self.order_pool.hot.next[current_idx] {
                        current_idx = next;
                    } else {
                        break;
                    }
                }

                if bucket.volume == 0 {
                    buckets.remove(&price);
                    need_update_best = true;
                }
            }
        }

        if need_update_best {
            self.update_best_price(is_bid);
        }

        filled
    }

    /// 非 ARM 架构回退
    #[cfg(not(target_arch = "aarch64"))]
    fn try_match(&mut self, cmd: &mut OrderCommand) -> Size {
        let is_bid = cmd.action == OrderAction::Bid;
        let limit_price = cmd.price;
        let mut filled = 0;

        let best_price = if is_bid { self.best_ask } else { self.best_bid };
        if let Some(best) = best_price {
            if (is_bid && best > limit_price) || (!is_bid && best < limit_price) {
                return 0;
            }
        } else {
            return 0;
        }

        let prices_to_match: Vec<Price> = if is_bid {
            self.ask_buckets.range(..=limit_price).map(|(p, _)| *p).collect()
        } else {
            self.bid_buckets.range(limit_price..).rev().map(|(p, _)| *p).collect()
        };

        let mut need_update_best = false;

        for price in prices_to_match {
            if filled >= cmd.size {
                break;
            }

            let buckets = if is_bid { &mut self.ask_buckets } else { &mut self.bid_buckets };
            
            if let Some(bucket) = buckets.get_mut(&price) {
                let mut current_idx = bucket.head;
                
                while filled < cmd.size && self.order_pool.hot.active[current_idx] {
                    let remaining = cmd.size - filled;
                    let order_remaining = self.order_pool.hot.sizes[current_idx] - self.order_pool.hot.filled[current_idx];
                    let trade_size = remaining.min(order_remaining);

                    self.order_pool.hot.filled[current_idx] += trade_size;
                    bucket.volume -= trade_size;
                    filled += trade_size;

                    let maker_uid = self.order_pool.cold[current_idx].uid;
                    let reserve = if is_bid {
                        cmd.reserve_price
                    } else {
                        self.order_pool.cold[current_idx].reserve_price
                    };
                    
                    cmd.matcher_events.push(MatcherTradeEvent::new_trade(
                        trade_size,
                        price,
                        self.order_pool.hot.order_ids[current_idx],
                        maker_uid,
                        reserve,
                    ));

                    if self.order_pool.hot.filled[current_idx] >= self.order_pool.hot.sizes[current_idx] {
                        let order_id = self.order_pool.hot.order_ids[current_idx];
                        self.order_index.remove(&order_id);
                        self.order_pool.dealloc(current_idx);
                    }

                    if let Some(next) = self.order_pool.hot.next[current_idx] {
                        current_idx = next;
                    } else {
                        break;
                    }
                }

                if bucket.volume == 0 {
                    buckets.remove(&price);
                    need_update_best = true;
                }
            }
        }

        if need_update_best {
            self.update_best_price(is_bid);
        }

        filled
    }

    /// SIMD 批量撮合优化（高性能版本）
    fn try_match_simd_batch(&mut self, cmd: &mut OrderCommand) -> Size {
        let is_bid = cmd.action == OrderAction::Bid;
        let limit_price = cmd.price;
        let mut filled = 0;

        // 快速路径：检查最优价格
        let best_price = if is_bid { self.best_ask } else { self.best_bid };
        if let Some(best) = best_price {
            if (is_bid && best > limit_price) || (!is_bid && best < limit_price) {
                return 0;
            }
        } else {
            return 0;
        }

        // 收集价格档位
        let prices_to_match: Vec<Price> = if is_bid {
            self.ask_buckets.range(..=limit_price).map(|(p, _)| *p).collect()
        } else {
            self.bid_buckets.range(limit_price..).rev().map(|(p, _)| *p).collect()
        };

        let mut need_update_best = false;
        let mut prices_to_remove = Vec::new();

        for price in prices_to_match {
            if filled >= cmd.size {
                break;
            }

            // 收集该价格档的所有活跃订单
            let mut order_indices = Vec::new();
            {
                let buckets = if is_bid { &self.ask_buckets } else { &self.bid_buckets };
                if let Some(bucket) = buckets.get(&price) {
                    let mut current_idx = bucket.head;
                    
                    while self.order_pool.hot.active[current_idx] {
                        order_indices.push(current_idx);
                        if let Some(next) = self.order_pool.hot.next[current_idx] {
                            current_idx = next;
                        } else {
                            break;
                        }
                    }
                }
            }

            if order_indices.is_empty() {
                continue;
            }

            // SIMD 批量处理（如果订单数量 >= 4）
            if order_indices.len() >= 4 {
                let matched = self.simd_match_orders_internal(
                    &order_indices,
                    cmd.size - filled,
                    price,
                    cmd.action,
                    cmd.reserve_price,
                    &mut cmd.matcher_events,
                );
                filled += matched;
            } else {
                // 少量订单使用标准处理
                for &idx in &order_indices {
                    if filled >= cmd.size {
                        break;
                    }
                    
                    let order_remaining = self.order_pool.hot.sizes[idx] - self.order_pool.hot.filled[idx];
                    let trade_size = (cmd.size - filled).min(order_remaining);

                    self.order_pool.hot.filled[idx] += trade_size;
                    filled += trade_size;

                    let maker_uid = self.order_pool.cold[idx].uid;
                    let reserve = if is_bid {
                        cmd.reserve_price
                    } else {
                        self.order_pool.cold[idx].reserve_price
                    };
                    
                    cmd.matcher_events.push(MatcherTradeEvent::new_trade(
                        trade_size,
                        price,
                        self.order_pool.hot.order_ids[idx],
                        maker_uid,
                        reserve,
                    ));

                    if self.order_pool.hot.filled[idx] >= self.order_pool.hot.sizes[idx] {
                        let order_id = self.order_pool.hot.order_ids[idx];
                        self.order_index.remove(&order_id);
                        self.order_pool.dealloc(idx);
                    }
                }
            }

            // 更新桶信息
            {
                let buckets = if is_bid { &mut self.ask_buckets } else { &mut self.bid_buckets };
                if let Some(bucket) = buckets.get_mut(&price) {
                    // 重新计算桶的总量
                    let mut new_volume = 0;
                    for &idx in &order_indices {
                        if self.order_pool.hot.active[idx] {
                            new_volume += self.order_pool.hot.sizes[idx] - self.order_pool.hot.filled[idx];
                        }
                    }
                    bucket.volume = new_volume;
                    
                    if bucket.volume == 0 {
                        prices_to_remove.push(price);
                        need_update_best = true;
                    }
                }
            }
        }

        // 清理空桶
        for price in prices_to_remove {
            if is_bid {
                self.ask_buckets.remove(&price);
            } else {
                self.bid_buckets.remove(&price);
            }
        }

        if need_update_best {
            self.update_best_price(is_bid);
        }

        filled
    }

    /// SIMD 批量处理订单
    #[inline]
    fn simd_match_orders_internal(
        &mut self,
        order_indices: &[OrderIdx],
        need_size: Size,
        price: Price,
        taker_action: OrderAction,
        taker_reserve: Price,
        events: &mut Vec<MatcherTradeEvent>,
    ) -> Size {
        // 收集订单数据（SOA 优势）
        let sizes: Vec<i64> = order_indices.iter()
            .map(|&idx| self.order_pool.hot.sizes[idx])
            .collect();
        
        let filled: Vec<i64> = order_indices.iter()
            .map(|&idx| self.order_pool.hot.filled[idx])
            .collect();

        // SIMD 批量计算匹配量
        let (matched_sizes, _total_matched) = simd_batch_match_prepare(&sizes, &filled, need_size);

        // 应用匹配结果
        let mut actual_filled = 0i64;
        for (i, &idx) in order_indices.iter().enumerate() {
            let match_size = matched_sizes[i];
            if match_size > 0 {
                self.order_pool.hot.filled[idx] += match_size;
                actual_filled += match_size;

                let maker_uid = self.order_pool.cold[idx].uid;
                let reserve = if taker_action == OrderAction::Bid {
                    taker_reserve
                } else {
                    self.order_pool.cold[idx].reserve_price
                };

                events.push(MatcherTradeEvent::new_trade(
                    match_size,
                    price,
                    self.order_pool.hot.order_ids[idx],
                    maker_uid,
                    reserve,
                ));
            }
        }

        actual_filled
    }

    /// 插入订单到价格桶
    fn insert_to_bucket(&mut self, order_idx: OrderIdx, price: Price, action: OrderAction) {
        let size = self.order_pool.hot.sizes[order_idx] - self.order_pool.hot.filled[order_idx];
        let is_ask = action == OrderAction::Ask;

        let buckets = if is_ask {
            &mut self.ask_buckets
        } else {
            &mut self.bid_buckets
        };

        let is_new = !buckets.contains_key(&price);

        buckets
            .entry(price)
            .and_modify(|bucket| {
                bucket.volume += size;
                let old_head = bucket.head;
                self.order_pool.hot.next[order_idx] = Some(old_head);
                self.order_pool.hot.prev[old_head] = Some(order_idx);
                bucket.head = order_idx;
            })
            .or_insert_with(|| {
                PriceBucket {
                    price,
                    volume: size,
                    head: order_idx,
                }
            });

        if is_new {
            self.update_best_price(is_ask);
        }
    }

    /// 更新最优价格缓存
    fn update_best_price(&mut self, is_ask: bool) {
        if is_ask {
            self.best_ask = self.ask_buckets.keys().next().copied();
        } else {
            self.best_bid = self.bid_buckets.keys().next_back().copied();
        }
    }

    /// 取消订单
    fn cancel_order(&mut self, cmd: &mut OrderCommand) -> CommandResultCode {
        if let Some(&order_idx) = self.order_index.get(&cmd.order_id) {
            let price = self.order_pool.hot.prices[order_idx];
            let action = self.order_pool.cold[order_idx].action;
            let remaining = self.order_pool.hot.sizes[order_idx] - self.order_pool.hot.filled[order_idx];

            cmd.matcher_events.push(MatcherTradeEvent::new_reject(remaining, price));
            cmd.action = action;

            self.order_index.remove(&cmd.order_id);
            self.order_pool.dealloc(order_idx);

            CommandResultCode::Success
        } else {
            CommandResultCode::MatchingUnknownOrderId
        }
    }
}

impl super::OrderBook for DirectOrderBookOptimized {
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
            _ => CommandResultCode::MatchingUnsupportedCommand,
        }
    }

    fn cancel_order(&mut self, cmd: &mut OrderCommand) -> CommandResultCode {
        self.cancel_order(cmd)
    }

    fn move_order(&mut self, _cmd: &mut OrderCommand) -> CommandResultCode {
        CommandResultCode::MatchingUnsupportedCommand // 简化实现
    }

    fn reduce_order(&mut self, _cmd: &mut OrderCommand) -> CommandResultCode {
        CommandResultCode::MatchingUnsupportedCommand // 简化实现
    }

    fn get_symbol_spec(&self) -> &CoreSymbolSpecification {
        &self.symbol_spec
    }

    fn get_l2_data(&self, depth: usize) -> L2MarketData {
        let mut data = L2MarketData::new(depth);

        for (price, bucket) in self.ask_buckets.iter().take(depth) {
            data.ask_prices.push(*price);
            data.ask_volumes.push(bucket.volume);
        }

        for (price, bucket) in self.bid_buckets.iter().rev().take(depth) {
            data.bid_prices.push(*price);
            data.bid_volumes.push(bucket.volume);
        }

        data
    }

    fn get_order_by_id(&self, order_id: OrderId) -> Option<(Price, OrderAction)> {
        self.order_index.get(&order_id).map(|&idx| {
            let price = self.order_pool.hot.prices[idx];
            let action = self.order_pool.cold[idx].action;
            (price, action)
        })
    }

    fn get_total_ask_volume(&self) -> Size {
        self.ask_buckets.values().map(|b| b.volume).sum()
    }

    fn get_total_bid_volume(&self) -> Size {
        self.bid_buckets.values().map(|b| b.volume).sum()
    }

    fn get_ask_buckets_count(&self) -> usize {
        self.ask_buckets.len()
    }

    fn get_bid_buckets_count(&self) -> usize {
        self.bid_buckets.len()
    }

    fn serialize_state(&self) -> crate::core::orderbook::OrderBookState {
        // 简化：暂不支持序列化优化版本
        crate::core::orderbook::OrderBookState::Direct(
            crate::core::orderbook::DirectOrderBook::new(self.symbol_spec.clone())
        )
    }
}

