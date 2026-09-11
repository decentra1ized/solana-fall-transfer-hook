pub mod instructions;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("4XVSBLrhe9wcgL1TNBRHXwEa3nphwT2bQGHNo6Y7BLge");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        instructions::transfer_with_hook::handler(ctx, amount, decimals)
    }
}
