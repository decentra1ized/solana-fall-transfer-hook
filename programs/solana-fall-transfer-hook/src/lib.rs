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

declare_id!("5u5R4Ui3jKtsTfoghhh8Gasgwovtcr9zXDhqJPBq2Tsj");

#[program]
pub mod solana_fall_transfer_hook {

    use super::*;

    pub fn initialize_mint(ctx: Context<InitializeMint>) -> Result<()> {
        initialize_mint::handler(ctx)
    }

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        initialize::handler(ctx)
    }

    pub fn initialize_extra_account_meta_list(
        ctx: Context<InitializeExtraAccountMetaList>,
    ) -> Result<()> {
        init_extra_account_meta::handler(ctx)
    }

    #[instruction(discriminator = ExecuteInstruction::SPL_DISCRIMINATOR_SLICE)]
    pub fn transfer_hook(ctx: Context<TransferHook>, amount: u64) -> Result<()> {
        transfer_hook::handler(ctx, amount)
    }

}
