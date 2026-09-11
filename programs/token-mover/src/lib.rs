pub mod instructions;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("SxB3pkCi2syAPfMUa2VsvzWdxGhrUDL8peNRcepV2e6");

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
