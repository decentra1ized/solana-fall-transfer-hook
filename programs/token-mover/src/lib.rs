pub mod transfer;

use anchor_lang::prelude::*;
pub use transfer::*;

declare_id!("4QPjNYnAKxeQA9zNqQ86Zz77fFRsb7eNas2pBjiNzwkP");

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