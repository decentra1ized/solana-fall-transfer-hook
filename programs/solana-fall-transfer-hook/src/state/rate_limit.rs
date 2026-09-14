use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct RateLimit {
    pub authority: Pubkey,
    pub mint: Pubkey,          // CHALLENGE 2: Added mint field
    pub max_amount: u64,
    pub window_start: i64,
    pub amount_transferred: u64,
}

impl RateLimit {
    pub const MAX_AMOUNT: u64 = 1_000_000;
    pub const ONE_HOUR: i64 = 3600;

    pub fn is_expired(&self, current_time: i64, duration: i64) -> bool {
        current_time - self.window_start > duration
    }

    pub fn reset(&mut self, current_time: i64) {
        self.window_start = current_time;
        self.amount_transferred = 0;
    }

    pub fn limit_exceeded(&self, amount: u64) -> bool {
        self.amount_transferred.saturating_add(amount) > self.max_amount
    }

    pub fn update(&mut self, amount: u64) {
        self.amount_transferred = self.amount_transferred.saturating_add(amount);
    }
}
