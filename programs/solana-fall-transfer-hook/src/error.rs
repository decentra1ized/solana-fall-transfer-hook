use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Custom error message")]
    CustomError,
    #[msg("Rate limit exceeded")]
    RateLimitExceeded,
    #[msg("Invalid mint account")]
    InvalidMint,
    #[msg("Invalid mint for rate limit account")]
    InvalidMintForRateLimit,
    #[msg("Transfer hook invoked outside of an active transfer")]
    NotTransferring,
}
