pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;
use spl_discriminator::SplDiscriminate;
use spl_transfer_hook_interface::instruction::ExecuteInstruction;

pub use constants::*;
pub use instructions::*;
pub use state::*;

// The public address of the transfer-hook program.
//
// Anchor uses this ID to confirm that instructions and PDAs belong
// to the correct program.
declare_id!("2JfEsBs1CWdqsrgrPzqZywenRgjTP1cRwuybYyRtccuM");

/// The main Transfer Hook program.
///
/// This program creates the Token-2022 mint, manages each owner's
/// RateLimit account, and validates transfers when Token-2022 calls
/// the hook.
#[program]
pub mod solana_fall_transfer_hook {
    use super::*;

    /// Creates a Token-2022 mint and connects its Transfer Hook
    /// extension to this program.
    pub fn initialize_mint(ctx: Context<InitializeMint>) -> Result<()> {
        initialize_mint::handler(ctx)
    }

    /// Creates a RateLimit PDA for one mint and one token owner.
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        initialize::handler(ctx)
    }

    /// Creates the metadata account that tells Token-2022 how to find
    /// the additional accounts required by the transfer hook.
    pub fn initialize_extra_account_meta_list(
        ctx: Context<InitializeExtraAccountMetaList>,
    ) -> Result<()> {
        init_extra_account_meta::handler(ctx)
    }

    /// Token-2022 calls this instruction automatically during a transfer.
    ///
    /// The special SPL discriminator makes this instruction compatible
    /// with the official Token-2022 Transfer Hook interface.
    #[instruction(
        discriminator = ExecuteInstruction::SPL_DISCRIMINATOR_SLICE
    )]
    pub fn transfer_hook(ctx: Context<TransferHook>, amount: u64) -> Result<()> {
        transfer_hook::handler(ctx, amount)
    }
}
