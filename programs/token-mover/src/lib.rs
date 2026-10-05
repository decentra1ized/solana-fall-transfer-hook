use anchor_lang::prelude::*;
use anchor_lang::solana_program::program_error::ProgramError;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

declare_id!("5a9YfzxbTZk9Jor4E37u4eFjaqyMsQdvUCBcZid9FUdn");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_with_hook<'a>(
        ctx: Context<'a, TransferWithHook<'a>>,
        amount: u64,
    ) -> Result<()> {
        let mut ix = anchor_spl::token_2022::spl_token_2022::instruction::transfer_checked(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.source_token.key(),
            &ctx.accounts.mint.key(),
            &ctx.accounts.destination_token.key(),
            &ctx.accounts.owner.key(),
            &[],
            amount,
            ctx.accounts.mint.decimals,
        )?;

        let hook_program = ctx
            .remaining_accounts
            .first()
            .ok_or(ProgramError::InvalidInstructionData)?;
        let mut infos = vec![
            ctx.accounts.source_token.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.destination_token.to_account_info(),
            ctx.accounts.owner.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
        ];
        add_extra_accounts_for_execute_cpi(
            &mut ix,
            &mut infos,
            hook_program.key,
            ctx.accounts.source_token.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.destination_token.to_account_info(),
            ctx.accounts.owner.to_account_info(),
            amount,
            ctx.remaining_accounts,
        )?;
        infos.extend_from_slice(ctx.remaining_accounts);
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
