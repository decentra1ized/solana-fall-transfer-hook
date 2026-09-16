use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("The mint has no transfer hook program set")]
    NoTransferHook,
}
