pub mod error;
pub mod instructions;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("FGD3pBmakx3wF1Yev1MhCy49fUtNWR9ErDmUWMum6Gt9");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        transfer_with_hook::handler(ctx, amount)
    }
}
