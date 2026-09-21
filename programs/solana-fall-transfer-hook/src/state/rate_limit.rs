use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct RateLimit {
    pub authority: Pubkey,          // The account that can update the rate limit
    pub mint: Pubkey,               // The mint this rate limit applies to
    pub max_amount: u64,            // The maximum amount that can be transferred within one window
    pub window_start: i64,          // The timestamp at which the current window opened
    pub amount_transferred: u64,    // The total amount transferred within the current window
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

    pub const MAX_AMOUNT: u64 = 1_000_000; // Example max amount
}
