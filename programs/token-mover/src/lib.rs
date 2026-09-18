use anchor_lang::prelude::*;
use anchor_lang::solana_program;
use anchor_spl::token_2022::spl_token_2022;
use spl_transfer_hook_interface::onchain::add_extra_accounts_for_execute_cpi;

declare_id!("2noUx1sNUg4yqxPYa7QJ3xW4CbwRCxKdnUCgp3TrmTw9");

#[program]
pub mod token_mover {
    use super::*;

    pub fn transfer_tokens<'info>(
        ctx: Context<'info, TransferTokens<'info>>,
        amount: u64,
        decimals: u8,
    ) -> Result<()> {
        let mut transfer_ix = spl_token_2022::instruction::transfer_checked(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.source.key(),
            &ctx.accounts.mint.key(),
            &ctx.accounts.destination.key(),
            &ctx.accounts.authority.key(),
            &[],
            amount,
            decimals,
        )?;

        let mut cpi_account_infos = vec![
            ctx.accounts.source.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.destination.to_account_info(),
            ctx.accounts.authority.to_account_info(),
        ];

        add_extra_accounts_for_execute_cpi(
            &mut transfer_ix,
            &mut cpi_account_infos,
            &ctx.accounts.transfer_hook_program.key(),
            ctx.accounts.source.to_account_info(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.destination.to_account_info(),
            ctx.accounts.authority.to_account_info(),
            amount,
            ctx.remaining_accounts,
        )?;

        cpi_account_infos.push(ctx.accounts.token_program.to_account_info());

        solana_program::program::invoke(
            &transfer_ix,
            &cpi_account_infos,
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct TransferTokens<'info> {
    /// CHECK: Source token account
    #[account(mut)]
    pub source: UncheckedAccount<'info>,
    /// CHECK: Mint account
    pub mint: UncheckedAccount<'info>,
    /// CHECK: Destination token account
    #[account(mut)]
    pub destination: UncheckedAccount<'info>,
    /// CHECK: Authority/owner of source
    pub authority: Signer<'info>,
    /// CHECK: Token-2022 program
    pub token_program: UncheckedAccount<'info>,
    /// CHECK: Transfer hook program
    pub transfer_hook_program: UncheckedAccount<'info>,
}
