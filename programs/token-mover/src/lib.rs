pub mod error;
pub mod instructions;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("G4ULKFLLbnkXnSThQdmf3QFDgh88a5B6YebB8nLSpfRX");

/// A second program that moves Token-2022 tokens on behalf of a user.
///
/// The transfer instruction cannot live in the transfer hook program itself:
/// Token-2022 would call the hook back while that same program is still on the
/// call stack, and Solana rejects re-entrancy. Splitting the transfer into its
/// own program keeps the call chain linear.
#[program]
pub mod token_mover {

    use super::*;

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        transfer_with_hook::handler(ctx, amount)
    }
}
