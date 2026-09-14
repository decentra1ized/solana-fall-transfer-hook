use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Custom error message")]
    CustomError,
    #[msg("Rate limit exceeded")]
    RateLimitExceeded,
    #[msg("Invalid mint account")]
    InvalidMint,
    #[msg("Transfer hook invoked outside of an active transfer")]
    NotTransferring,
    #[msg("Missing rate limit account")]
    MissingRateLimit,
    #[msg("Invalid rate limit account")]
    InvalidRateLimit,
    #[msg("Rate limit account must be writable")]
    ReadonlyRateLimit,
}
