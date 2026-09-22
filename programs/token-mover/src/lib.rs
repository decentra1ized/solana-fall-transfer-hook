pub mod instructions;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("89F2Kp7Kf2XakaRyRFuANsszE6jpZ8dq4bfGsHnPAkrg");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        instructions::transfer_with_hook::handler(ctx, amount)
    }
}
