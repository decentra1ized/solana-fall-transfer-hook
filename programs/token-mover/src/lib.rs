pub mod instructions;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("944aFDbhVh49S1MKL75vRk4RabKFrFMnBKnG2NQLkkAx");

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
