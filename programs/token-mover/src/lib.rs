pub mod instructions;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("665hPd5Zx3BpHjQiwWeXdzPr9RBiMR8yUad7GKcL3gHP");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer<'info>(ctx: Context<'info, TransferWithHook<'info>>, amount: u64) -> Result<()> {
        crate::instructions::transfer::handler(ctx, amount)
    }
}
