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
    #[msg("The mint is not owned by the Token-2022 program")]
    InvalidMintOwner,
}
