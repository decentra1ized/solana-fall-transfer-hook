use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("The mint does not have a transfer hook program configured")]
    NoTransferHook,
}
