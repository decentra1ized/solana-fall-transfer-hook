use anchor_lang::prelude::*;

pub mod instructions;
pub use instructions::*;

declare_id!("AHjtgrDjJvphzroaA5UE21R8hkvJPNVQMYkLGd5D18Tf");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        transfer_checked::handler(ctx, amount, decimals)
    }
}