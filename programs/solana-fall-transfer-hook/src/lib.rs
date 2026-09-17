pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;
use spl_discriminator::SplDiscriminate;
use spl_transfer_hook_interface::instruction::ExecuteInstruction;

pub use constants::*;
pub use state::*;

use instructions::initialize::Initialize;
use instructions::initialize_mint::InitializeMint;
use instructions::init_extra_account_meta::InitializeExtraAccountMetaList;
use instructions::transfer_hook::TransferHook;

declare_id!("2JfEsBs1CWdqsrgrPzqZywenRgjTP1cRwuybYyRtccuM");

#[program]
pub mod solana_fall_transfer_hook {

    use super::*;

    pub fn initialize_mint(ctx: Context<InitializeMint>) -> Result<()> {
        instructions::initialize_mint::handler(ctx)
    }

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        instructions::initialize::handler(ctx)
    }

    pub fn initialize_extra_account_meta_list(
        ctx: Context<InitializeExtraAccountMetaList>,
    ) -> Result<()> {
        instructions::init_extra_account_meta::handler(ctx)
    }

    #[instruction(discriminator = ExecuteInstruction::SPL_DISCRIMINATOR_SLICE)]
    pub fn transfer_hook(ctx: Context<TransferHook>, amount: u64) -> Result<()> {
        instructions::transfer_hook::handler(ctx, amount)
    }

}
