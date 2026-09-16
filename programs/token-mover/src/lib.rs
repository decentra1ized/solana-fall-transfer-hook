pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("7frLs5HjQV7hmGccUPGvBfS3bCY2Sbb7U47Eqk5aGhmn");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        transfer::handler(ctx, amount)
    }
}
