use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("The hook program must be passed as the first remaining account")]
    MissingHookProgram,
}
