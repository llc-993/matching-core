use crate::api::*;
use ahash::AHashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub uid: UserId,
    pub accounts: AHashMap<Currency, i64>, // 运行时使用 AHashMap（性能更好）
    pub positions: AHashMap<SymbolId, SymbolPositionRecord>,
}

impl UserProfile {
    pub fn new(uid: UserId) -> Self {
        Self {
            uid,
            accounts: AHashMap::new(),
            positions: AHashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolPositionRecord {
    pub uid: UserId,
    pub symbol: SymbolId,
    pub currency: Currency,
    pub direction: i32,
    pub open_volume_long: i64,
    pub open_volume_short: i64,
    pub open_price_long: i64,
    pub open_price_short: i64,
    pub profit: i64,
    pub pending_buy_size: i64,
    pub pending_sell_size: i64,
}

impl SymbolPositionRecord {
    pub fn new(uid: UserId, symbol: SymbolId, currency: Currency) -> Self {
        Self {
            uid,
            symbol,
            currency,
            direction: 0,
            open_volume_long: 0,
            open_volume_short: 0,
            open_price_long: 0,
            open_price_short: 0,
            profit: 0,
            pending_buy_size: 0,
            pending_sell_size: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.open_volume_long == 0
            && self.open_volume_short == 0
            && self.pending_buy_size == 0
            && self.pending_sell_size == 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfileService {
    profiles: AHashMap<UserId, UserProfile>, // 运行时使用 AHashMap
}

impl UserProfileService {
    pub fn new() -> Self {
        Self {
            profiles: AHashMap::new(),
        }
    }

    pub fn add_user(&mut self, uid: UserId) -> bool {
        if self.profiles.contains_key(&uid) {
            false
        } else {
            self.profiles.insert(uid, UserProfile::new(uid));
            true
        }
    }

    pub fn get_user(&self, uid: UserId) -> Option<&UserProfile> {
        self.profiles.get(&uid)
    }

    pub fn get_user_mut(&mut self, uid: UserId) -> Option<&mut UserProfile> {
        self.profiles.get_mut(&uid)
    }

    pub fn balance_adjustment(
        &mut self,
        uid: UserId,
        currency: Currency,
        amount: i64,
        _transaction_id: i64,
    ) -> CommandResultCode {
        if let Some(profile) = self.profiles.get_mut(&uid) {
            *profile.accounts.entry(currency).or_insert(0) += amount;
            CommandResultCode::Success
        } else {
            CommandResultCode::AuthInvalidUser
        }
    }
}
