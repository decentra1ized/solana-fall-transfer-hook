pub mod constants;
pub mod error;
pub mod instructions;

use anchor_lang::prelude::*;

pub use instructions::*;

pub use constants::*;
declare_id!("Dj5QqcpmvVSdqMdzvusRwMHQMzt2odaSV1C451P5p5Ab");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        instructions::transfer_checked::handler(ctx, amount)
    }
}
