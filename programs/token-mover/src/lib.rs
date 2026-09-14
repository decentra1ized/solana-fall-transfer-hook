pub mod instructions;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("HyVRH4sSU32Hob1RNJYDBTZEBTTbp1oNzZB8YP3nPrFt");

/// A minimal program that moves Token-2022 tokens on a user's behalf.
///
/// It exists separately from the hook program on purpose. Solana permits a
/// program to call itself directly, but not to be re-entered through another
/// program while it is still executing. Putting this instruction inside the
/// hook would give hook -> Token-2022 -> hook and fail with
/// ReentrancyNotAllowed; as its own program the chain is
/// mover -> Token-2022 -> hook, and no program appears twice.
#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        instructions::transfer_with_hook::handler(ctx, amount)
    }
}
