pub mod instructions;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("9jd4fjb23e3wmLHrdA76Giq4akqXcP8fJRKXyw2Udc7S");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        crate::instructions::transfer_with_hook::handler(ctx, amount)
    }
}
