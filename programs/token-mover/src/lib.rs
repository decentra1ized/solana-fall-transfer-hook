use anchor_lang::prelude::*;

pub mod instructions;
pub use instructions::*;

declare_id!("BtDWto9KheJtagyBtUKgXqh2P5n7sw9csi2REx8gLexZ");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(ctx: Context<'info, TransferWithHook<'info>>, amount: u64) -> Result<()> {
        instructions::transfer_with_hook::handler(ctx, amount)
    }
}
