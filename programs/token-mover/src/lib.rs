use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::spl_token_2022,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

declare_id!("FBuPLnKbSgFJNiUUC1v31NPS8GHEoBj6zARzvdZU5Wzg");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'info>(
        ctx: Context<'info, TransferWithHook<'info>>,
        amount: u64,
    ) -> Result<()> {
        let source = ctx.accounts.source_token.to_account_info();
        let mint = ctx.accounts.mint.to_account_info();
        let destination = ctx.accounts.destination_token.to_account_info();
        let owner = ctx.accounts.owner.to_account_info();

        // The hook program must be the first remaining account.
        let hook_program_id = ctx.remaining_accounts[0].key();

        // 1. Build the Token-2022 transfer_checked instruction.
        let mut ix = spl_token_2022::instruction::transfer_checked(
            ctx.accounts.token_program.key,
            &source.key(),
            &mint.key(),
            &destination.key(),
            &owner.key(),
            &[],
            amount,
            ctx.accounts.mint.decimals,
        )?;

        // 2. Add the normal transfer accounts.
        let mut infos = vec![
            source.clone(),
            mint.clone(),
            destination.clone(),
            owner.clone(),
        ];

        // 3. Add the accounts required by the transfer hook.
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

        // 4. Invoke Token-2022.
        anchor_lang::solana_program::program::invoke(&ix, &infos)?;

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
