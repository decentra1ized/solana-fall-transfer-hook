use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

declare_id!("asgSw6pju177UiPXzGzjnNtAAvfKhsDFvaoKQLeeuku");

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

        let mut instruction =
            anchor_spl::token_2022::spl_token_2022::instruction::transfer_checked(
                &ctx.accounts.token_program.key(),
                source.key,
                mint.key,
                destination.key,
                owner.key,
                &[],
                amount,
                ctx.accounts.mint.decimals,
            )?;
        let mut account_infos = vec![
            source.clone(),
            mint.clone(),
            destination.clone(),
            owner.clone(),
        ];
        let hook_program = ctx
            .remaining_accounts
            .first()
            .ok_or(ProgramError::NotEnoughAccountKeys)?;

        add_extra_accounts_for_execute_cpi(
            &mut instruction,
            &mut account_infos,
            hook_program.key,
            source,
            mint,
            destination,
            owner,
            amount,
            ctx.remaining_accounts,
        )?;

        anchor_lang::solana_program::program::invoke(&instruction, &account_infos)?;
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
