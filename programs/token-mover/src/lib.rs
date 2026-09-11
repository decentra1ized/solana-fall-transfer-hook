pub mod transfer;

use anchor_lang::prelude::*;

pub use transfer::*;

declare_id!("Fz9KWivnsnb5V2U3DcGYWpgyzFBN3od31EqzMRd6UrqM");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        transfer::handler(ctx, amount, decimals)
    }
}
