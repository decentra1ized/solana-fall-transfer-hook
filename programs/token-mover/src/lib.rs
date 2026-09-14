pub mod transfer_ix;

use anchor_lang::prelude::*;

declare_id!("3EYYznbubTgotYwKM9EZYCTs1sRnUhnqcWF2kMnsMdM6");

pub use transfer_ix::*;

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook(ctx: Context<TransferWithHook>, amount: u64) -> Result<()> {
        transfer_ix::handler(ctx, amount)
    }
}
