use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use anchor_spl::token_2022::spl_token_2022;
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

declare_id!("3EYYznbubTgotYwKM9EZYCTs1sRnUhnqcWF2kMnsMdM6");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'a>(
        ctx: Context<'a, TransferWithHook<'a>>,
        amount: u64,
    ) -> Result<()> {
        let source = ctx.accounts.source_token.to_account_info();
        let mint_ai = ctx.accounts.mint.to_account_info();
        let destination = ctx.accounts.destination_token.to_account_info();
        let owner_ai = ctx.accounts.owner.to_account_info();
        let decimals = ctx.accounts.mint.decimals;
        let token_program_id = ctx.accounts.token_program.key();
        let hook_program_id = ctx.remaining_accounts[0].key();

        let mut ix = spl_token_2022::instruction::transfer_checked(
            &token_program_id,
            &source.key(),
            &mint_ai.key(),
            &destination.key(),
            &owner_ai.key(),
            &[],
            amount,
            decimals,
        )?;

        let mut infos: Vec<AccountInfo> = vec![
            source.clone(),
            mint_ai.clone(),
            destination.clone(),
            owner_ai.clone(),
        ];

        add_extra_accounts_for_execute_cpi(
            &mut ix,
            &mut infos,
            &hook_program_id,
            source,
            mint_ai,
            destination,
            owner_ai,
            amount,
            &ctx.remaining_accounts,
        )?;

        anchor_lang::solana_program::program::invoke(&ix, &infos)?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct TransferWithHook<'info> {
    pub owner: Signer<'info>,
    #[account(mut, token::mint = mint, token::authority = owner)]
    pub source_token: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut, token::mint = mint)]
    pub destination_token: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}
