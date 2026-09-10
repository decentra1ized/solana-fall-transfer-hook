pub mod instructions;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("Gu7LZhPjzK7YEUGBcz1KdQ2A9oG4MYqNwyQ7waXBxpN8");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        crate::instructions::transfer_with_hook::handle_transfer_with_hook(ctx, amount, decimals)
    }
}
