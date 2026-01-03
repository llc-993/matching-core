# Matching Core

[中文文档](./README.md) | English Documentation

High-performance matching engine core library built with Rust, supporting multiple order types and trading instruments.

## Features

### Core Functionality
- **High-Performance Matching Engine**: Supports millisecond-level order matching latency
- **Multiple Order Types**: GTC, IOC, FOK, Post-Only, Stop Order, Iceberg, GTD, Day
- **Multiple Trading Instruments**: Spot, Futures, Perpetual Contracts, Call Options, Put Options
- **Memory Optimization**: SOA memory layout, order pool pre-allocation, SmallVec to reduce heap allocations
- **Zero-Copy Serialization**: High-performance WAL using rkyv

### Technical Highlights
- **LMAX Disruptor Pattern**: Lock-free ring buffer for high throughput
- **Sharding Architecture**: Supports multiple risk engines and matching engine sharding
- **Persistence**: WAL logging and snapshot mechanism
- **SIMD Optimization**: Batch matching optimization
- **ART Index**: Adaptive Radix Tree for price indexing

## Quick Start

### Installation

```bash
git clone <repository-url>
cd matching-core
cargo build --release
```

### Basic Usage

```rust
use matching_core::api::*;
use matching_core::core::orderbook::{OrderBook, AdvancedOrderBook};

// Create trading pair configuration
let spec = CoreSymbolSpecification {
    symbol_id: 1,
    symbol_type: SymbolType::CurrencyExchangePair,
    base_currency: 0,
    quote_currency: 1,
    base_scale_k: 1,
    quote_scale_k: 1,
    taker_fee: 0,
    maker_fee: 0,
    margin_buy: 0,
    margin_sell: 0,
};

// Create order book
let mut book = AdvancedOrderBook::new(spec);

// Place sell order
let mut ask = OrderCommand {
    uid: 1,
    order_id: 1,
    symbol: 1,
    price: 10000,
    size: 100,
    action: OrderAction::Ask,
    order_type: OrderType::Gtc,
    reserve_price: 10000,
    timestamp: 1000,
    ..Default::default()
};
book.new_order(&mut ask);

// Buy order matching
let mut bid = OrderCommand {
    uid: 2,
    order_id: 2,
    symbol: 1,
    price: 10000,
    size: 50,
    action: OrderAction::Bid,
    order_type: OrderType::Ioc,
    reserve_price: 10000,
    timestamp: 1001,
    ..Default::default()
};
book.new_order(&mut bid);

// View trade events
for event in bid.matcher_events {
    println!("Trade: {} @ {}", event.size, event.price);
}
```

### Advanced Order Type Examples

#### Post-Only Order (Maker Only)

```rust
let mut post_only = OrderCommand {
    uid: 1,
    order_id: 1,
    symbol: 1,
    price: 9999,
    size: 10,
    action: OrderAction::Bid,
    order_type: OrderType::PostOnly,
    reserve_price: 9999,
    timestamp: 1000,
    ..Default::default()
};
book.new_order(&mut post_only);
```

#### Iceberg Order

```rust
let mut iceberg = OrderCommand {
    uid: 1,
    order_id: 1,
    symbol: 1,
    price: 10000,
    size: 1000,        // Total quantity
    action: OrderAction::Ask,
    order_type: OrderType::Iceberg,
    reserve_price: 10000,
    timestamp: 1000,
    visible_size: Some(100),  // Visible quantity
    ..Default::default()
};
book.new_order(&mut iceberg);
```

#### Stop Order

```rust
let mut stop = OrderCommand {
    uid: 1,
    order_id: 1,
    symbol: 1,
    price: 11000,      // Limit price
    size: 10,
    action: OrderAction::Bid,
    order_type: OrderType::StopLimit,
    reserve_price: 11000,
    timestamp: 1000,
    stop_price: Some(10500),  // Trigger price
    ..Default::default()
};
book.new_order(&mut stop);
```

#### GTD Order (Good-Till-Date)

```rust
let mut gtd = OrderCommand {
    uid: 1,
    order_id: 1,
    symbol: 1,
    price: 10000,
    size: 100,
    action: OrderAction::Ask,
    order_type: OrderType::Gtd(2000),
    reserve_price: 10000,
    timestamp: 1000,
    expire_time: Some(2000),  // Expiration timestamp
    ..Default::default()
};
book.new_order(&mut gtd);
```

## Complete Usage Flow

### Step 1: Create Symbol Specification

```rust
use matching_core::api::*;

// Define a trading pair (e.g., BTC/USDT)
let spec = CoreSymbolSpecification {
    symbol_id: 1,
    symbol_type: SymbolType::CurrencyExchangePair,
    base_currency: 0,      // BTC
    quote_currency: 1,      // USDT
    base_scale_k: 1,
    quote_scale_k: 1,
    taker_fee: 10,         // 0.1% taker fee (in basis points)
    maker_fee: 5,          // 0.05% maker fee (in basis points)
    margin_buy: 0,
    margin_sell: 0,
};
```

### Step 2: Initialize Order Book

```rust
use matching_core::core::orderbook::AdvancedOrderBook;

// Create order book with the specification
let mut orderbook = AdvancedOrderBook::new(spec);
```

### Step 3: Place Limit Order (Maker)

```rust
// Place a sell order (Maker)
let mut sell_order = OrderCommand {
    uid: 100,              // User ID
    order_id: 1,           // Order ID
    symbol: 1,            // Symbol ID
    price: 50000,          // Price: 50000 USDT
    size: 100,             // Size: 100 BTC
    action: OrderAction::Ask,
    order_type: OrderType::Gtc,  // Good-Till-Cancel
    reserve_price: 50000,
    timestamp: 1000,
    ..Default::default()
};

orderbook.new_order(&mut sell_order);

// Check if order was placed successfully
if sell_order.matcher_events.is_empty() {
    println!("Sell order placed successfully");
}
```

### Step 4: Place Market Order (Taker)

```rust
// Place a buy order (Taker) that will match with the sell order
let mut buy_order = OrderCommand {
    uid: 101,
    order_id: 2,
    symbol: 1,
    price: 50000,          // Same price as sell order
    size: 50,              // Buy 50 BTC (partial fill)
    action: OrderAction::Bid,
    order_type: OrderType::Ioc,  // Immediate-or-Cancel
    reserve_price: 50000,
    timestamp: 1001,
    ..Default::default()
};

orderbook.new_order(&mut buy_order);

// Process trade events
for event in &buy_order.matcher_events {
    match event.event_type {
        MatcherEventType::Trade => {
            println!(
                "Trade executed: {} @ {} (Maker: {}, Taker: {})",
                event.size,
                event.price,
                event.maker_order_id,
                event.taker_order_id
            );
        }
        MatcherEventType::Cancel => {
            println!("Order cancelled: {}", event.taker_order_id);
        }
        _ => {}
    }
}
```

### Step 5: Query Order Book State

```rust
// Get best bid and ask prices
let best_bid = orderbook.get_best_bid();
let best_ask = orderbook.get_best_ask();

println!("Best bid: {:?}", best_bid);
println!("Best ask: {:?}", best_ask);

// Get market depth
let depth = orderbook.get_market_depth(10);  // Top 10 levels
println!("Market depth: {:?}", depth);
```

### Step 6: Cancel Order

```rust
// Cancel an existing order
let mut cancel_cmd = OrderCommand {
    uid: 100,
    order_id: 1,           // Order ID to cancel
    symbol: 1,
    price: 0,
    size: 0,
    action: OrderAction::Cancel,
    order_type: OrderType::Gtc,
    reserve_price: 0,
    timestamp: 1002,
    ..Default::default()
};

orderbook.new_order(&mut cancel_cmd);
```

### Step 7: Complete Example with Multiple Order Types

```rust
use matching_core::api::*;
use matching_core::core::orderbook::AdvancedOrderBook;
use std::time::{SystemTime, UNIX_EPOCH};

fn get_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn main() {
    // 1. Create symbol specification
    let spec = CoreSymbolSpecification {
        symbol_id: 1,
        symbol_type: SymbolType::CurrencyExchangePair,
        base_currency: 0,
        quote_currency: 1,
        base_scale_k: 1,
        quote_scale_k: 1,
        taker_fee: 10,
        maker_fee: 5,
        margin_buy: 0,
        margin_sell: 0,
    };

    // 2. Initialize order book
    let mut orderbook = AdvancedOrderBook::new(spec);

    // 3. Place multiple sell orders (Makers)
    for i in 1..=5 {
        let mut sell = OrderCommand {
            uid: 100,
            order_id: i,
            symbol: 1,
            price: 50000 + (i * 100) as u64,  // Prices: 50100, 50200, etc.
            size: 10 * i as u64,
            action: OrderAction::Ask,
            order_type: OrderType::Gtc,
            reserve_price: 50000 + (i * 100) as u64,
            timestamp: get_timestamp(),
            ..Default::default()
        };
        orderbook.new_order(&mut sell);
        println!("Placed sell order {} at price {}", i, sell.price);
    }

    // 4. Place buy order (Taker) - will match with best ask
    let mut buy = OrderCommand {
        uid: 101,
        order_id: 100,
        symbol: 1,
        price: 51000,      // Higher than best ask, will match at best ask
        size: 30,          // Will match with first 3 sell orders
        action: OrderAction::Bid,
        order_type: OrderType::Ioc,
        reserve_price: 51000,
        timestamp: get_timestamp(),
        ..Default::default()
    };

    orderbook.new_order(&mut buy);

    // 5. Process and display trade events
    println!("\nTrade Events:");
    for event in &buy.matcher_events {
        if let MatcherEventType::Trade = event.event_type {
            println!(
                "  Trade: {} @ {} (Maker: {}, Taker: {})",
                event.size,
                event.price,
                event.maker_order_id,
                event.taker_order_id
            );
        }
    }

    // 6. Query order book state
    println!("\nOrder Book State:");
    let best_bid = orderbook.get_best_bid();
    let best_ask = orderbook.get_best_ask();
    println!("Best bid: {:?}", best_bid);
    println!("Best ask: {:?}", best_ask);
}
```

## Advanced Order Types Demo

### Complete Advanced Orders Example

```rust
use matching_core::api::*;
use matching_core::core::orderbook::AdvancedOrderBook;

fn main() {
    let spec = CoreSymbolSpecification {
        symbol_id: 1,
        symbol_type: SymbolType::CurrencyExchangePair,
        base_currency: 0,
        quote_currency: 1,
        base_scale_k: 1,
        quote_scale_k: 1,
        taker_fee: 10,
        maker_fee: 5,
        margin_buy: 0,
        margin_sell: 0,
    };

    let mut orderbook = AdvancedOrderBook::new(spec);

    // 1. Post-Only Order (will be rejected if it would immediately match)
    println!("=== Post-Only Order Demo ===");
    let mut post_only = OrderCommand {
        uid: 1,
        order_id: 1,
        symbol: 1,
        price: 50000,
        size: 10,
        action: OrderAction::Bid,
        order_type: OrderType::PostOnly,
        reserve_price: 50000,
        timestamp: 1000,
        ..Default::default()
    };
    orderbook.new_order(&mut post_only);
    println!("Post-Only order result: {:?}", post_only.matcher_events);

    // 2. Iceberg Order (hides true order size)
    println!("\n=== Iceberg Order Demo ===");
    let mut iceberg = OrderCommand {
        uid: 1,
        order_id: 2,
        symbol: 1,
        price: 50000,
        size: 1000,           // Total: 1000
        action: OrderAction::Ask,
        order_type: OrderType::Iceberg,
        reserve_price: 50000,
        timestamp: 1001,
        visible_size: Some(100),  // Only show 100
        ..Default::default()
    };
    orderbook.new_order(&mut iceberg);
    println!("Iceberg order placed: total={}, visible={}", 
        iceberg.size, iceberg.visible_size.unwrap());

    // 3. Stop Limit Order
    println!("\n=== Stop Limit Order Demo ===");
    let mut stop_limit = OrderCommand {
        uid: 1,
        order_id: 3,
        symbol: 1,
        price: 51000,         // Limit price
        size: 10,
        action: OrderAction::Bid,
        order_type: OrderType::StopLimit,
        reserve_price: 51000,
        timestamp: 1002,
        stop_price: Some(50500),  // Trigger when price reaches 50500
        ..Default::default()
    };
    orderbook.new_order(&mut stop_limit);
    println!("Stop limit order placed: trigger={}, limit={}", 
        stop_limit.stop_price.unwrap(), stop_limit.price);

    // 4. GTD Order (Good-Till-Date)
    println!("\n=== GTD Order Demo ===");
    let mut gtd = OrderCommand {
        uid: 1,
        order_id: 4,
        symbol: 1,
        price: 50000,
        size: 100,
        action: OrderAction::Ask,
        order_type: OrderType::Gtd(2000),  // Expires at timestamp 2000
        reserve_price: 50000,
        timestamp: 1003,
        expire_time: Some(2000),
        ..Default::default()
    };
    orderbook.new_order(&mut gtd);
    println!("GTD order placed: expires at {}", gtd.expire_time.unwrap());

    // 5. FOK Order (Fill-or-Kill)
    println!("\n=== FOK Order Demo ===");
    let mut fok = OrderCommand {
        uid: 2,
        order_id: 5,
        symbol: 1,
        price: 50000,
        size: 50,            // Will try to fill 50
        action: OrderAction::Bid,
        order_type: OrderType::Fok,
        reserve_price: 50000,
        timestamp: 1004,
        ..Default::default()
    };
    orderbook.new_order(&mut fok);
    
    if fok.matcher_events.iter().any(|e| matches!(e.event_type, MatcherEventType::Trade)) {
        println!("FOK order filled successfully");
    } else {
        println!("FOK order cancelled (could not fill completely)");
    }
}
```

## Performance Metrics

### Throughput
- **TPS (Transactions Per Second)**: Supports millions of orders per second
  - 10,000 orders: **7,247,910 TPS**
  - 100,000 orders: **4,968,213 TPS**
- **QPS (Queries Per Second)**: High-concurrency matching queries
  - 10,000 orders: **3,623,955 QPS**
  - 100,000 orders: **2,484,106 QPS**

### Latency
- **Average Latency**: < 1 microsecond (single order processing)
- **Batch Processing**: ~1.38 milliseconds for 10,000 orders
- **P99 Latency**: < 10 microseconds

### Memory
- **Memory Usage**: Optimized SOA layout, reduced memory fragmentation
  - 10,000 orders: **1.91 MB**
  - 100,000 orders: **19.07 MB**
- **Order Pool**: Pre-allocation mechanism, reduced dynamic allocation

### Performance Data Table

| Orders | TPS | QPS | Memory (MB) | Latency (ms) |
|---------|-----|-----|------------|--------------|
| 1,000 | 6,559,183 | 3,279,591 | 0.19 | 0.15 |
| 5,000 | 7,242,000 | 3,621,000 | 0.95 | 0.69 |
| 10,000 | 7,247,910 | 3,623,955 | 1.91 | 1.38 |
| 50,000 | 3,834,037 | 1,917,018 | 9.54 | 13.04 |
| 100,000 | 4,968,213 | 2,484,106 | 19.07 | 20.13 |

### Benchmarking

Generate performance data:

```bash
cargo run --example generate_benchmark_data --release
```

Generate performance charts (requires matplotlib and pandas):

```bash
pip3 install matplotlib pandas
python3 scripts/plot_benchmark.py
```

View comprehensive test report:

```bash
cargo run --example comprehensive_test --release
```

Run Criterion benchmarks:

```bash
cargo bench --bench comprehensive_bench
```

## Project Structure

```
matching-core/
├── src/
│   ├── api/              # API type definitions
│   │   ├── types.rs      # Basic types
│   │   ├── commands.rs   # Order commands
│   │   └── events.rs     # Matching events
│   ├── core/             # Core engine
│   │   ├── exchange.rs   # Exchange core
│   │   ├── pipeline.rs   # Processing pipeline
│   │   ├── orderbook/    # Order book implementation
│   │   │   ├── naive.rs           # Basic implementation
│   │   │   ├── direct.rs          # High-performance implementation
│   │   │   ├── direct_optimized.rs # Deep optimization
│   │   │   └── advanced.rs        # Advanced order types
│   │   ├── processors/   # Processors
│   │   │   ├── risk_engine.rs     # Risk engine
│   │   │   └── matching_engine.rs # Matching engine
│   │   ├── journal.rs    # WAL logging
│   │   └── snapshot.rs   # Snapshots
│   └── lib.rs
├── examples/             # Example code
│   ├── advanced_demo.rs      # Advanced order demo
│   ├── comprehensive_test.rs # Comprehensive test
│   └── load_test.rs          # Load test
├── benches/              # Benchmarks
│   ├── comprehensive_bench.rs # Comprehensive benchmark
│   └── advanced_orderbook_bench.rs
└── tests/                # Unit tests
    ├── advanced_orders_test.rs
    └── edge_cases_test.rs
```

## Supported Order Types

| Order Type | Description | Status |
|------------|-------------|--------|
| GTC | Good-Till-Cancel, until cancelled | ✅ |
| IOC | Immediate-or-Cancel, immediate fill or cancel | ✅ |
| FOK | Fill-or-Kill, fill all or cancel all | ✅ |
| Post-Only | Maker only, reject orders that would immediately match | ✅ |
| Stop Limit | Stop limit order | ✅ |
| Stop Market | Stop market order | ✅ |
| Iceberg | Iceberg order, hides true order size | ✅ |
| Day | Valid for the day | ✅ |
| GTD | Good-Till-Date, expires on specified date | ✅ |

## Supported Trading Instruments

| Instrument Type | Description | Status |
|-----------------|-------------|--------|
| CurrencyExchangePair | Spot trading pair | ✅ |
| FuturesContract | Futures contract | ✅ |
| PerpetualSwap | Perpetual contract | ✅ |
| CallOption | Call option | ✅ |
| PutOption | Put option | ✅ |

## Repository

```text
https://github.com/llc-993/matching-core
```

## Running Examples

### Basic Matching Demo

```bash
cargo run --example advanced_demo --release
```

### Comprehensive Test Suite

```bash
cargo run --example comprehensive_test --release
```

### Load Test

```bash
cargo run --example load_test --release
```

## Testing

Run all tests:

```bash
cargo test --release
```

Run specific tests:

```bash
cargo test --test advanced_orders_test --release
cargo test --test edge_cases_test --release
```

## Dependencies

Main dependencies:
- `disruptor`: LMAX Disruptor pattern implementation
- `rkyv`: Zero-copy serialization
- `ahash`: Fast hash algorithm
- `slab`: Object pool
- `smallvec`: Small vector optimization
- `serde`: Serialization framework

