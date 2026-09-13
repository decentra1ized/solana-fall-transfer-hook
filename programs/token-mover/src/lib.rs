pub mod instructions;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("BayGZRTSTBZWYzoiZSxPVnMyYzfp6qun46t2BTvuYFUZ");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer<'info>(ctx: Context<'info, TransferWithHook<'info>>, amount: u64) -> Result<()> {
        crate::instructions::transfer::handler(ctx, amount)
    }
}
