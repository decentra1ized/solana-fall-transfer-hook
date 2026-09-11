use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct RateLimit {
    pub authority: Pubkey,
    pub mint: Pubkey,
    pub max_amount: u64,
    pub window_start: i64,
    pub amount_transferred: u64,
}

impl RateLimit {
    pub fn limit_exceeded(&self, amount: u64) -> bool {
        self.amount_transferred.saturating_add(amount) > self.max_amount
    }

    pub fn update(&mut self, amount: u64) {
        self.amount_transferred = self.amount_transferred.saturating_add(amount);
    }

    pub fn reset(&mut self, now: i64) {
        self.amount_transferred = 0;
        self.window_start = now;
    }

    pub fn is_expired(&self, now: i64, window: i64) -> bool {
        now - self.window_start > window
    }

    pub const MAX_AMOUNT: u64 = 1_000_000;
}
