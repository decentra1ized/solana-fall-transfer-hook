pub mod instructions;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("CeiWFb5oj3YFHLQpRwbFYBjFeTDC57a8WxUN1m3j4D6D");

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
