pub mod instructions;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("DEcE2jFKtFzJZVXv9mLhRfresDisL8Zk9yChM9LvhyPD");

#[program]
pub mod token_mover {

    use super::*;

    pub fn transfer_with_hook<'info>(ctx: Context<'info, TransferWithHook<'info>>, amount: u64) -> Result<()> {
        transfer_checked::handler(ctx, amount)
    }

}
