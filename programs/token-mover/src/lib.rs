use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke;
use anchor_spl::token_2022::spl_token_2022;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

declare_id!("J2q1RQvwr41WTPeVSn4DKxd2u4Fs7n8NMKp7fmbhmodp");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'a>(
        ctx: Context<'a, TransferWithHook<'a>>,
        amount: u64,
    ) -> Result<()> {
        let source = ctx.accounts.source_token.to_account_info();
        let mint = ctx.accounts.mint.to_account_info();
        let destination = ctx.accounts.destination_token.to_account_info();
        let owner = ctx.accounts.owner.to_account_info();
        let token_program = ctx.accounts.token_program.to_account_info();

        let decimals = ctx.accounts.mint.decimals;
        let hook_program_id = ctx.remaining_accounts[0].key();

        let mut ix = spl_token_2022::instruction::transfer_checked(
            token_program.key,
            source.key,
            mint.key,
            destination.key,
            owner.key,
            &[],
            amount,
            decimals,
        )?;

        let mut infos = vec![
            source.clone(),
            mint.clone(),
            destination.clone(),
            owner.clone(),
        ];

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

        invoke(&ix, &infos)?;

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
