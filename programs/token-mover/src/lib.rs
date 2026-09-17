pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::{
    token_2022::spl_token_2022,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("47XDPL8uis7NurtvdUspADkFB4Ed9PZE7yuDUgyC6WfJ");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook(
        ctx: Context<TransferWithHook>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {

        // Accounts from our TransferWithHook context
        let source = ctx.accounts.source_token.to_account_info();
        let mint = ctx.accounts.mint.to_account_info();
        let destination = ctx.accounts.destination_token.to_account_info();
        let owner = ctx.accounts.owner.to_account_info();

        // The transfer-hook program is the first remaining account.
        let hook_program_id = ctx.remaining_accounts[0].key();

        // 1. Build a normal Token-2022 transfer_checked instruction
        let mut ix = spl_token_2022::instruction::transfer_checked(
            &spl_token_2022::ID,
            &source.key(),
            &mint.key(),
            &destination.key(),
            &owner.key(),
            &[],
            amount,
            decimals,
        )?;

        // 2. Account infos in the same order as the transfer instruction
        let mut infos = vec![
            source.clone(),
            mint.clone(),
            destination.clone(),
            owner.clone(),
        ];

        // 3. Add the accounts required by the transfer hook
        add_extra_accounts_for_execute_cpi(
            &mut ix,
            &mut infos,
            &hook_program_id,
            source,
            mint,
            destination,
            owner,
            amount,
            ctx.remaining_accounts,
        )?;

        // 4. Invoke Token-2022
        invoke(&ix, &infos)?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct TransferWithHook<'info> {
    pub owner: Signer<'info>,

    #[account(
        mut,
        token::mint = mint,
        token::authority = owner,
    )]
    pub source_token: InterfaceAccount<'info, TokenAccount>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        token::mint = mint,
    )]
    pub destination_token: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}