pub mod instructions;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("FWBmTDPx7uva7vtt93EbFu5YgoSEwkrudFKmTUKb6NSh");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        crate::instructions::transfer::handler(ctx, amount)
    }
}
