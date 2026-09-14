pub mod transfer;

use anchor_lang::prelude::*;

pub use transfer::*;

declare_id!("BuVxQA1UTLGhJHiHLtHm9uoDupgxA8fbofBaez5Diyes");

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