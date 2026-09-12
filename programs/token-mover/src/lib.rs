pub mod instructions;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("6Mf9NjVm56sjDdEkjW2Uep4V6vWR9QBHTmdQfsP8XGeE");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        instructions::transfer::handler(ctx, amount)
    }
}
