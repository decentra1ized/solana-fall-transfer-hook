 use anchor_lang::prelude::*;

pub mod instructions;

use instructions::transfer::*;

declare_id!("2ey6hxim9uPPFKRoQVHV1AtP7yiLKRnagdU1PVpMNvj1");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        instructions::transfer::handler(ctx, amount)
    }
}