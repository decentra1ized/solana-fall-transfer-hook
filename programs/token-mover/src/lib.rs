pub mod transfer;

use anchor_lang::prelude::*;
pub use transfer::*;

declare_id!("J3SRcFJV4FXRWZZm2uzbCDeWfoFr2wP3HvtJqxS8n182");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        transfer::handler(ctx, amount)
    }
}