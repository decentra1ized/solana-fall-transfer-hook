pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("9qtpT2XcoJHQtrZrvVrTUXkgvbv5hDyrtUG4dGKDWZHK");

#[program]
pub mod token_mover {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        crate::instructions::initialize::handle_initialize(ctx)
    }

    pub fn increment(ctx: Context<Increment>) -> Result<()> {
        crate::instructions::increment::handle_increment(ctx)
    }

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        crate::instructions::transfer_with_hook::handle_transfer_with_hook(
            ctx,
            amount,
            decimals,
        )
    }
}
