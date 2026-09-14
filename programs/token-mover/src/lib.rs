pub mod instructions;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("DcrS988SMDyxs7g5AJjRfxQPxB29ArUbqKWDSTRHB6wb");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        instructions::handler(ctx, amount)
    }
}
