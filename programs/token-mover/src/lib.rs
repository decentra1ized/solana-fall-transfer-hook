pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("Dzvsn5KRY1CUPCo8TYXzoJvQ8bYbYJ1tCb4YMdSCbg6M");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        crate::instructions::transfer_with_hook::handle_transfer_with_hook(ctx, amount, decimals)
    }
}
